use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::support::sha256_hex;
use crate::{
    CellId, CellRegistration, ConduitConfig, ConduitMetrics, ConduitQueue, ScheduledEnvelope,
    SchedulerError, SchedulerMetrics, SiteId, StableScheduler,
};

pub const CELL_SNAPSHOT_SCHEMA_VERSION: &str = "0.2.0";
pub const PREVIOUS_CELL_SNAPSHOT_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum SnapshotValue {
    Boolean(bool),
    Integer(i64),
    Text(String),
    IntegerList(Vec<i64>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentSnapshot {
    pub schema_version: String,
    pub component: String,
    pub component_kind: String,
    pub state: BTreeMap<String, SnapshotValue>,
}

impl ComponentSnapshot {
    pub fn normalize(mut self) -> Result<Self, String> {
        migrate_snapshot_version(&mut self.schema_version)?;
        Ok(self)
    }

    pub fn digest(&self) -> String {
        digest_json(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CellSnapshot {
    pub schema_version: String,
    pub site: SiteId,
    pub cell: CellId,
    pub captured_at_us: u64,
    pub components: Vec<ComponentSnapshot>,
}

impl CellSnapshot {
    pub fn normalize(mut self) -> Result<Self, String> {
        migrate_snapshot_version(&mut self.schema_version)?;
        self.components = self
            .components
            .into_iter()
            .map(ComponentSnapshot::normalize)
            .collect::<Result<Vec<_>, _>>()?;
        self.components
            .sort_by(|left, right| left.component.cmp(&right.component));
        if self
            .components
            .windows(2)
            .any(|items| items[0].component == items[1].component)
        {
            return Err(format!("cell {} repeats a component snapshot", self.cell));
        }
        Ok(self)
    }

    pub fn digest(&self) -> String {
        digest_json(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConduitSnapshot {
    pub id: String,
    pub pending: usize,
    pub high_water: usize,
    pub queue_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSnapshot {
    pub schema_version: String,
    pub model_digest: String,
    pub captured_at_us: u64,
    pub cells: Vec<CellSnapshot>,
    pub conduits: Vec<ConduitSnapshot>,
    pub scheduler: SchedulerMetrics,
}

#[derive(Clone, Debug)]
pub struct ConduitStateSnapshot {
    pub config: ConduitConfig,
    pub pending: Vec<ScheduledEnvelope>,
    pub metrics: ConduitMetrics,
}

#[derive(Clone, Debug)]
pub struct SchedulerStateSnapshot {
    pub cells: Vec<CellRegistration>,
    pub conduits: Vec<ConduitStateSnapshot>,
    pub halted: bool,
    pub captured_at_us: u64,
    pub next_sequence: u64,
    pub metrics: SchedulerMetrics,
}

impl SchedulerStateSnapshot {
    pub fn capture(scheduler: &StableScheduler) -> Result<Self, SchedulerError> {
        if !scheduler.is_sealed() {
            return Err(SchedulerError::NotSealed);
        }
        let (_, halted, captured_at_us, next_sequence, metrics) = scheduler.state();
        Ok(Self {
            cells: scheduler.cells().to_vec(),
            conduits: scheduler
                .conduits()
                .iter()
                .map(|conduit| ConduitStateSnapshot {
                    config: conduit.config().clone(),
                    pending: conduit.snapshot(),
                    metrics: conduit.metrics(),
                })
                .collect(),
            halted,
            captured_at_us,
            next_sequence,
            metrics,
        })
    }

    pub fn restore(self) -> Result<StableScheduler, SchedulerError> {
        let conduits = self
            .conduits
            .into_iter()
            .map(|conduit| ConduitQueue::restore(conduit.config, conduit.pending, conduit.metrics))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(StableScheduler::restore_state(
            self.cells,
            conduits,
            self.halted,
            self.captured_at_us,
            self.next_sequence,
            self.metrics,
        ))
    }
}

impl ProjectSnapshot {
    pub fn from_json(source: &str) -> Result<Self, String> {
        serde_json::from_str::<Self>(source)
            .map_err(|error| format!("invalid project snapshot: {error}"))?
            .normalize()
    }

    pub fn to_json(&self) -> Result<String, String> {
        let normalized = self.clone().normalize()?;
        serde_json::to_string_pretty(&normalized)
            .map(|source| format!("{source}\n"))
            .map_err(|error| format!("cannot serialize project snapshot: {error}"))
    }

    pub fn normalize(mut self) -> Result<Self, String> {
        migrate_snapshot_version(&mut self.schema_version)?;
        self.cells = self
            .cells
            .into_iter()
            .map(CellSnapshot::normalize)
            .collect::<Result<Vec<_>, _>>()?;
        self.cells
            .sort_by(|left, right| (&left.site, &left.cell).cmp(&(&right.site, &right.cell)));
        if self
            .cells
            .windows(2)
            .any(|cells| cells[0].site == cells[1].site && cells[0].cell == cells[1].cell)
        {
            return Err("project snapshot repeats a cell".into());
        }
        Ok(self)
    }

    pub fn capture(
        model_digest: impl Into<String>,
        scheduler: &StableScheduler,
        mut cells: Vec<CellSnapshot>,
    ) -> Result<Self, String> {
        cells = cells
            .into_iter()
            .map(CellSnapshot::normalize)
            .collect::<Result<Vec<_>, _>>()?;
        cells.sort_by(|left, right| (&left.site, &left.cell).cmp(&(&right.site, &right.cell)));
        let conduits = scheduler
            .conduits()
            .iter()
            .map(|conduit| {
                let queue = conduit.snapshot();
                let mut digest = Sha256::new();
                for envelope in queue {
                    digest.update(envelope.sequence.to_le_bytes());
                    digest.update(envelope.sent_at_us.to_le_bytes());
                    digest.update(envelope.deliver_at_us.to_le_bytes());
                    digest.update(envelope.conduit.to_le_bytes());
                    update_text(&mut digest, envelope.source_site.as_str());
                    update_text(&mut digest, envelope.source_cell.as_str());
                    update_text(&mut digest, envelope.destination_site.as_str());
                    update_text(&mut digest, envelope.destination_cell.as_str());
                    envelope
                        .message
                        .visit_canonical_bytes(|bytes| digest.update(bytes));
                }
                ConduitSnapshot {
                    id: conduit.config().id.to_string(),
                    pending: conduit.len(),
                    high_water: conduit.metrics().high_water,
                    queue_digest: {
                        let digest = digest.finalize();
                        let mut output = String::with_capacity(digest.len().saturating_mul(2));
                        use std::fmt::Write as _;
                        for byte in digest {
                            write!(&mut output, "{byte:02x}")
                                .expect("writing hexadecimal to a String");
                        }
                        output
                    },
                }
            })
            .collect();
        Self {
            schema_version: CELL_SNAPSHOT_SCHEMA_VERSION.into(),
            model_digest: model_digest.into(),
            captured_at_us: scheduler.time_us(),
            cells,
            conduits,
            scheduler: scheduler.metrics().clone(),
        }
        .normalize()
    }

    pub fn digest(&self) -> String {
        digest_json(self)
    }
}

fn migrate_snapshot_version(version: &mut String) -> Result<(), String> {
    if version == PREVIOUS_CELL_SNAPSHOT_SCHEMA_VERSION {
        *version = CELL_SNAPSHOT_SCHEMA_VERSION.into();
    }
    if version == CELL_SNAPSHOT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(format!("unsupported snapshot schema {version}"))
    }
}

fn digest_json(value: &impl Serialize) -> String {
    let normalized = serde_json::to_vec(value).expect("snapshot serialization is infallible");
    sha256_hex(&normalized)
}

fn update_text(digest: &mut Sha256, value: &str) {
    digest.update(value.len().to_le_bytes());
    digest.update(value.as_bytes());
}

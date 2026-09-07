use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::Path;

use crate::ProjectSnapshot;
use crate::support::sha256_hex;
use serde::{Deserialize, Serialize};

pub const REPLAY_SCHEMA_VERSION: &str = "0.3.0";
pub const PREVIOUS_REPLAY_SCHEMA_VERSION: &str = "0.2.0";
pub const QUANTIZATION_CONTRACT_VERSION: &str = "0.1.0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RunInput {
    Command {
        at_us: u64,
        source: String,
        target: String,
        command: String,
        #[serde(default)]
        values: BTreeMap<String, i64>,
    },
    Fault {
        at_us: u64,
        target: String,
        fault: String,
        active: bool,
    },
}

impl RunInput {
    pub const fn at_us(&self) -> u64 {
        match self {
            Self::Command { at_us, .. } | Self::Fault { at_us, .. } => *at_us,
        }
    }

    fn stable_key(&self) -> (u64, &str, &str) {
        match self {
            Self::Command {
                at_us,
                target,
                command,
                ..
            } => (*at_us, target, command),
            Self::Fault {
                at_us,
                target,
                fault,
                ..
            } => (*at_us, target, fault),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    pub schema_version: String,
    pub model_digest: String,
    pub simulation_version: String,
    pub scenario: String,
    pub quantization_contract: String,
    #[serde(default = "default_clock_policy")]
    pub clock_policy: String,
    pub clock_step_us: u64,
    pub seed: u64,
    pub initial_state_digest: String,
    pub event_limit: usize,
    #[serde(default)]
    pub limits: BTreeMap<String, u64>,
    #[serde(default)]
    pub expected_outcomes: Vec<String>,
    pub inputs: Vec<RunInput>,
}

impl RunManifest {
    pub fn normalize(mut self) -> Result<Self, ReplayError> {
        migrate_version(&mut self.schema_version)?;
        if self.model_digest.len() != 64 || self.initial_state_digest.len() != 64 {
            return Err(ReplayError::Invalid(
                "run manifest requires SHA-256 state digests".into(),
            ));
        }
        if self.quantization_contract != QUANTIZATION_CONTRACT_VERSION {
            return Err(ReplayError::Invalid(format!(
                "unsupported quantization contract {}",
                self.quantization_contract
            )));
        }
        if self.clock_step_us == 0 || self.event_limit == 0 {
            return Err(ReplayError::Invalid(
                "run clock and event limit must be non-zero".into(),
            ));
        }
        if self.clock_policy != "fixed-step" {
            return Err(ReplayError::Invalid(format!(
                "unsupported clock policy {}",
                self.clock_policy
            )));
        }
        self.inputs
            .sort_by(|left, right| left.stable_key().cmp(&right.stable_key()));
        Ok(self)
    }

    pub fn digest(&self) -> String {
        digest_json(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayCheckpoint {
    pub event_index: usize,
    pub time_us: u64,
    pub project_digest: String,
    pub component_digests: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_digests: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ProjectSnapshot>,
}

impl ReplayCheckpoint {
    pub fn capture(event_index: usize, snapshot: &ProjectSnapshot) -> Self {
        let mut component_digests = BTreeMap::new();
        let mut field_digests = BTreeMap::new();
        for cell in &snapshot.cells {
            for component in &cell.components {
                let id = format!("{}/{}/{}", cell.site, cell.cell, component.component);
                component_digests.insert(id.clone(), component.digest());
                field_digests.insert(
                    id,
                    component
                        .state
                        .iter()
                        .map(|(field, value)| (field.clone(), digest_json(value)))
                        .collect(),
                );
            }
        }
        Self {
            event_index,
            time_us: snapshot.captured_at_us,
            project_digest: snapshot.digest(),
            component_digests,
            field_digests,
            snapshot: Some(snapshot.clone()),
        }
    }

    fn normalize(mut self) -> Result<Self, ReplayError> {
        if let Some(snapshot) = self.snapshot.take() {
            let snapshot = snapshot.normalize().map_err(ReplayError::Invalid)?;
            let captured = Self::capture(self.event_index, &snapshot);
            if self.time_us != captured.time_us
                || self.project_digest != captured.project_digest
                || self.component_digests != captured.component_digests
                || self.field_digests != captured.field_digests
            {
                return Err(ReplayError::Invalid(format!(
                    "checkpoint {} digest evidence does not match its full snapshot",
                    self.event_index
                )));
            }
            self.snapshot = Some(snapshot);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayOutcome {
    pub status: String,
    pub final_digest: String,
    pub event_count: usize,
    pub alarms: Vec<String>,
    pub metrics: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayArtifact {
    pub schema_version: String,
    pub manifest: RunManifest,
    pub checkpoints: Vec<ReplayCheckpoint>,
    pub outcome: ReplayOutcome,
}

impl ReplayArtifact {
    pub fn normalize(mut self) -> Result<Self, ReplayError> {
        migrate_version(&mut self.schema_version)?;
        self.manifest = self.manifest.normalize()?;
        self.checkpoints = self
            .checkpoints
            .into_iter()
            .map(ReplayCheckpoint::normalize)
            .collect::<Result<Vec<_>, _>>()?;
        self.checkpoints
            .sort_by_key(|checkpoint| (checkpoint.event_index, checkpoint.time_us));
        if self
            .checkpoints
            .windows(2)
            .any(|items| items[0].event_index == items[1].event_index)
        {
            return Err(ReplayError::Invalid(
                "replay repeats a checkpoint index".into(),
            ));
        }
        Ok(self)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, ReplayError> {
        let source = fs::read_to_string(path).map_err(ReplayError::io)?;
        serde_json::from_str::<Self>(&source)
            .map_err(|error| ReplayError::Invalid(error.to_string()))?
            .normalize()
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), ReplayError> {
        let source =
            serde_json::to_string(self).map_err(|error| ReplayError::Invalid(error.to_string()))?;
        fs::write(path, format!("{source}\n")).map_err(ReplayError::io)
    }

    pub fn digest(&self) -> String {
        digest_json(self)
    }
}

pub struct ReplayVerifier;

impl ReplayVerifier {
    pub fn verify(
        expected: &ReplayArtifact,
        actual: &ReplayArtifact,
    ) -> Result<ReplayOutcome, ReplayError> {
        if expected.manifest != actual.manifest {
            return Err(ReplayError::Diverged(ReplayDivergence {
                event_index: 0,
                component: "manifest".into(),
                field: "normalized-run-manifest".into(),
                expected: expected.manifest.digest(),
                actual: actual.manifest.digest(),
            }));
        }
        for (expected_checkpoint, actual_checkpoint) in
            expected.checkpoints.iter().zip(&actual.checkpoints)
        {
            if expected_checkpoint.project_digest != actual_checkpoint.project_digest {
                return Err(checkpoint_divergence(
                    expected_checkpoint,
                    actual_checkpoint,
                ));
            }
            for (component, expected_digest) in &expected_checkpoint.component_digests {
                let actual_digest = actual_checkpoint.component_digests.get(component);
                if actual_digest != Some(expected_digest) {
                    if let Some(divergence) =
                        first_field_divergence(expected_checkpoint, actual_checkpoint, component)
                    {
                        return Err(ReplayError::Diverged(divergence));
                    }
                    return Err(ReplayError::Diverged(ReplayDivergence {
                        event_index: expected_checkpoint.event_index,
                        component: component.clone(),
                        field: "component-state".into(),
                        expected: expected_digest.clone(),
                        actual: actual_digest.cloned().unwrap_or_else(|| "missing".into()),
                    }));
                }
            }
        }
        if expected.checkpoints.len() != actual.checkpoints.len() {
            return Err(ReplayError::Invalid(
                "replay checkpoint count differs".into(),
            ));
        }
        if expected.outcome != actual.outcome {
            return Err(ReplayError::Diverged(ReplayDivergence {
                event_index: actual.outcome.event_count,
                component: "project".into(),
                field: "final-outcome".into(),
                expected: expected.outcome.final_digest.clone(),
                actual: actual.outcome.final_digest.clone(),
            }));
        }
        Ok(actual.outcome.clone())
    }
}

fn first_field_divergence(
    expected: &ReplayCheckpoint,
    actual: &ReplayCheckpoint,
    component: &str,
) -> Option<ReplayDivergence> {
    let expected_fields = expected.field_digests.get(component)?;
    let actual_fields = actual.field_digests.get(component);
    for (field, expected_digest) in expected_fields {
        let actual_digest = actual_fields.and_then(|fields| fields.get(field));
        if actual_digest != Some(expected_digest) {
            return Some(ReplayDivergence {
                event_index: expected.event_index,
                component: component.into(),
                field: field.clone(),
                expected: expected_digest.clone(),
                actual: actual_digest.cloned().unwrap_or_else(|| "missing".into()),
            });
        }
    }
    actual_fields.and_then(|fields| {
        fields
            .keys()
            .find(|field| !expected_fields.contains_key(*field))
            .map(|field| ReplayDivergence {
                event_index: expected.event_index,
                component: component.into(),
                field: field.clone(),
                expected: "missing".into(),
                actual: fields[field].clone(),
            })
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayDivergence {
    pub event_index: usize,
    pub component: String,
    pub field: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug)]
pub enum ReplayError {
    Io(String),
    Invalid(String),
    Diverged(ReplayDivergence),
}

impl ReplayError {
    fn io(error: impl Display) -> Self {
        Self::Io(error.to_string())
    }
}

impl Display for ReplayError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(detail) => write!(formatter, "replay I/O failed: {detail}"),
            Self::Invalid(detail) => write!(formatter, "invalid replay: {detail}"),
            Self::Diverged(item) => write!(
                formatter,
                "replay diverged at event {} component {} field {}: expected {}, actual {}",
                item.event_index, item.component, item.field, item.expected, item.actual
            ),
        }
    }
}

impl std::error::Error for ReplayError {}

fn checkpoint_divergence(expected: &ReplayCheckpoint, actual: &ReplayCheckpoint) -> ReplayError {
    ReplayError::Diverged(ReplayDivergence {
        event_index: expected.event_index,
        component: "project".into(),
        field: "checkpoint".into(),
        expected: expected.project_digest.clone(),
        actual: actual.project_digest.clone(),
    })
}

fn migrate_version(version: &mut String) -> Result<(), ReplayError> {
    if version == PREVIOUS_REPLAY_SCHEMA_VERSION {
        *version = REPLAY_SCHEMA_VERSION.into();
    }
    if version == REPLAY_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ReplayError::Invalid(format!(
            "unsupported replay schema {version}"
        )))
    }
}

fn digest_json(value: &impl Serialize) -> String {
    sha256_hex(&serde_json::to_vec(value).expect("replay serialization"))
}

fn default_clock_policy() -> String {
    "fixed-step".into()
}

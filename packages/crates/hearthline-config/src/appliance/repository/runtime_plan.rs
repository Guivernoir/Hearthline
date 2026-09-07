use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::ConfigError;

pub const RUNTIME_CAPACITY_SCHEMA_VERSION: &str = "1.2.0";
const PREVIOUS_RUNTIME_CAPACITY_SCHEMA_VERSION: &str = "1.1.0";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCapacityManifest {
    pub schema_version: String,
    pub reserve_percent: usize,
    #[serde(default)]
    pub workload: RuntimeWorkloadEnvelope,
    pub partitioning: RuntimePartitioningConfig,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeWorkloadEnvelope {
    pub conduit_queue_capacity: usize,
    pub conduit_reviewed_burst: usize,
    pub immediate_event_burst: usize,
    pub delayed_event_burst: usize,
    pub trace_entry_limit: usize,
    pub session_memory_limit_bytes: usize,
    pub loader_stack_limit_bytes: usize,
    pub conduit_overflow_policy: String,
}

impl RuntimeWorkloadEnvelope {
    fn previous_default() -> Self {
        Self {
            conduit_queue_capacity: 32,
            conduit_reviewed_burst: 24,
            immediate_event_burst: 48,
            delayed_event_burst: 48,
            trace_entry_limit: 168,
            session_memory_limit_bytes: 268_435_456,
            loader_stack_limit_bytes: 2_097_152,
            conduit_overflow_policy: "reject-newest".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePartitioningConfig {
    pub strategy: String,
    #[serde(default)]
    pub rules: Vec<RuntimePartitionRule>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePartitionRule {
    pub id: String,
    pub site: String,
    pub environment: String,
    #[serde(default)]
    pub appliance_ids: Vec<String>,
    #[serde(default)]
    pub appliance_prefixes: Vec<String>,
}

impl RuntimeCapacityManifest {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(|error| {
            ConfigError::new(format!("cannot read {}: {error}", path.display()))
        })?;
        let mut manifest: Self = serde_yaml_ng::from_str(&source).map_err(|error| {
            ConfigError::new(format!("invalid runtime capacity manifest: {error}"))
        })?;
        if manifest.schema_version == PREVIOUS_RUNTIME_CAPACITY_SCHEMA_VERSION {
            manifest.schema_version = RUNTIME_CAPACITY_SCHEMA_VERSION.into();
            manifest.workload = RuntimeWorkloadEnvelope::previous_default();
        }
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != RUNTIME_CAPACITY_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "runtime capacity manifest uses schema {}, expected {}",
                self.schema_version, RUNTIME_CAPACITY_SCHEMA_VERSION
            )));
        }
        if self.reserve_percent < 25 || self.reserve_percent >= 100 {
            return Err(ConfigError::new(
                "runtime capacity reserve_percent must be between 25 and 99",
            ));
        }
        let workload = &self.workload;
        if workload.conduit_queue_capacity == 0
            || workload.conduit_reviewed_burst == 0
            || workload.immediate_event_burst == 0
            || workload.delayed_event_burst == 0
            || workload.trace_entry_limit == 0
            || workload.session_memory_limit_bytes == 0
            || workload.loader_stack_limit_bytes == 0
        {
            return Err(ConfigError::new(
                "runtime workload evidence requires non-zero queue, event, memory, and stack limits",
            ));
        }
        if workload.conduit_reviewed_burst.saturating_mul(4)
            > workload.conduit_queue_capacity.saturating_mul(3)
        {
            return Err(ConfigError::new(
                "runtime conduit workload does not preserve 25% reserve",
            ));
        }
        if !matches!(
            workload.conduit_overflow_policy.as_str(),
            "reject-newest" | "drop-oldest" | "coalesce-latest" | "stop-simulation"
        ) {
            return Err(ConfigError::new(
                "runtime conduit workload requires an explicit supported overflow policy",
            ));
        }
        if self.partitioning.strategy != "site-environment-with-rules" {
            return Err(ConfigError::new(format!(
                "unsupported runtime partitioning strategy {}",
                self.partitioning.strategy
            )));
        }
        let mut rule_ids = BTreeSet::new();
        for rule in &self.partitioning.rules {
            if rule.id.trim().is_empty()
                || rule.site.trim().is_empty()
                || rule.environment.trim().is_empty()
            {
                return Err(ConfigError::new(
                    "runtime partition rules require id, site, and environment",
                ));
            }
            if rule.appliance_ids.is_empty() && rule.appliance_prefixes.is_empty() {
                return Err(ConfigError::new(format!(
                    "runtime partition rule {} has no appliance selectors",
                    rule.id
                )));
            }
            if !rule_ids.insert(rule.id.as_str()) {
                return Err(ConfigError::new(format!(
                    "runtime partition rule {} is duplicated",
                    rule.id
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn project_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config")
    }

    fn manifest() -> RuntimeCapacityManifest {
        serde_yaml_ng::from_str(
            &fs::read_to_string(project_config().join("runtime/capacity.yaml"))
                .expect("canonical capacity manifest"),
        )
        .expect("capacity YAML")
    }

    fn expect_invalid(
        mut manifest: RuntimeCapacityManifest,
        mutate: impl FnOnce(&mut RuntimeCapacityManifest),
        expected: &str,
    ) {
        mutate(&mut manifest);
        let error = manifest.validate().expect_err("invalid capacity manifest");
        assert!(error.to_string().contains(expected), "{error}");
    }

    #[test]
    fn capacity_manifest_rejects_every_missing_workload_evidence_field() {
        let canonical = manifest();
        for field in 0..7 {
            expect_invalid(
                canonical.clone(),
                |manifest| match field {
                    0 => manifest.workload.conduit_queue_capacity = 0,
                    1 => manifest.workload.conduit_reviewed_burst = 0,
                    2 => manifest.workload.immediate_event_burst = 0,
                    3 => manifest.workload.delayed_event_burst = 0,
                    4 => manifest.workload.trace_entry_limit = 0,
                    5 => manifest.workload.session_memory_limit_bytes = 0,
                    6 => manifest.workload.loader_stack_limit_bytes = 0,
                    _ => unreachable!(),
                },
                "requires non-zero",
            );
        }
        expect_invalid(
            canonical.clone(),
            |manifest| manifest.workload.conduit_reviewed_burst = 25,
            "does not preserve 25% reserve",
        );
        expect_invalid(
            canonical,
            |manifest| manifest.workload.conduit_overflow_policy = "unbounded".into(),
            "explicit supported overflow policy",
        );
    }

    #[test]
    fn capacity_manifest_enforces_schema_reserve_strategy_and_rule_contracts() {
        let canonical = manifest();
        expect_invalid(
            canonical.clone(),
            |manifest| manifest.schema_version = "9.9.9".into(),
            "uses schema",
        );
        for reserve in [24, 100] {
            expect_invalid(
                canonical.clone(),
                |manifest| manifest.reserve_percent = reserve,
                "between 25 and 99",
            );
        }
        expect_invalid(
            canonical.clone(),
            |manifest| manifest.partitioning.strategy = "global-fixed-table".into(),
            "unsupported runtime partitioning strategy",
        );
        for field in 0..3 {
            expect_invalid(
                canonical.clone(),
                |manifest| match field {
                    0 => manifest.partitioning.rules[0].id = " ".into(),
                    1 => manifest.partitioning.rules[0].site = " ".into(),
                    2 => manifest.partitioning.rules[0].environment = " ".into(),
                    _ => unreachable!(),
                },
                "require id, site, and environment",
            );
        }
        expect_invalid(
            canonical.clone(),
            |manifest| {
                manifest.partitioning.rules[0].appliance_ids.clear();
                manifest.partitioning.rules[0].appliance_prefixes.clear();
            },
            "has no appliance selectors",
        );
        expect_invalid(
            canonical.clone(),
            |manifest| {
                let duplicate = manifest.partitioning.rules[0].clone();
                manifest.partitioning.rules.push(duplicate);
            },
            "is duplicated",
        );
        for policy in [
            "reject-newest",
            "drop-oldest",
            "coalesce-latest",
            "stop-simulation",
        ] {
            let mut valid = canonical.clone();
            valid.workload.conduit_overflow_policy = policy.into();
            valid.validate().expect("supported overflow policy");
        }
    }

    #[test]
    fn previous_capacity_schema_migrates_with_reviewed_workload_defaults() {
        let mut document: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            &fs::read_to_string(project_config().join("runtime/capacity.yaml")).unwrap(),
        )
        .unwrap();
        document["schema_version"] = serde_yaml_ng::Value::String("1.1.0".into());
        document
            .as_mapping_mut()
            .unwrap()
            .remove(serde_yaml_ng::Value::String("workload".into()));
        let path = std::env::temp_dir().join(format!(
            "hearthline-runtime-capacity-{}.yaml",
            std::process::id()
        ));
        fs::write(&path, serde_yaml_ng::to_string(&document).unwrap()).unwrap();
        let migrated = RuntimeCapacityManifest::load(&path).expect("previous schema migration");
        fs::remove_file(path).unwrap();
        assert_eq!(migrated.schema_version, RUNTIME_CAPACITY_SCHEMA_VERSION);
        assert_eq!(migrated.workload.conduit_queue_capacity, 32);
        assert_eq!(migrated.workload.conduit_overflow_policy, "reject-newest");
    }
}

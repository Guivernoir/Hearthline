use std::collections::{BTreeMap, BTreeSet};

use hearthline_engine::{FormingPhase, SlipPhase};
use serde::{Deserialize, Serialize};

use crate::ConfigError;

pub(super) const CONTROL_BINDING_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ControlLanguage {
    StructuredText,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ControlDataType {
    Bool,
    Int,
    Dint,
    Real,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum ControlInputSource {
    Signal { tag: String },
    StartRequest,
    SafetyReady,
    ResetRequest,
    TripActive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlInputBinding {
    pub variable: String,
    pub data_type: ControlDataType,
    pub source: ControlInputSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlOutputBinding {
    pub variable: String,
    pub command_tag: String,
    pub states: BTreeMap<i64, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlPhaseBinding {
    pub variable: String,
    pub values: BTreeMap<i64, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlTaskConfig {
    pub name: String,
    pub interval_ms: u64,
    pub watchdog_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlSequenceConfig {
    pub step_variable: String,
    pub timer_variable: String,
    pub idle_step: i64,
    pub start_step: i64,
    pub fault_step: i64,
    pub start_input: String,
    pub safety_input: String,
    pub reset_input: String,
    pub trip_input: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlBindingConfig {
    pub schema_version: String,
    pub controller: String,
    pub language: ControlLanguage,
    pub program: String,
    pub task: ControlTaskConfig,
    pub sequence: ControlSequenceConfig,
    pub phase: ControlPhaseBinding,
    pub inputs: Vec<ControlInputBinding>,
    pub outputs: Vec<ControlOutputBinding>,
}

impl ControlBindingConfig {
    pub(super) fn from_yaml(source: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ConfigError::new(format!("invalid control binding YAML: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != CONTROL_BINDING_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "control binding schema {} is unsupported; expected {}",
                self.schema_version, CONTROL_BINDING_SCHEMA_VERSION
            )));
        }
        for (field, value) in [
            ("controller", self.controller.as_str()),
            ("program", self.program.as_str()),
            ("task name", self.task.name.as_str()),
            ("step variable", self.sequence.step_variable.as_str()),
            ("timer variable", self.sequence.timer_variable.as_str()),
            ("phase variable", self.phase.variable.as_str()),
        ] {
            require_name(field, value)?;
        }
        if self.task.interval_ms == 0 || self.task.watchdog_ms < self.task.interval_ms {
            return Err(ConfigError::new(
                "control task requires a positive interval and watchdog not shorter than one scan",
            ));
        }
        if self.inputs.is_empty() || self.outputs.is_empty() || self.phase.values.is_empty() {
            return Err(ConfigError::new(
                "control binding requires inputs, actuator outputs, and phase values",
            ));
        }
        let mut variables = BTreeSet::new();
        let mut signal_tags = BTreeSet::new();
        let mut source_roles = BTreeSet::new();
        for input in &self.inputs {
            require_name("control input variable", &input.variable)?;
            require_unique_name(&mut variables, &input.variable, "control variable")?;
            match &input.source {
                ControlInputSource::Signal { tag } => {
                    require_name("control signal tag", tag)?;
                    if !signal_tags.insert(tag.to_ascii_lowercase()) {
                        return Err(ConfigError::new(format!(
                            "control binding repeats signal source {tag}"
                        )));
                    }
                }
                source => {
                    let role = source.role();
                    if !source_roles.insert(role) {
                        return Err(ConfigError::new(format!(
                            "control binding repeats {role} input source"
                        )));
                    }
                    if input.data_type != ControlDataType::Bool {
                        return Err(ConfigError::new(format!(
                            "control input {} for {role} must use BOOL",
                            input.variable
                        )));
                    }
                }
            }
        }
        for required in [
            "start-request",
            "safety-ready",
            "reset-request",
            "trip-active",
        ] {
            if !source_roles.contains(required) {
                return Err(ConfigError::new(format!(
                    "control binding requires one {required} input source"
                )));
            }
        }
        for (role, configured) in [
            ("start-request", self.sequence.start_input.as_str()),
            ("safety-ready", self.sequence.safety_input.as_str()),
            ("reset-request", self.sequence.reset_input.as_str()),
            ("trip-active", self.sequence.trip_input.as_str()),
        ] {
            if self
                .source_variable(role)
                .is_none_or(|variable| !variable.eq_ignore_ascii_case(configured))
            {
                return Err(ConfigError::new(format!(
                    "control sequence {role} variable {configured} does not match its input binding"
                )));
            }
        }
        require_unique_name(&mut variables, &self.phase.variable, "control variable")?;
        let mut command_tags = BTreeSet::new();
        for output in &self.outputs {
            require_name("control output variable", &output.variable)?;
            require_name("control output command tag", &output.command_tag)?;
            require_unique_name(&mut variables, &output.variable, "control variable")?;
            if !command_tags.insert(output.command_tag.to_ascii_lowercase()) {
                return Err(ConfigError::new(format!(
                    "control binding repeats command tag {}",
                    output.command_tag
                )));
            }
            if output.states.is_empty()
                || output.states.values().any(|state| state.trim().is_empty())
            {
                return Err(ConfigError::new(format!(
                    "control output {} requires non-empty state mappings",
                    output.variable
                )));
            }
        }
        for phase in self.phase.values.values() {
            validate_process_phase(phase)?;
        }
        Ok(())
    }

    pub(super) fn source_variable(&self, role: &str) -> Option<&str> {
        self.inputs
            .iter()
            .find_map(|input| (input.source.role() == role).then_some(input.variable.as_str()))
    }
}

fn validate_process_phase(value: &str) -> Result<(), ConfigError> {
    if forming_phase(value).is_ok()
        || matches!(
            value,
            "water-charge"
                | "deflocculant-charge"
                | "ball-clay-dose"
                | "kaolin-dose"
                | "feldspar-dose"
                | "quartz-dose"
                | "wet-mixing"
                | "screening"
                | "magnetic-separation"
                | "conditioning"
                | "quality-check"
                | "heating-to-transfer-temperature"
                | "transfer-to-forming"
                | "batch-complete"
        )
    {
        Ok(())
    } else {
        Err(ConfigError::new(format!(
            "unknown process phase mapping {value}"
        )))
    }
}

impl ControlInputSource {
    pub(super) const fn role(&self) -> &'static str {
        match self {
            Self::Signal { .. } => "signal",
            Self::StartRequest => "start-request",
            Self::SafetyReady => "safety-ready",
            Self::ResetRequest => "reset-request",
            Self::TripActive => "trip-active",
        }
    }
}

pub(super) fn forming_phase(value: &str) -> Result<FormingPhase, ConfigError> {
    match value {
        "idle" => Ok(FormingPhase::Idle),
        "mould-filling" => Ok(FormingPhase::Filling),
        "air-pressurizing" => Ok(FormingPhase::Pressurizing),
        "pressure-dwell" => Ok(FormingPhase::PressureDwell),
        "excess-slip-drain" => Ok(FormingPhase::Draining),
        "depressurizing" => Ok(FormingPhase::Depressurizing),
        "release-water" => Ok(FormingPhase::ReleaseWater),
        "release-air" => Ok(FormingPhase::ReleaseAir),
        "mould-opening" => Ok(FormingPhase::OpeningMould),
        "robot-pickup" => Ok(FormingPhase::RobotPickup),
        "operator-delivery" => Ok(FormingPhase::RobotDelivery),
        "mould-wash" => Ok(FormingPhase::MouldWash),
        "cleaning-air-purge" => Ok(FormingPhase::AirPurge),
        "vacuum-dry" => Ok(FormingPhase::VacuumDry),
        "mould-closing" => Ok(FormingPhase::ClosingMould),
        "faulted" => Ok(FormingPhase::Faulted),
        _ => Err(ConfigError::new(format!(
            "unknown Forming phase mapping {value}"
        ))),
    }
}

pub(super) fn body_preparation_phase(value: &str) -> Result<SlipPhase, ConfigError> {
    match value {
        "idle" => Ok(SlipPhase::Idle),
        "water-charge" => Ok(SlipPhase::WaterCharge),
        "deflocculant-charge" => Ok(SlipPhase::DeflocculantCharge),
        "ball-clay-dose" => Ok(SlipPhase::BallClayCharge),
        "kaolin-dose" => Ok(SlipPhase::KaolinCharge),
        "feldspar-dose" => Ok(SlipPhase::FeldsparCharge),
        "quartz-dose" => Ok(SlipPhase::QuartzCharge),
        "wet-mixing" => Ok(SlipPhase::WetMixing),
        "screening" => Ok(SlipPhase::Screening),
        "magnetic-separation" => Ok(SlipPhase::MagneticSeparation),
        "conditioning" => Ok(SlipPhase::Conditioning),
        "quality-check" => Ok(SlipPhase::QualityCheck),
        "heating-to-transfer-temperature" => Ok(SlipPhase::TemperatureTrim),
        "transfer-to-forming" => Ok(SlipPhase::Transfer),
        "batch-complete" => Ok(SlipPhase::Complete),
        "faulted" => Ok(SlipPhase::Faulted),
        _ => Err(ConfigError::new(format!(
            "unknown Body Preparation phase mapping {value}"
        ))),
    }
}

fn require_name(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() || value.len() > 64 {
        return Err(ConfigError::new(format!(
            "{field} must contain 1 to 64 bytes"
        )));
    }
    Ok(())
}

fn require_unique_name(
    values: &mut BTreeSet<String>,
    value: &str,
    field: &str,
) -> Result<(), ConfigError> {
    let normalized = value.to_ascii_lowercase();
    if !values.insert(normalized) {
        return Err(ConfigError::new(format!("{field} {value} is repeated")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn fixture() -> ControlBindingConfig {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../project/control/bindings/body-preparation/area-01-vplc-01.yaml");
        serde_yaml_ng::from_str(&fs::read_to_string(path).expect("canonical binding"))
            .expect("binding YAML")
    }

    fn reject(
        mut binding: ControlBindingConfig,
        mutate: impl FnOnce(&mut ControlBindingConfig),
        expected: &str,
    ) {
        mutate(&mut binding);
        let error = binding.validate().expect_err("invalid binding must fail");
        assert!(error.to_string().contains(expected), "{error}");
    }

    #[test]
    fn binding_rejects_schema_task_collection_and_name_boundaries() {
        let canonical = fixture();
        reject(
            canonical.clone(),
            |binding| binding.schema_version = "9.9.9".into(),
            "schema",
        );
        reject(
            canonical.clone(),
            |binding| binding.task.interval_ms = 0,
            "positive interval",
        );
        reject(
            canonical.clone(),
            |binding| binding.task.watchdog_ms = binding.task.interval_ms - 1,
            "watchdog",
        );
        reject(
            canonical.clone(),
            |binding| binding.inputs.clear(),
            "requires inputs",
        );
        reject(
            canonical.clone(),
            |binding| binding.outputs.clear(),
            "requires inputs",
        );
        reject(
            canonical.clone(),
            |binding| binding.phase.values.clear(),
            "requires inputs",
        );
        reject(
            canonical.clone(),
            |binding| binding.controller = " ".into(),
            "1 to 64 bytes",
        );
        reject(
            canonical,
            |binding| binding.program = "x".repeat(65),
            "1 to 64 bytes",
        );
    }

    #[test]
    fn binding_rejects_duplicate_missing_and_mismatched_input_contracts() {
        let canonical = fixture();
        let signal = canonical
            .inputs
            .iter()
            .find(|input| matches!(input.source, ControlInputSource::Signal { .. }))
            .expect("signal input")
            .clone();
        reject(
            canonical.clone(),
            |binding| {
                let mut duplicate = signal.clone();
                duplicate.variable = "UniqueSignalVariable".into();
                binding.inputs.push(duplicate);
            },
            "repeats signal source",
        );
        let start = canonical
            .inputs
            .iter()
            .find(|input| matches!(input.source, ControlInputSource::StartRequest))
            .expect("start input")
            .clone();
        reject(
            canonical.clone(),
            |binding| {
                let mut duplicate = start.clone();
                duplicate.variable = "UniqueStartVariable".into();
                binding.inputs.push(duplicate);
            },
            "repeats start-request input source",
        );
        reject(
            canonical.clone(),
            |binding| {
                binding
                    .inputs
                    .iter_mut()
                    .find(|input| matches!(input.source, ControlInputSource::StartRequest))
                    .unwrap()
                    .data_type = ControlDataType::Int;
            },
            "must use BOOL",
        );
        for role in [
            "start-request",
            "safety-ready",
            "reset-request",
            "trip-active",
        ] {
            reject(
                canonical.clone(),
                |binding| binding.inputs.retain(|input| input.source.role() != role),
                "requires one",
            );
        }
        reject(
            canonical.clone(),
            |binding| binding.sequence.start_input = "WrongStartInput".into(),
            "does not match its input binding",
        );
        reject(
            canonical,
            |binding| binding.outputs[0].variable = binding.inputs[0].variable.clone(),
            "control variable",
        );
    }

    #[test]
    fn binding_rejects_duplicate_commands_empty_states_and_unknown_phases() {
        let canonical = fixture();
        reject(
            canonical.clone(),
            |binding| {
                let mut duplicate = binding.outputs[0].clone();
                duplicate.variable = "UniqueOutputVariable".into();
                binding.outputs.push(duplicate);
            },
            "repeats command tag",
        );
        reject(
            canonical.clone(),
            |binding| binding.outputs[0].states.clear(),
            "non-empty state mappings",
        );
        reject(
            canonical.clone(),
            |binding| {
                *binding.outputs[0].states.values_mut().next().unwrap() = " ".into();
            },
            "non-empty state mappings",
        );
        reject(
            canonical,
            |binding| {
                *binding.phase.values.values_mut().next().unwrap() = "unknown-phase".into();
            },
            "unknown process phase mapping",
        );
    }

    #[test]
    fn phase_and_input_role_mappings_cover_all_supported_values() {
        for phase in [
            "idle",
            "mould-filling",
            "air-pressurizing",
            "pressure-dwell",
            "excess-slip-drain",
            "depressurizing",
            "release-water",
            "release-air",
            "mould-opening",
            "robot-pickup",
            "operator-delivery",
            "mould-wash",
            "cleaning-air-purge",
            "vacuum-dry",
            "mould-closing",
            "faulted",
        ] {
            forming_phase(phase).expect("supported Forming phase");
        }
        for phase in [
            "idle",
            "water-charge",
            "deflocculant-charge",
            "ball-clay-dose",
            "kaolin-dose",
            "feldspar-dose",
            "quartz-dose",
            "wet-mixing",
            "screening",
            "magnetic-separation",
            "conditioning",
            "quality-check",
            "heating-to-transfer-temperature",
            "transfer-to-forming",
            "batch-complete",
            "faulted",
        ] {
            body_preparation_phase(phase).expect("supported Body Preparation phase");
        }
        assert!(forming_phase("unknown").is_err());
        assert!(body_preparation_phase("unknown").is_err());
        for source in [
            ControlInputSource::Signal { tag: "tag".into() },
            ControlInputSource::StartRequest,
            ControlInputSource::SafetyReady,
            ControlInputSource::ResetRequest,
            ControlInputSource::TripActive,
        ] {
            assert!(!source.role().is_empty());
        }
    }
}

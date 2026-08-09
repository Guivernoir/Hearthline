use std::collections::{BTreeMap, BTreeSet};

use hearthline_engine::FormingPhase;
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
            forming_phase(phase)?;
        }
        Ok(())
    }

    pub(super) fn source_variable(&self, role: &str) -> Option<&str> {
        self.inputs
            .iter()
            .find_map(|input| (input.source.role() == role).then_some(input.variable.as_str()))
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

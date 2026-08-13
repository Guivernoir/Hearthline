use hearthline_engine::{FormingPhase, SequenceInputs, SequenceRuntime, SequenceScan};

use crate::appliance::source_revision;
use crate::{BehaviorConfig, ConfigError, ConfigRepository};

use super::binding::{ControlBindingConfig, ControlInputSource, forming_phase};
use super::compiler::compile_program;
use super::parser;

#[derive(Clone, Debug)]
pub(crate) struct ConfiguredControlProgram {
    runtime: SequenceRuntime,
    binding: ControlBindingConfig,
    source_path: String,
    binding_path: String,
    source: String,
    binding_yaml: String,
    revision: String,
}

impl ConfiguredControlProgram {
    pub(crate) fn phase(&self) -> FormingPhase {
        let code = self
            .runtime
            .current_assignment(&self.binding.phase.variable)
            .expect("compiled sequence has a phase assignment");
        let name = self
            .binding
            .phase
            .values
            .get(&code)
            .expect("compiled phase code is mapped");
        forming_phase(name).expect("compiled phase name is valid")
    }

    pub(crate) const fn runtime(&self) -> &SequenceRuntime {
        &self.runtime
    }

    pub(crate) fn program_name(&self) -> &str {
        self.runtime.program().name.as_str()
    }

    pub(crate) fn execute_scan(&mut self, inputs: SequenceInputs) -> SequenceScan {
        self.runtime.execute_scan(inputs)
    }

    pub(crate) fn elapse_with_timer_override(
        &mut self,
        elapsed_ms: u64,
        inputs: SequenceInputs,
        timer_override_ms: Option<u64>,
    ) -> Option<SequenceScan> {
        self.runtime
            .elapse_with_timer_override(elapsed_ms, inputs, timer_override_ms)
    }

    pub(crate) fn force_fault(&mut self) -> SequenceScan {
        self.runtime.force_fault()
    }

    pub(crate) fn source_path(&self) -> &str {
        &self.source_path
    }

    pub(crate) fn binding_path(&self) -> &str {
        &self.binding_path
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn binding_yaml(&self) -> &str {
        &self.binding_yaml
    }

    pub(crate) fn revision(&self) -> &str {
        &self.revision
    }

    pub(crate) fn task_name(&self) -> &str {
        &self.binding.task.name
    }

    pub(crate) const fn watchdog_ms(&self) -> u64 {
        self.binding.task.watchdog_ms
    }
}

pub(crate) fn load_control_program(
    appliances: &ConfigRepository,
    controller_id: &str,
) -> Result<Option<ConfiguredControlProgram>, ConfigError> {
    let controller = appliances
        .get(controller_id)
        .ok_or_else(|| ConfigError::new(format!("unknown controller {controller_id}")))?;
    let BehaviorConfig::VirtualController {
        scan_interval_ms,
        program_ref,
        io_binding,
    } = &controller.config.behavior
    else {
        return Err(ConfigError::new(format!(
            "appliance {controller_id} is not a virtual controller"
        )));
    };
    let executable_source = program_ref.starts_with("control/");
    let executable_binding = io_binding.starts_with("control/");
    if !executable_source && !executable_binding {
        return Ok(None);
    }
    if executable_source != executable_binding {
        return Err(ConfigError::new(format!(
            "controller {controller_id} must reference both executable control source and binding"
        )));
    }

    let (_, source) = appliances.read_project_source(program_ref)?;
    let (_, binding_yaml) = appliances.read_project_source(io_binding)?;
    let binding = ControlBindingConfig::from_yaml(&binding_yaml)?;
    if binding.controller != controller_id {
        return Err(ConfigError::new(format!(
            "control binding names controller {}, expected {controller_id}",
            binding.controller
        )));
    }
    if binding.task.interval_ms != *scan_interval_ms {
        return Err(ConfigError::new(format!(
            "control task interval {} ms does not match {controller_id} scan interval {scan_interval_ms} ms",
            binding.task.interval_ms
        )));
    }
    validate_repository_bindings(appliances, controller_id, &binding)?;
    let parsed = parser::parse(&source)?;
    let runtime = SequenceRuntime::new(compile_program(&binding, &parsed)?);
    let revision = source_revision(&format!("{source}\n---\n{binding_yaml}"));
    Ok(Some(ConfiguredControlProgram {
        runtime,
        binding,
        source_path: program_ref.clone(),
        binding_path: io_binding.clone(),
        source,
        binding_yaml,
        revision,
    }))
}

fn validate_repository_bindings(
    appliances: &ConfigRepository,
    controller_id: &str,
    binding: &ControlBindingConfig,
) -> Result<(), ConfigError> {
    let controller = appliances
        .get(controller_id)
        .expect("controller was loaded");
    for input in &binding.inputs {
        let ControlInputSource::Signal { tag } = &input.source else {
            continue;
        };
        let found = appliances.appliances().any(|candidate| {
            candidate.config.environment == controller.config.environment
                && candidate.config.zone == controller.config.zone
                && matches!(
                    &candidate.config.behavior,
                    BehaviorConfig::FieldSensor { signal_tag, .. } if signal_tag == tag
                )
        });
        if !found {
            return Err(ConfigError::new(format!(
                "control input {} references unknown area signal {tag}",
                input.variable
            )));
        }
    }
    for output in &binding.outputs {
        let actuator = appliances.appliances().find(|candidate| {
            candidate.config.environment == controller.config.environment
                && candidate.config.zone == controller.config.zone
                && matches!(
                    &candidate.config.behavior,
                    BehaviorConfig::FieldActuator { command_tag, .. }
                        if command_tag == &output.command_tag
                )
        });
        let Some(actuator) = actuator else {
            return Err(ConfigError::new(format!(
                "control output {} references unknown area command tag {}",
                output.variable, output.command_tag
            )));
        };
        let BehaviorConfig::FieldActuator { states, .. } = &actuator.config.behavior else {
            unreachable!("actuator selection checks behavior")
        };
        if let Some(state) = output
            .states
            .values()
            .find(|state| !states.contains(*state))
        {
            return Err(ConfigError::new(format!(
                "control output {} maps unsupported actuator state {state}",
                output.variable
            )));
        }
    }
    Ok(())
}

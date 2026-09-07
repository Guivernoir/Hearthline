use std::collections::{BTreeMap, BTreeSet};

use hearthline_engine::{
    SequenceAssignment, SequenceCondition, SequenceProgram, SequenceStep, SequenceTransition,
};
use hearthline_model::Text;

use crate::ConfigError;

use super::binding::{ControlBindingConfig, ControlDataType};
use super::parser::{Branch, ConditionTerm, DataType, Literal, Program, Variable, VariableSection};

pub(super) fn compile_program(
    binding: &ControlBindingConfig,
    program: &Program,
) -> Result<SequenceProgram, ConfigError> {
    if !program.name.eq_ignore_ascii_case(&binding.program) {
        return Err(ConfigError::new(format!(
            "Structured Text PROGRAM {} does not match binding program {}",
            program.name, binding.program
        )));
    }
    let variables = variable_index(program)?;
    validate_declarations(binding, program, &variables)?;
    let branch_numbers: BTreeSet<i64> = program
        .branches
        .iter()
        .map(|branch| branch.number)
        .collect();
    if branch_numbers.len() != program.branches.len() {
        return Err(ConfigError::new(
            "Structured Text CASE repeats a branch number",
        ));
    }
    let steps = program
        .branches
        .iter()
        .map(|branch| compile_branch(binding, branch, &branch_numbers))
        .collect::<Result<Vec<_>, _>>()?;
    SequenceProgram::new(
        Text::from(program.name.as_str()),
        binding.task.interval_ms,
        binding.sequence.idle_step,
        binding.sequence.fault_step,
        steps,
    )
    .ok_or_else(|| {
        ConfigError::new(
            "Structured Text sequence exceeds runtime bounds or has an invalid transition",
        )
    })
}

fn variable_index(program: &Program) -> Result<BTreeMap<String, &Variable>, ConfigError> {
    let mut variables = BTreeMap::new();
    for variable in &program.variables {
        let key = variable.name.to_ascii_lowercase();
        if variables.insert(key, variable).is_some() {
            return Err(ConfigError::new(format!(
                "Structured Text variable {} is declared more than once",
                variable.name
            )));
        }
    }
    Ok(variables)
}

fn validate_declarations(
    binding: &ControlBindingConfig,
    program: &Program,
    variables: &BTreeMap<String, &Variable>,
) -> Result<(), ConfigError> {
    for input in &binding.inputs {
        require_variable(
            variables,
            &input.variable,
            VariableSection::Input,
            Some(binding_type(input.data_type)),
        )?;
    }
    require_integer_output(variables, &binding.phase.variable)?;
    for output in &binding.outputs {
        require_integer_output(variables, &output.variable)?;
    }
    let step = require_variable(
        variables,
        &binding.sequence.step_variable,
        VariableSection::Local,
        None,
    )?;
    if !matches!(step.data_type, DataType::Int | DataType::Dint)
        || step.initial != Some(Literal::Integer(binding.sequence.idle_step))
    {
        return Err(ConfigError::new(format!(
            "step variable {} must be an initialized INT or DINT at idle step {}",
            step.name, binding.sequence.idle_step
        )));
    }
    require_variable(
        variables,
        &binding.sequence.timer_variable,
        VariableSection::Local,
        Some(DataType::Ton),
    )?;
    if !program
        .case_variable
        .eq_ignore_ascii_case(&binding.sequence.step_variable)
    {
        return Err(ConfigError::new(format!(
            "Structured Text CASE must use step variable {}",
            binding.sequence.step_variable
        )));
    }
    Ok(())
}

fn compile_branch(
    binding: &ControlBindingConfig,
    branch: &Branch,
    branch_numbers: &BTreeSet<i64>,
) -> Result<SequenceStep, ConfigError> {
    let mut expected: BTreeSet<String> = binding
        .outputs
        .iter()
        .map(|output| output.variable.to_ascii_lowercase())
        .collect();
    expected.insert(binding.phase.variable.to_ascii_lowercase());
    let mut seen = BTreeSet::new();
    let mut assignments = Vec::with_capacity(branch.assignments.len());
    for assignment in &branch.assignments {
        let variable = assignment.variable.to_ascii_lowercase();
        if !expected.contains(&variable) || !seen.insert(variable) {
            return Err(ConfigError::new(format!(
                "CASE branch {} has an unexpected or repeated assignment to {}",
                branch.number, assignment.variable
            )));
        }
        let Literal::Integer(value) = assignment.value else {
            return Err(ConfigError::new(format!(
                "CASE branch {} output {} must use an integer state code",
                branch.number, assignment.variable
            )));
        };
        validate_assignment_value(binding, &assignment.variable, value)?;
        assignments.push(SequenceAssignment {
            variable: Text::from(assignment.variable.as_str()),
            value,
        });
    }
    if seen != expected {
        return Err(ConfigError::new(format!(
            "CASE branch {} must assign the phase and every configured actuator output",
            branch.number
        )));
    }
    let transition = compile_transition(binding, branch)?;
    if !branch_numbers.contains(&transition.target) {
        return Err(ConfigError::new(format!(
            "CASE branch {} targets missing step {}",
            branch.number, transition.target
        )));
    }
    SequenceStep::new(branch.number, assignments, Some(transition)).ok_or_else(|| {
        ConfigError::new(format!(
            "CASE branch {} exceeds runtime bounds",
            branch.number
        ))
    })
}

fn compile_transition(
    binding: &ControlBindingConfig,
    branch: &Branch,
) -> Result<SequenceTransition, ConfigError> {
    let transition = branch.transition.as_ref().ok_or_else(|| {
        ConfigError::new(format!(
            "CASE branch {} requires one transition",
            branch.number
        ))
    })?;
    if !transition
        .assignment
        .variable
        .eq_ignore_ascii_case(&binding.sequence.step_variable)
    {
        return Err(ConfigError::new(format!(
            "CASE branch {} transition must assign {}",
            branch.number, binding.sequence.step_variable
        )));
    }
    let Literal::Integer(target) = transition.assignment.value else {
        return Err(ConfigError::new(format!(
            "CASE branch {} transition target must be an integer step",
            branch.number
        )));
    };
    let condition = if branch.number == binding.sequence.idle_step {
        compile_idle_transition(binding, branch, target)?
    } else if branch.number == binding.sequence.fault_step {
        compile_fault_transition(binding, branch, target)?
    } else {
        compile_timer_transition(binding, branch)?
    };
    Ok(SequenceTransition { condition, target })
}

fn compile_idle_transition(
    binding: &ControlBindingConfig,
    branch: &Branch,
    target: i64,
) -> Result<SequenceCondition, ConfigError> {
    let transition = branch.transition.as_ref().expect("transition checked");
    let expected = [
        (binding.sequence.start_input.as_str(), false),
        (binding.sequence.safety_input.as_str(), false),
    ];
    if branch.timer.is_some()
        || !condition_matches(&transition.condition, expected)
        || target != binding.sequence.start_step
    {
        return Err(ConfigError::new(
            "idle branch must start at the configured step when start and safety inputs are true",
        ));
    }
    Ok(SequenceCondition::StartPermitted)
}

fn compile_fault_transition(
    binding: &ControlBindingConfig,
    branch: &Branch,
    target: i64,
) -> Result<SequenceCondition, ConfigError> {
    let transition = branch.transition.as_ref().expect("transition checked");
    let expected = [
        (binding.sequence.reset_input.as_str(), false),
        (binding.sequence.safety_input.as_str(), false),
        (binding.sequence.trip_input.as_str(), true),
    ];
    if branch.timer.is_some()
        || !condition_matches(&transition.condition, expected)
        || target != binding.sequence.idle_step
    {
        return Err(ConfigError::new(
            "fault branch must reset to idle when reset and safety are true and trip is false",
        ));
    }
    Ok(SequenceCondition::ResetPermitted)
}

fn compile_timer_transition(
    binding: &ControlBindingConfig,
    branch: &Branch,
) -> Result<SequenceCondition, ConfigError> {
    let timer = branch.timer.as_ref().ok_or_else(|| {
        ConfigError::new(format!("CASE branch {} requires a TON call", branch.number))
    })?;
    let transition = branch.transition.as_ref().expect("transition checked");
    let done = format!("{}.Q", binding.sequence.timer_variable);
    if !timer
        .variable
        .eq_ignore_ascii_case(&binding.sequence.timer_variable)
        || !condition_matches(&timer.input, [("TRUE", false)])
        || !condition_matches(&transition.condition, [(done.as_str(), false)])
    {
        return Err(ConfigError::new(format!(
            "CASE branch {} must transition from the configured TON done bit",
            branch.number
        )));
    }
    Ok(SequenceCondition::TimerElapsed {
        duration_ms: timer.duration_ms,
    })
}

fn condition_matches<'a>(
    actual: &[ConditionTerm],
    expected: impl IntoIterator<Item = (&'a str, bool)>,
) -> bool {
    let actual: BTreeSet<(String, bool)> = actual
        .iter()
        .map(|term| (term.variable.to_ascii_lowercase(), term.negated))
        .collect();
    let expected: BTreeSet<(String, bool)> = expected
        .into_iter()
        .map(|(variable, negated)| (variable.to_ascii_lowercase(), negated))
        .collect();
    actual.len() == expected.len() && actual == expected
}

fn validate_assignment_value(
    binding: &ControlBindingConfig,
    variable: &str,
    value: i64,
) -> Result<(), ConfigError> {
    let states = if variable.eq_ignore_ascii_case(&binding.phase.variable) {
        &binding.phase.values
    } else {
        &binding
            .outputs
            .iter()
            .find(|output| output.variable.eq_ignore_ascii_case(variable))
            .expect("expected output variable")
            .states
    };
    if states.contains_key(&value) {
        Ok(())
    } else {
        Err(ConfigError::new(format!(
            "control variable {variable} uses unmapped state code {value}"
        )))
    }
}

fn require_integer_output<'a>(
    variables: &'a BTreeMap<String, &Variable>,
    name: &str,
) -> Result<&'a Variable, ConfigError> {
    let variable = require_variable(variables, name, VariableSection::Output, None)?;
    if matches!(variable.data_type, DataType::Int | DataType::Dint) {
        Ok(variable)
    } else {
        Err(ConfigError::new(format!(
            "control output {name} must be INT or DINT"
        )))
    }
}

fn require_variable<'a>(
    variables: &'a BTreeMap<String, &Variable>,
    name: &str,
    section: VariableSection,
    data_type: Option<DataType>,
) -> Result<&'a Variable, ConfigError> {
    let variable = variables
        .get(&name.to_ascii_lowercase())
        .copied()
        .ok_or_else(|| {
            ConfigError::new(format!("Structured Text variable {name} is not declared"))
        })?;
    if variable.section != section || data_type.is_some_and(|kind| variable.data_type != kind) {
        return Err(ConfigError::new(format!(
            "Structured Text variable {name} has the wrong section or data type"
        )));
    }
    Ok(variable)
}

const fn binding_type(data_type: ControlDataType) -> DataType {
    match data_type {
        ControlDataType::Bool => DataType::Bool,
        ControlDataType::Int => DataType::Int,
        ControlDataType::Dint => DataType::Dint,
        ControlDataType::Real => DataType::Real,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::hmi::actions::process::parser::{Assignment, TimerCall};

    fn fixture() -> (ControlBindingConfig, Program) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let binding = ControlBindingConfig::from_yaml(
            &fs::read_to_string(
                root.join("project/control/bindings/body-preparation/area-01-vplc-01.yaml"),
            )
            .expect("canonical binding"),
        )
        .expect("valid canonical binding");
        let program = super::super::parser::parse(
            &fs::read_to_string(
                root.join("project/control/programs/body-preparation/area-01-vplc-01.st"),
            )
            .expect("canonical Structured Text"),
        )
        .expect("valid canonical Structured Text");
        (binding, program)
    }

    fn reject(
        binding: &ControlBindingConfig,
        mut program: Program,
        mutate: impl FnOnce(&mut Program),
        expected: &str,
    ) {
        mutate(&mut program);
        let error = compile_program(binding, &program).expect_err("invalid AST must be rejected");
        assert!(error.to_string().contains(expected), "{error}");
    }

    fn branch(program: &mut Program, number: i64) -> &mut Branch {
        program
            .branches
            .iter_mut()
            .find(|branch| branch.number == number)
            .expect("canonical branch")
    }

    #[test]
    fn compiler_rejects_program_declaration_and_case_identity_drift() {
        let (binding, program) = fixture();
        reject(
            &binding,
            program.clone(),
            |program| program.name = "Other".into(),
            "does not match",
        );
        reject(
            &binding,
            program.clone(),
            |program| program.branches[1].number = program.branches[0].number,
            "repeats a branch number",
        );
        reject(
            &binding,
            program.clone(),
            |program| program.variables.push(program.variables[0].clone()),
            "declared more than once",
        );
        reject(
            &binding,
            program.clone(),
            |program| program.case_variable = "WrongStep".into(),
            "CASE must use step variable",
        );

        let step_name = binding.sequence.step_variable.clone();
        reject(
            &binding,
            program.clone(),
            |program| {
                program
                    .variables
                    .iter_mut()
                    .find(|variable| variable.name == step_name)
                    .unwrap()
                    .data_type = DataType::Bool;
            },
            "initialized INT or DINT",
        );
        let step_name = binding.sequence.step_variable.clone();
        reject(
            &binding,
            program.clone(),
            |program| {
                program
                    .variables
                    .iter_mut()
                    .find(|variable| variable.name == step_name)
                    .unwrap()
                    .initial = Some(Literal::Integer(999));
            },
            "initialized INT or DINT",
        );
        let timer_name = binding.sequence.timer_variable.clone();
        reject(
            &binding,
            program.clone(),
            |program| {
                program
                    .variables
                    .iter_mut()
                    .find(|variable| variable.name == timer_name)
                    .unwrap()
                    .section = VariableSection::Input;
            },
            "wrong section or data type",
        );
        let input_name = binding.inputs[0].variable.clone();
        reject(
            &binding,
            program.clone(),
            |program| {
                program
                    .variables
                    .retain(|variable| variable.name != input_name);
            },
            "is not declared",
        );
        let phase_name = binding.phase.variable.clone();
        reject(
            &binding,
            program,
            |program| {
                program
                    .variables
                    .iter_mut()
                    .find(|variable| variable.name == phase_name)
                    .unwrap()
                    .data_type = DataType::Bool;
            },
            "must be INT or DINT",
        );
    }

    #[test]
    fn compiler_rejects_incomplete_duplicate_and_unmapped_branch_assignments() {
        let (binding, program) = fixture();
        let normal = binding.sequence.start_step;
        reject(
            &binding,
            program.clone(),
            |program| branch(program, normal).assignments[0].variable = "Unexpected".into(),
            "unexpected or repeated assignment",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                let duplicate = branch(program, normal).assignments[0].clone();
                branch(program, normal).assignments.push(duplicate);
            },
            "unexpected or repeated assignment",
        );
        reject(
            &binding,
            program.clone(),
            |program| branch(program, normal).assignments[0].value = Literal::Bool(true),
            "must use an integer state code",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, normal).assignments.pop();
            },
            "must assign the phase and every configured actuator output",
        );
        reject(
            &binding,
            program,
            |program| branch(program, normal).assignments[0].value = Literal::Integer(9_999),
            "uses unmapped state code",
        );
    }

    #[test]
    fn compiler_rejects_invalid_transition_shapes_for_each_branch_class() {
        let (binding, program) = fixture();
        let idle = binding.sequence.idle_step;
        let fault = binding.sequence.fault_step;
        let normal = binding.sequence.start_step;
        let canonical_timer = branch(&mut program.clone(), normal)
            .timer
            .clone()
            .expect("normal timer");

        reject(
            &binding,
            program.clone(),
            |program| branch(program, normal).transition = None,
            "requires one transition",
        );
        let step_name = binding.sequence.step_variable.clone();
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, normal)
                    .transition
                    .as_mut()
                    .unwrap()
                    .assignment
                    .variable = format!("Wrong{step_name}");
            },
            "transition must assign",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, normal)
                    .transition
                    .as_mut()
                    .unwrap()
                    .assignment
                    .value = Literal::Bool(true);
            },
            "target must be an integer step",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, normal)
                    .transition
                    .as_mut()
                    .unwrap()
                    .assignment
                    .value = Literal::Integer(9_999);
            },
            "targets missing step",
        );

        for (number, timer, expected) in [
            (
                idle,
                Some(canonical_timer.clone()),
                "idle branch must start",
            ),
            (
                fault,
                Some(canonical_timer.clone()),
                "fault branch must reset",
            ),
            (normal, None, "requires a TON call"),
        ] {
            reject(
                &binding,
                program.clone(),
                |program| branch(program, number).timer = timer,
                expected,
            );
        }

        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, idle)
                    .transition
                    .as_mut()
                    .unwrap()
                    .condition
                    .pop();
            },
            "idle branch must start",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, idle)
                    .transition
                    .as_mut()
                    .unwrap()
                    .assignment
                    .value = Literal::Integer(fault);
            },
            "idle branch must start",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, fault)
                    .transition
                    .as_mut()
                    .unwrap()
                    .condition[0]
                    .negated = true;
            },
            "fault branch must reset",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, fault)
                    .transition
                    .as_mut()
                    .unwrap()
                    .assignment
                    .value = Literal::Integer(binding.sequence.start_step);
            },
            "fault branch must reset",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, normal).timer.as_mut().unwrap().variable = "WrongTimer".into();
            },
            "configured TON done bit",
        );
        reject(
            &binding,
            program.clone(),
            |program| {
                branch(program, normal).timer.as_mut().unwrap().input[0].negated = true;
            },
            "configured TON done bit",
        );
        reject(
            &binding,
            program,
            |program| {
                branch(program, normal)
                    .transition
                    .as_mut()
                    .unwrap()
                    .condition[0]
                    .negated = true;
            },
            "configured TON done bit",
        );
    }

    #[test]
    fn binding_type_maps_every_supported_control_scalar() {
        assert_eq!(binding_type(ControlDataType::Bool), DataType::Bool);
        assert_eq!(binding_type(ControlDataType::Int), DataType::Int);
        assert_eq!(binding_type(ControlDataType::Dint), DataType::Dint);
        assert_eq!(binding_type(ControlDataType::Real), DataType::Real);
        let _ = TimerCall {
            variable: "timer".into(),
            input: Vec::new(),
            duration_ms: 1,
        };
        let _ = Assignment {
            variable: "output".into(),
            value: Literal::Integer(0),
        };
    }
}

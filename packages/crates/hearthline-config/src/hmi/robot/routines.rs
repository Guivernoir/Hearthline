use std::collections::BTreeMap;

use hearthline_engine::{RobotInstruction, RobotProgram};

use super::parser::ParsedRobotRoutine;
use crate::{ConfigError, RobotHandoffConfig, RobotMotionProfileConfig};

pub(in crate::hmi) fn routine_map(
    routines: &[ParsedRobotRoutine],
) -> Result<BTreeMap<String, RobotProgram>, ConfigError> {
    let mut mapped = BTreeMap::new();
    for routine in routines {
        if mapped
            .insert(routine.name.clone(), routine.program.clone())
            .is_some()
        {
            return Err(ConfigError::new(format!(
                "robot source repeats routine {}",
                routine.name
            )));
        }
    }
    Ok(mapped)
}

pub(in crate::hmi) fn validate_automatic_routines(
    profile: &RobotMotionProfileConfig,
    routines: &BTreeMap<String, RobotProgram>,
) -> Result<(), ConfigError> {
    validate_routines_for_handoffs(&profile.handoffs, routines)
}

pub(in crate::hmi) fn validate_routines_for_handoffs(
    handoffs: &[RobotHandoffConfig],
    routines: &BTreeMap<String, RobotProgram>,
) -> Result<(), ConfigError> {
    for handoff in handoffs {
        let Some(program) = routines.get(&handoff.program) else {
            return Err(ConfigError::new(format!(
                "robot handoff {} references missing routine {}",
                handoff.mould, handoff.program
            )));
        };
        let mut close_seen = false;
        let mut open_seen = false;
        for line in program.lines() {
            match line.instruction {
                RobotInstruction::Gripper { closed: true } if !close_seen && !open_seen => {
                    close_seen = true;
                }
                RobotInstruction::Gripper { closed: false } if close_seen && !open_seen => {
                    open_seen = true;
                }
                RobotInstruction::Gripper { .. } => {
                    return Err(ConfigError::new(format!(
                        "robot routine {} has an invalid gripper sequence",
                        handoff.program
                    )));
                }
                _ => {}
            }
        }
        if !close_seen || !open_seen {
            return Err(ConfigError::new(format!(
                "robot routine {} requires one close and one later open command",
                handoff.program
            )));
        }
    }
    Ok(())
}

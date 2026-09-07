mod parser;
mod routines;

pub(in crate::hmi) use parser::{ParsedRobotProgram, parse};
pub(in crate::hmi) use routines::{
    routine_map, validate_automatic_routines, validate_routine_assignments,
};

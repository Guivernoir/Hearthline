mod actions;
mod builder;
mod quantities;
mod robot;
mod state;

mod schema {
    pub(crate) use hearthline_config::*;
}

pub use builder::support::build_forming_telemetry_packet;
pub use hearthline_config::*;
pub use state::{PlantControlRuntime, PlantRuntimeOwnership, PlantRuntimeStore};

pub fn validate_robot_control_program(source: &str) -> Result<(), ConfigError> {
    robot::parse(source, hearthline_engine::RobotPose::default()).map(|_| ())
}

pub fn validate_structured_text_control_program(source: &str) -> Result<(), ConfigError> {
    actions::process::validate_structured_text_syntax(source)
}

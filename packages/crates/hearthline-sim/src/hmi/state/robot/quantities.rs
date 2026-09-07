use hearthline_engine::{RobotJoints, RobotMotionError, RobotPose, RobotWorkspace};
use hearthline_model::{Angle, AngularSpeed, FixedValue, LinearSpeed, Percentage, Position};

use super::super::super::{HmiRobotPose, HmiRobotWorkspace};
use crate::{ConfigError, RobotPoseConfig, RobotWorkspaceConfig};

pub(in crate::hmi) fn pose(config: RobotPoseConfig) -> RobotPose {
    robot_pose(HmiRobotPose {
        x: config.x,
        y: config.y,
        z: config.z,
        w: config.w,
        p: config.p,
        r: config.r,
    })
    .expect("validated robot profile pose")
}

pub(in crate::hmi) fn robot_pose(value: HmiRobotPose) -> Result<RobotPose, RobotMotionError> {
    Ok(RobotPose::new(
        position(value.x)?,
        position(value.y)?,
        position(value.z)?,
        angle(value.w)?,
        angle(value.p)?,
        angle(value.r)?,
    ))
}

pub(super) fn hmi_pose(value: RobotPose) -> HmiRobotPose {
    HmiRobotPose {
        x: position_to_mm(value.x),
        y: position_to_mm(value.y),
        z: position_to_mm(value.z),
        w: angle_to_degrees(value.w),
        p: angle_to_degrees(value.p),
        r: angle_to_degrees(value.r),
    }
}

pub(super) fn workspace(config: &RobotWorkspaceConfig) -> RobotWorkspace {
    RobotWorkspace {
        minimum: pose(config.minimum),
        maximum: pose(config.maximum),
        joint_minimum: RobotJoints::new(
            config
                .joint_minimum
                .map(|value| angle(value).unwrap_or(Angle::ZERO)),
        ),
        joint_maximum: RobotJoints::new(
            config
                .joint_maximum
                .map(|value| angle(value).unwrap_or(Angle::ZERO)),
        ),
    }
}

pub(super) fn hmi_workspace(value: RobotWorkspace) -> HmiRobotWorkspace {
    HmiRobotWorkspace {
        minimum: hmi_pose(value.minimum),
        maximum: hmi_pose(value.maximum),
        joint_minimum: joint_degrees(value.joint_minimum),
        joint_maximum: joint_degrees(value.joint_maximum),
    }
}

fn quantized(value: f64) -> Result<FixedValue, RobotMotionError> {
    if !value.is_finite() {
        return Err(RobotMotionError::InvalidIncrement);
    }
    format!("{value:.6}")
        .parse()
        .map_err(|_| RobotMotionError::InvalidIncrement)
}

pub(in crate::hmi) fn position(value_mm: f64) -> Result<Position, RobotMotionError> {
    Ok(Position::from_raw(quantized(value_mm)?.raw() / 1_000))
}

pub(in crate::hmi) fn angle(value_degrees: f64) -> Result<Angle, RobotMotionError> {
    Ok(Angle::from_raw(quantized(value_degrees)?.raw() / 1_000))
}

pub(in crate::hmi) fn percentage(value: f64) -> Result<Percentage, RobotMotionError> {
    Ok(Percentage::from_raw(quantized(value)?.raw() / 10_000))
}

pub(super) fn linear_speed(value_mm_s: f64) -> Result<LinearSpeed, ConfigError> {
    quantized(value_mm_s)
        .map(|value| LinearSpeed::from_raw(value.raw() / 1_000))
        .map_err(|_| ConfigError::new("invalid robot linear speed"))
}

pub(super) fn angular_speed(value_deg_s: f64) -> Result<AngularSpeed, ConfigError> {
    quantized(value_deg_s)
        .map(|value| AngularSpeed::from_raw(value.raw() / 1_000))
        .map_err(|_| ConfigError::new("invalid robot angular speed"))
}

pub(super) fn position_to_mm(value: Position) -> f64 {
    value.raw() as f64 / 1_000.0
}

pub(super) fn angle_to_degrees(value: Angle) -> f64 {
    value.raw() as f64 / 1_000.0
}

pub(super) fn percentage_to_f64(value: Percentage) -> f64 {
    value.raw() as f64 / 100.0
}

pub(super) fn joint_degrees(value: RobotJoints) -> [f64; 6] {
    value.axes.map(angle_to_degrees)
}

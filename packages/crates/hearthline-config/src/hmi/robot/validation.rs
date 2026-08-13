use std::collections::BTreeSet;

use hearthline_model::ComponentId;

use crate::{ConfigError, RobotMotionProfileConfig, RobotPoseConfig, RobotWorkspaceConfig};

pub(in crate::hmi) fn validate_profile(
    appliance_id: &str,
    profile: &RobotMotionProfileConfig,
) -> Result<(), ConfigError> {
    let architecture = &profile.architecture;
    for reference in [
        &architecture.manipulator,
        &architecture.pendant,
        &architecture.safety_interface,
        &architecture.cell_controller,
    ] {
        ComponentId::new(reference).map_err(|error| ConfigError::new(error.to_string()))?;
    }
    if architecture.servo_axes != 6 || architecture.interpolation_cycle_ms == 0 {
        return Err(ConfigError::new(format!(
            "robot controller {appliance_id} requires six servo axes and a nonzero interpolation cycle"
        )));
    }
    if !profile.program_ref.ends_with(".g") {
        return Err(ConfigError::new(format!(
            "robot actuator {appliance_id} program reference must use the .g extension"
        )));
    }
    if !profile.max_linear_speed_mm_s.is_finite()
        || profile.max_linear_speed_mm_s <= 0.0
        || !profile.max_joint_speed_deg_s.is_finite()
        || profile.max_joint_speed_deg_s <= 0.0
        || !profile.default_speed_percent.is_finite()
        || !(0.0..=100.0).contains(&profile.default_speed_percent)
        || profile.default_speed_percent == 0.0
    {
        return Err(ConfigError::new(format!(
            "robot actuator {appliance_id} has invalid motion speed limits"
        )));
    }
    let minimum = pose_values(profile.workspace.minimum);
    let maximum = pose_values(profile.workspace.maximum);
    if minimum.into_iter().zip(maximum).any(|(minimum, maximum)| {
        !minimum.is_finite() || !maximum.is_finite() || minimum >= maximum
    }) || profile
        .workspace
        .joint_minimum
        .into_iter()
        .zip(profile.workspace.joint_maximum)
        .any(|(minimum, maximum)| {
            !minimum.is_finite() || !maximum.is_finite() || minimum >= maximum
        })
    {
        return Err(ConfigError::new(format!(
            "robot actuator {appliance_id} has an invalid Cartesian or joint workspace"
        )));
    }
    if !pose_in_workspace(profile.home, &profile.workspace) {
        return Err(ConfigError::new(format!(
            "robot actuator {appliance_id} home pose is outside its workspace"
        )));
    }

    let position_ids = profile
        .taught_positions
        .iter()
        .map(|position| position.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "robot taught position", &position_ids)?;
    if let Some(position) = profile.taught_positions.iter().find(|position| {
        position.label.trim().is_empty() || !pose_in_workspace(position.pose, &profile.workspace)
    }) {
        return Err(ConfigError::new(format!(
            "robot actuator {appliance_id} taught position {} has an empty label or is outside the workspace",
            position.id
        )));
    }
    if !position_ids.iter().any(|id| id == "home") {
        return Err(ConfigError::new(format!(
            "robot actuator {appliance_id} requires a home taught position"
        )));
    }

    let frame_ids = profile
        .frames
        .iter()
        .map(|frame| frame.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "robot frame", &frame_ids)?;
    if profile.frames.is_empty()
        || profile.frames.iter().any(|frame| {
            frame.label.trim().is_empty()
                || !pose_values(frame.pose).into_iter().all(f64::is_finite)
                || frame
                    .parent
                    .as_ref()
                    .is_some_and(|parent| !frame_ids.contains(parent))
        })
        || !frame_ids.contains(&profile.active_user_frame)
    {
        return Err(ConfigError::new(format!(
            "robot controller {appliance_id} has an invalid or unresolved user frame"
        )));
    }

    let payload_ids = profile
        .payloads
        .iter()
        .map(|payload| payload.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "robot payload", &payload_ids)?;
    if profile.payloads.is_empty()
        || profile.payloads.iter().any(|payload| {
            payload.label.trim().is_empty()
                || !payload.mass_kg.is_finite()
                || payload.mass_kg <= 0.0
                || !payload.center_of_mass_mm.into_iter().all(f64::is_finite)
        })
        || !payload_ids.contains(&profile.active_payload)
    {
        return Err(ConfigError::new(format!(
            "robot controller {appliance_id} has an invalid payload definition"
        )));
    }

    let tool_ids = profile
        .tools
        .iter()
        .map(|tool| tool.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "robot tool", &tool_ids)?;
    if profile.tools.is_empty()
        || profile.tools.iter().any(|tool| {
            tool.label.trim().is_empty()
                || !pose_values(tool.tcp).into_iter().all(f64::is_finite)
                || !payload_ids.contains(&tool.payload)
        })
        || !tool_ids.contains(&profile.active_tool)
    {
        return Err(ConfigError::new(format!(
            "robot controller {appliance_id} has an invalid tool definition"
        )));
    }

    let handoff_moulds = profile
        .handoffs
        .iter()
        .map(|handoff| handoff.mould.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "robot handoff mould", &handoff_moulds)?;
    let handoff_programs = profile
        .handoffs
        .iter()
        .map(|handoff| handoff.program.clone())
        .collect::<Vec<_>>();
    require_unique_values(appliance_id, "robot handoff program", &handoff_programs)?;
    if profile.handoffs.is_empty()
        || profile.handoffs.iter().any(|handoff| {
            !valid_program_id(&handoff.program)
                || !handoff.pickup_tolerance_mm.is_finite()
                || handoff.pickup_tolerance_mm <= 0.0
                || !handoff.handoff_tolerance_mm.is_finite()
                || handoff.handoff_tolerance_mm <= 0.0
                || !handoff.orientation_tolerance_deg.is_finite()
                || handoff.orientation_tolerance_deg <= 0.0
                || !frame_ids.contains(&handoff.user_frame)
                || [
                    &handoff.approach_position,
                    &handoff.pickup_position,
                    &handoff.handoff_position,
                    &handoff.retreat_position,
                ]
                .into_iter()
                .any(|position| !position_ids.contains(position))
        })
    {
        return Err(ConfigError::new(format!(
            "robot controller {appliance_id} has an invalid mould handoff definition"
        )));
    }
    Ok(())
}

fn valid_program_id(value: &str) -> bool {
    value.len() == 5
        && value.starts_with('O')
        && value[1..].bytes().all(|byte| byte.is_ascii_digit())
}

fn pose_in_workspace(pose: RobotPoseConfig, workspace: &RobotWorkspaceConfig) -> bool {
    pose_values(pose)
        .into_iter()
        .zip(pose_values(workspace.minimum))
        .zip(pose_values(workspace.maximum))
        .all(|((value, minimum), maximum)| {
            value.is_finite() && value >= minimum && value <= maximum
        })
}

fn pose_values(pose: RobotPoseConfig) -> [f64; 6] {
    [pose.x, pose.y, pose.z, pose.w, pose.p, pose.r]
}

fn require_unique_ids(
    appliance_id: &str,
    field: &str,
    values: &[String],
) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ComponentId::new(value).map_err(|error| ConfigError::new(error.to_string()))?;
        if !seen.insert(value) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} repeats {field} {value}"
            )));
        }
    }
    Ok(())
}

fn require_unique_values(
    appliance_id: &str,
    field: &str,
    values: &[String],
) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} has an empty {field}"
            )));
        }
        if !seen.insert(value) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} repeats {field} {value}"
            )));
        }
    }
    Ok(())
}

use std::collections::BTreeSet;

use hearthline_model::{ComponentId, ComponentKind};

use crate::{BehaviorConfig, ConfigError, ConfigRepository};

use super::HmiSession;

pub(crate) fn validate_behavior(
    behavior: &BehaviorConfig,
    appliance_id: &str,
) -> Result<(), ConfigError> {
    match behavior {
        BehaviorConfig::OperatorInterface {
            controller,
            permissions,
            signal_tags,
            command_tags,
            safety_components,
            control_station,
            parameters,
            recipes,
            active_recipe,
            supervisory_profile,
        } => {
            ComponentId::new(controller).map_err(|error| ConfigError::new(error.to_string()))?;
            if permissions.is_empty() {
                return Err(ConfigError::new(format!(
                    "operator interface {appliance_id} requires at least one permission"
                )));
            }
            require_unique_values(appliance_id, "permission", permissions)?;
            require_unique_ids(appliance_id, "signal tag", signal_tags)?;
            require_unique_ids(appliance_id, "command tag", command_tags)?;
            require_unique_ids(appliance_id, "safety component", safety_components)?;
            if let Some(station) = control_station {
                ComponentId::new(&station.target)
                    .map_err(|error| ConfigError::new(error.to_string()))?;
                if let Some(selector) = &station.mode_selector {
                    if selector.positions.is_empty()
                        || !selector.positions.contains(&selector.initial_position)
                    {
                        return Err(ConfigError::new(format!(
                            "operator interface {appliance_id} mode selector requires its initial position"
                        )));
                    }
                    let positions = selector
                        .positions
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>();
                    require_unique_values(appliance_id, "selector position", &positions)?;
                    if selector
                        .positions
                        .contains(&crate::OperatorControlMode::Setup)
                        && selector
                            .setup_password_sha256
                            .as_ref()
                            .is_none_or(|digest| {
                                digest.len() != 64
                                    || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                            })
                    {
                        return Err(ConfigError::new(format!(
                            "operator interface {appliance_id} setup mode requires a SHA-256 password digest"
                        )));
                    }
                    require_unique_values(
                        appliance_id,
                        "bypassed permissive",
                        &selector.bypassed_permissives,
                    )?;
                    require_unique_values(
                        appliance_id,
                        "retained protection",
                        &selector.retained_protections,
                    )?;
                }
            }
            let parameter_ids = parameters
                .iter()
                .map(|parameter| parameter.id.clone())
                .collect::<Vec<_>>();
            require_unique_ids(appliance_id, "parameter", &parameter_ids)?;
            if let Some(parameter) = parameters.iter().find(|parameter| {
                !parameter.minimum.is_finite()
                    || !parameter.maximum.is_finite()
                    || !parameter.step.is_finite()
                    || !parameter.initial_value.is_finite()
                    || parameter.minimum >= parameter.maximum
                    || parameter.step <= 0.0
                    || parameter.initial_value < parameter.minimum
                    || parameter.initial_value > parameter.maximum
            }) {
                return Err(ConfigError::new(format!(
                    "operator interface {appliance_id} parameter {} has an invalid range or initial value",
                    parameter.id
                )));
            }
            let recipe_ids = recipes
                .iter()
                .map(|recipe| recipe.id.clone())
                .collect::<Vec<_>>();
            require_unique_ids(appliance_id, "recipe", &recipe_ids)?;
            if let Some(active) = active_recipe
                && !recipe_ids.contains(active)
            {
                return Err(ConfigError::new(format!(
                    "operator interface {appliance_id} selects unknown active recipe {active}"
                )));
            }
            if let Some(profile) = supervisory_profile {
                validate_supervisory_profile(appliance_id, profile)?;
            }
            Ok(())
        }
        BehaviorConfig::RemoteIo {
            control_cabinet: Some(cabinet),
            ..
        } => validate_control_cabinet(appliance_id, cabinet),
        BehaviorConfig::FieldSensor {
            minimum,
            maximum,
            initial_value: Some(initial_value),
            ..
        } if minimum < maximum
            && (!initial_value.is_finite()
                || initial_value < minimum
                || initial_value > maximum) =>
        {
            Err(ConfigError::new(format!(
                "sensor {appliance_id} initial value must be within {minimum}..={maximum}"
            )))
        }
        BehaviorConfig::FieldActuator {
            safe_state,
            states,
            motion_profile,
            utility_cabinet,
            ..
        } => {
            require_unique_values(appliance_id, "actuator state", states)?;
            if !states.is_empty() && !states.contains(safe_state) {
                return Err(ConfigError::new(format!(
                    "actuator {appliance_id} safe state {safe_state} is not in its configured states"
                )));
            }
            if let Some(profile) = motion_profile {
                super::robot::validate_profile(appliance_id, profile)?;
            }
            if let Some(cabinet) = utility_cabinet {
                validate_utility_cabinet(appliance_id, cabinet)?;
            }
            Ok(())
        }
        BehaviorConfig::Safety {
            permissives,
            initially_permissive,
            ..
        } => {
            require_unique_values(appliance_id, "permissive", permissives)?;
            require_unique_values(appliance_id, "initially permissive", initially_permissive)?;
            if let Some(unknown) = initially_permissive
                .iter()
                .find(|candidate| !permissives.contains(candidate))
            {
                return Err(ConfigError::new(format!(
                    "safety interface {appliance_id} initializes unknown permissive {unknown}"
                )));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_control_cabinet(
    appliance_id: &str,
    cabinet: &crate::MouldControlCabinetConfig,
) -> Result<(), ConfigError> {
    ComponentId::new(&cabinet.target).map_err(|error| ConfigError::new(error.to_string()))?;
    ComponentId::new(&cabinet.utility_cabinet)
        .map_err(|error| ConfigError::new(error.to_string()))?;
    if cabinet.enclosure_rating.trim().is_empty()
        || cabinet.safety_relay.trim().is_empty()
        || cabinet.control_voltage_vdc == 0
        || cabinet.modules.is_empty()
    {
        return Err(ConfigError::new(format!(
            "remote I/O {appliance_id} has an incomplete mould control cabinet"
        )));
    }
    require_unique_values(appliance_id, "control cabinet module", &cabinet.modules)
}

fn validate_utility_cabinet(
    appliance_id: &str,
    cabinet: &crate::MouldUtilityCabinetConfig,
) -> Result<(), ConfigError> {
    ComponentId::new(&cabinet.target).map_err(|error| ConfigError::new(error.to_string()))?;
    ComponentId::new(&cabinet.remote_io).map_err(|error| ConfigError::new(error.to_string()))?;
    let circuit_ids = cabinet
        .circuits
        .iter()
        .map(|circuit| circuit.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "utility circuit", &circuit_ids)?;
    if cabinet.enclosure_rating.trim().is_empty()
        || cabinet.isolation_state.trim().is_empty()
        || cabinet.control_voltage_vdc == 0
        || cabinet.circuits.is_empty()
        || cabinet.circuits.iter().any(|circuit| {
            circuit.label.trim().is_empty()
                || circuit.source.trim().is_empty()
                || circuit.command_states.is_empty()
                || circuit
                    .nominal_pressure
                    .is_some_and(|pressure| !pressure.is_finite())
        })
    {
        return Err(ConfigError::new(format!(
            "actuator {appliance_id} has an incomplete mould-embedded utility section"
        )));
    }
    for circuit in &cabinet.circuits {
        require_unique_values(
            appliance_id,
            "utility circuit state",
            &circuit.command_states,
        )?;
    }
    Ok(())
}

fn validate_supervisory_profile(
    appliance_id: &str,
    profile: &crate::SupervisoryProfileConfig,
) -> Result<(), ConfigError> {
    if profile.namespace.trim().is_empty()
        || profile.model_id.trim().is_empty()
        || profile.repository.revision.trim().is_empty()
        || profile.repository.deployed_revision.trim().is_empty()
    {
        return Err(ConfigError::new(format!(
            "operator interface {appliance_id} has incomplete supervisory metadata"
        )));
    }
    ComponentId::new(&profile.repository.engineering_node)
        .map_err(|error| ConfigError::new(error.to_string()))?;
    let template_ids = profile
        .templates
        .iter()
        .map(|template| template.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "supervisory template", &template_ids)?;
    if profile.templates.is_empty()
        || profile.templates.iter().any(|template| {
            template.label.trim().is_empty()
                || template.attributes.is_empty()
                || template
                    .parent
                    .as_ref()
                    .is_some_and(|parent| !template_ids.contains(parent))
        })
    {
        return Err(ConfigError::new(format!(
            "operator interface {appliance_id} has an invalid supervisory template"
        )));
    }
    let asset_ids = profile
        .assets
        .iter()
        .map(|asset| asset.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "supervisory asset", &asset_ids)?;
    if profile.assets.is_empty()
        || profile.assets.iter().any(|asset| {
            asset.label.trim().is_empty()
                || !template_ids.contains(&asset.template)
                || asset
                    .parent
                    .as_ref()
                    .is_some_and(|parent| !asset_ids.contains(parent))
        })
    {
        return Err(ConfigError::new(format!(
            "operator interface {appliance_id} has an invalid supervisory asset"
        )));
    }
    let node_ids = profile
        .deployment_nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "supervisory deployment node", &node_ids)?;
    if profile.deployment_nodes.is_empty()
        || profile
            .deployment_nodes
            .iter()
            .any(|node| node.label.trim().is_empty() || ComponentId::new(&node.host).is_err())
        || !node_ids.contains(&profile.repository.engineering_node)
    {
        return Err(ConfigError::new(format!(
            "operator interface {appliance_id} has an invalid supervisory deployment"
        )));
    }
    let role_ids = profile
        .roles
        .iter()
        .map(|role| role.id.clone())
        .collect::<Vec<_>>();
    require_unique_ids(appliance_id, "supervisory role", &role_ids)?;
    if profile.roles.is_empty()
        || profile
            .roles
            .iter()
            .any(|role| role.label.trim().is_empty() || role.permissions.is_empty())
        || !role_ids.contains(&profile.identity.role)
        || profile.identity.user.trim().is_empty()
        || profile.identity.authentication.trim().is_empty()
        || profile.history.sample_interval_ms == 0
        || !(8..=120).contains(&profile.history.capacity)
        || profile.history.tags.is_empty()
    {
        return Err(ConfigError::new(format!(
            "operator interface {appliance_id} has an invalid supervisory security or history model"
        )));
    }
    require_unique_values(
        appliance_id,
        "supervisory history tag",
        &profile.history.tags,
    )
}

pub(crate) fn validate_repository(appliances: &ConfigRepository) -> Result<(), ConfigError> {
    for loaded in appliances.appliances() {
        match &loaded.config.behavior {
            BehaviorConfig::FieldActuator {
                motion_profile: Some(profile),
                ..
            } => validate_robot_references(appliances, &loaded.config.id, profile)?,
            BehaviorConfig::RemoteIo {
                control_cabinet: Some(cabinet),
                ..
            } => {
                require_loaded_reference(appliances, &loaded.config.id, &cabinet.utility_cabinet)?;
            }
            BehaviorConfig::FieldActuator {
                utility_cabinet: Some(cabinet),
                ..
            } => {
                require_loaded_reference(appliances, &loaded.config.id, &cabinet.remote_io)?;
            }
            BehaviorConfig::OperatorInterface {
                supervisory_profile: Some(profile),
                ..
            } => validate_supervisory_references(appliances, &loaded.config.id, profile)?,
            _ => {}
        }
    }
    for loaded in appliances.appliances().filter(|loaded| {
        matches!(
            loaded.config.kind,
            ComponentKind::Hmi | ComponentKind::ScadaWorkstation
        ) && loaded.config.tags.iter().any(|tag| tag == "interactive")
    }) {
        HmiSession::from_repository(appliances, &loaded.config.id)?;
    }
    Ok(())
}

fn validate_robot_references(
    appliances: &ConfigRepository,
    appliance_id: &str,
    profile: &crate::RobotMotionProfileConfig,
) -> Result<(), ConfigError> {
    if profile.architecture.cell_controller != appliance_id {
        return Err(ConfigError::new(format!(
            "robot motion profile on {appliance_id} names {} as its controller",
            profile.architecture.cell_controller
        )));
    }
    for reference in [
        &profile.architecture.manipulator,
        &profile.architecture.pendant,
        &profile.architecture.safety_interface,
        &profile.architecture.cell_controller,
    ] {
        require_loaded_reference(appliances, appliance_id, reference)?;
    }
    Ok(())
}

fn validate_supervisory_references(
    appliances: &ConfigRepository,
    appliance_id: &str,
    profile: &crate::SupervisoryProfileConfig,
) -> Result<(), ConfigError> {
    for node in &profile.deployment_nodes {
        require_loaded_reference(appliances, appliance_id, &node.host)?;
    }
    for asset in &profile.assets {
        for component in &asset.components {
            require_loaded_reference(appliances, appliance_id, component)?;
        }
        for tag in &asset.historized_tags {
            require_signal_reference(appliances, appliance_id, tag)?;
        }
    }
    for tag in &profile.history.tags {
        require_signal_reference(appliances, appliance_id, tag)?;
    }
    Ok(())
}

fn require_loaded_reference(
    appliances: &ConfigRepository,
    appliance_id: &str,
    reference: &str,
) -> Result<(), ConfigError> {
    appliances.get(reference).map(|_| ()).ok_or_else(|| {
        ConfigError::new(format!(
            "appliance {appliance_id} references unknown appliance {reference}"
        ))
    })
}

fn require_signal_reference(
    appliances: &ConfigRepository,
    appliance_id: &str,
    tag: &str,
) -> Result<(), ConfigError> {
    appliances
        .appliances()
        .any(|candidate| {
            matches!(
                &candidate.config.behavior,
                BehaviorConfig::FieldSensor { signal_tag, .. } if signal_tag == tag
            )
        })
        .then_some(())
        .ok_or_else(|| {
            ConfigError::new(format!(
                "appliance {appliance_id} references unknown signal tag {tag}"
            ))
        })
}

fn require_unique_ids(
    appliance_id: &str,
    field: &str,
    values: &[String],
) -> Result<(), ConfigError> {
    for value in values {
        ComponentId::new(value).map_err(|error| ConfigError::new(error.to_string()))?;
    }
    require_unique_values(appliance_id, field, values)
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

use std::collections::BTreeMap;

use crate::{BehaviorConfig, ConfigRepository, OperatorControlMode, OperatorStationConfig};

use super::super::state::ControlStationRuntime;
use super::super::{HmiControlMode, HmiParameter, HmiRecipe};

pub(super) struct OperatorMetadata {
    pub(super) stations: BTreeMap<String, ControlStationRuntime>,
    pub(super) parameters: Vec<HmiParameter>,
    pub(super) recipes: Vec<HmiRecipe>,
    pub(super) active_recipe: Option<String>,
}

pub(super) fn operator_metadata(
    appliances: &ConfigRepository,
    controller: &str,
) -> OperatorMetadata {
    let mut stations = BTreeMap::new();
    let mut parameters = Vec::new();
    let mut recipes = Vec::new();
    let mut active_recipe = None;
    for candidate in appliances.appliances() {
        let BehaviorConfig::OperatorInterface {
            controller: assigned,
            control_station,
            parameters: configured_parameters,
            recipes: configured_recipes,
            active_recipe: configured_active_recipe,
            ..
        } = &candidate.config.behavior
        else {
            continue;
        };
        if assigned != controller {
            continue;
        }
        if let Some(station) = control_station {
            stations.insert(
                candidate.config.id.clone(),
                station_runtime(
                    candidate.config.id.clone(),
                    candidate.config.label.clone(),
                    station,
                ),
            );
        }
        parameters.extend(configured_parameters.iter().map(|parameter| HmiParameter {
            id: parameter.id.clone(),
            label: parameter.label.clone(),
            target: parameter.target.clone(),
            unit: parameter.unit.clone(),
            minimum: parameter.minimum,
            maximum: parameter.maximum,
            step: parameter.step,
            value: parameter.initial_value,
        }));
        recipes.extend(configured_recipes.iter().map(|recipe| HmiRecipe {
            id: recipe.id.clone(),
            label: recipe.label.clone(),
            description: recipe.description.clone(),
        }));
        if configured_active_recipe.is_some() {
            active_recipe.clone_from(configured_active_recipe);
        }
    }
    OperatorMetadata {
        stations,
        parameters,
        recipes,
        active_recipe,
    }
}

fn station_runtime(
    id: String,
    label: String,
    station: &OperatorStationConfig,
) -> ControlStationRuntime {
    let selector = station.mode_selector.as_ref();
    ControlStationRuntime {
        id,
        label,
        station_type: station.station_type.to_string(),
        target: station.target.clone(),
        positions: selector
            .map(|selector| selector.positions.iter().copied().map(mode).collect())
            .unwrap_or_default(),
        selected_mode: selector
            .map(|selector| mode(selector.initial_position))
            .unwrap_or(HmiControlMode::Auto),
        setup_password_sha256: selector.and_then(|selector| selector.setup_password_sha256.clone()),
        setup_authenticated: false,
        bypassed_permissives: selector
            .map(|selector| selector.bypassed_permissives.clone())
            .unwrap_or_default(),
        retained_protections: selector
            .map(|selector| selector.retained_protections.clone())
            .unwrap_or_default(),
    }
}

const fn mode(mode: OperatorControlMode) -> HmiControlMode {
    match mode {
        OperatorControlMode::Manual => HmiControlMode::Manual,
        OperatorControlMode::Auto => HmiControlMode::Auto,
        OperatorControlMode::Setup => HmiControlMode::Setup,
    }
}

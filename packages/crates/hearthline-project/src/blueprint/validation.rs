use std::collections::{BTreeMap, BTreeSet};

use hearthline_model::ComponentId;

use crate::ProjectError;

use super::{
    BlueprintDefinition, BlueprintNode, BlueprintParameter, BlueprintParameterKind,
    BlueprintParameterValue, BlueprintRepository,
};

pub(super) fn validate_import_graph(
    repository: &BlueprintRepository,
    id: &str,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<(), ProjectError> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.into()) {
        return Err(ProjectError::Blueprint(format!(
            "blueprint import cycle at {id}"
        )));
    }
    let definition = repository.definition(id)?;
    for import in &definition.imports {
        validate_import_graph(repository, &import.blueprint, visiting, visited)?;
    }
    visiting.remove(id);
    visited.insert(id.into());
    Ok(())
}

pub(super) fn validate_definition(definition: &BlueprintDefinition) -> Result<(), ProjectError> {
    ComponentId::new(&definition.id).map_err(|error| {
        ProjectError::Blueprint(format!("blueprint {}: {error}", definition.id))
    })?;
    let parameter_ids = definition
        .parameters
        .iter()
        .map(|parameter| parameter.id.as_str())
        .collect::<BTreeSet<_>>();
    require_unique(
        parameter_ids.len(),
        definition.parameters.len(),
        &definition.id,
        "parameter",
    )?;
    let node_ids = definition
        .nodes
        .iter()
        .map(|node| node.local_id.as_str())
        .collect::<BTreeSet<_>>();
    require_unique(
        node_ids.len(),
        definition.nodes.len(),
        &definition.id,
        "node",
    )?;
    let connection_ids = definition
        .connections
        .iter()
        .map(|connection| connection.local_id.as_str())
        .collect::<BTreeSet<_>>();
    require_unique(
        connection_ids.len(),
        definition.connections.len(),
        &definition.id,
        "connection",
    )?;
    let export_ids = definition
        .exports
        .iter()
        .map(|export| export.id.as_str())
        .collect::<BTreeSet<_>>();
    require_unique(
        export_ids.len(),
        definition.exports.len(),
        &definition.id,
        "export",
    )?;
    for node in &definition.nodes {
        validate_local_id(&definition.id, "node", &node.local_id)?;
        require_repeat_parameter(
            &definition.id,
            &node.local_id,
            node.repeat_parameter.as_deref(),
            &parameter_ids,
        )?;
    }
    for connection in &definition.connections {
        validate_local_id(&definition.id, "connection", &connection.local_id)?;
        if !node_ids.contains(connection.from_node.as_str())
            || !node_ids.contains(connection.to_node.as_str())
        {
            return Err(ProjectError::Blueprint(format!(
                "blueprint {} connection {} references an unknown node",
                definition.id, connection.local_id
            )));
        }
    }
    for nested in &definition.nested {
        validate_local_id(&definition.id, "nested instance", &nested.local_id)?;
        require_repeat_parameter(
            &definition.id,
            &nested.local_id,
            nested.repeat_parameter.as_deref(),
            &parameter_ids,
        )?;
    }
    for export in &definition.exports {
        validate_local_id(&definition.id, "export", &export.id)?;
        let node = definition
            .nodes
            .iter()
            .find(|node| node.local_id == export.node)
            .ok_or_else(|| {
                ProjectError::Blueprint(format!(
                    "blueprint {} export {} references unknown node {}",
                    definition.id, export.id, export.node
                ))
            })?;
        if !node.ports.contains(&export.port) {
            return Err(ProjectError::Blueprint(format!(
                "blueprint {} export {} references unknown port {} on {}",
                definition.id, export.id, export.port, export.node
            )));
        }
        if export.contract.trim().is_empty() {
            return Err(ProjectError::Blueprint(format!(
                "blueprint {} export {} requires a contract",
                definition.id, export.id
            )));
        }
    }
    Ok(())
}

fn require_unique(
    unique: usize,
    total: usize,
    blueprint: &str,
    item: &str,
) -> Result<(), ProjectError> {
    if unique == total {
        Ok(())
    } else {
        Err(ProjectError::Blueprint(format!(
            "blueprint {blueprint} repeats a {item}"
        )))
    }
}

fn validate_local_id(blueprint: &str, kind: &str, id: &str) -> Result<(), ProjectError> {
    ComponentId::new(id).map_err(|error| {
        ProjectError::Blueprint(format!("blueprint {blueprint} {kind} {id}: {error}"))
    })?;
    Ok(())
}

fn require_repeat_parameter(
    blueprint: &str,
    local_id: &str,
    parameter: Option<&str>,
    known: &BTreeSet<&str>,
) -> Result<(), ProjectError> {
    if parameter.is_some_and(|parameter| !known.contains(parameter)) {
        return Err(ProjectError::Blueprint(format!(
            "blueprint {blueprint} item {local_id} references unknown repeat parameter {}",
            parameter.expect("checked parameter")
        )));
    }
    Ok(())
}

pub(super) fn validated_values(
    definition: &BlueprintDefinition,
    supplied: &BTreeMap<String, BlueprintParameterValue>,
) -> Result<BTreeMap<String, BlueprintParameterValue>, ProjectError> {
    let mut values = BTreeMap::new();
    for parameter in &definition.parameters {
        let value = supplied.get(&parameter.id).or(parameter.default.as_ref());
        let Some(value) = value else {
            if parameter.required {
                return Err(ProjectError::Blueprint(format!(
                    "blueprint {} requires parameter {}",
                    definition.id, parameter.id
                )));
            }
            continue;
        };
        validate_value(parameter, value)?;
        values.insert(parameter.id.clone(), value.clone());
    }
    if supplied
        .keys()
        .any(|id| !definition.parameters.iter().any(|item| &item.id == id))
    {
        return Err(ProjectError::Blueprint(format!(
            "blueprint {} received an unknown parameter",
            definition.id
        )));
    }
    Ok(values)
}

fn validate_value(
    parameter: &BlueprintParameter,
    value: &BlueprintParameterValue,
) -> Result<(), ProjectError> {
    let valid = match (&parameter.kind, value) {
        (BlueprintParameterKind::Boolean, BlueprintParameterValue::Boolean(_)) => true,
        (
            BlueprintParameterKind::Integer { minimum, maximum },
            BlueprintParameterValue::Integer(value),
        ) => value >= minimum && value <= maximum,
        (BlueprintParameterKind::Identifier, BlueprintParameterValue::Text(value)) => {
            ComponentId::new(value).is_ok()
        }
        (BlueprintParameterKind::Choice { values }, BlueprintParameterValue::Text(value)) => {
            values.contains(value)
        }
        (
            BlueprintParameterKind::IdentifierList { minimum, maximum },
            BlueprintParameterValue::List(values),
        ) => {
            values.len() >= *minimum
                && values.len() <= *maximum
                && values.iter().all(|value| ComponentId::new(value).is_ok())
        }
        (
            BlueprintParameterKind::Quantity {
                unit,
                scale,
                minimum_raw,
                maximum_raw,
            },
            BlueprintParameterValue::Text(value),
        ) => parse_quantity(value, unit, *scale)
            .is_some_and(|raw| raw >= *minimum_raw && raw <= *maximum_raw),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ProjectError::Blueprint(format!(
            "blueprint parameter {} has an invalid value",
            parameter.id
        )))
    }
}

fn parse_quantity(value: &str, unit: &str, scale: i64) -> Option<i64> {
    let number = value.strip_suffix(unit)?.trim();
    let negative = number.starts_with('-');
    let number = number.trim_start_matches(['-', '+']);
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if fraction.len() > decimal_places(scale)?
        || !whole.chars().all(|character| character.is_ascii_digit())
        || !fraction.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    let mut raw = whole.parse::<i64>().ok()?.checked_mul(scale)?;
    if !fraction.is_empty() {
        let divisor = 10_i64.checked_pow(fraction.len() as u32)?;
        raw = raw.checked_add(
            fraction
                .parse::<i64>()
                .ok()?
                .checked_mul(scale.checked_div(divisor)?)?,
        )?;
    }
    Some(if negative { -raw } else { raw })
}

fn decimal_places(mut scale: i64) -> Option<usize> {
    let mut places = 0;
    while scale > 1 && scale % 10 == 0 {
        scale /= 10;
        places += 1;
    }
    (scale == 1).then_some(places)
}

pub(super) fn repeat_count(
    node: &BlueprintNode,
    values: &BTreeMap<String, BlueprintParameterValue>,
) -> Result<usize, ProjectError> {
    optional_repeat_count(&node.local_id, node.repeat_parameter.as_deref(), values)
}

pub(super) fn optional_repeat_count(
    local_id: &str,
    parameter: Option<&str>,
    values: &BTreeMap<String, BlueprintParameterValue>,
) -> Result<usize, ProjectError> {
    let Some(parameter) = parameter else {
        return Ok(1);
    };
    match values.get(parameter) {
        Some(BlueprintParameterValue::Integer(value)) if (1..=1_000).contains(value) => {
            Ok(*value as usize)
        }
        _ => Err(ProjectError::Blueprint(format!(
            "item {local_id} repeat parameter {parameter} must be between 1 and 1000"
        ))),
    }
}

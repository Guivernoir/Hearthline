use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::{CapacityPlan, SourceDigest};

use super::{CapacityDelta, MAX_DOCUMENT_BYTES, MAX_YAML_INDENT, TransactionError};

pub(super) fn validate_source_path(path: &Path) -> Result<PathBuf, TransactionError> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(TransactionError::Validation(
            "source path must be normalized and relative".into(),
        ));
    }
    let normalized = path.to_string_lossy().replace('\\', "/");
    let allowed = [
        "project/config/appliances/",
        "project/config/connections/",
        "project/config/scenarios/",
        "project/config/blueprints/",
        "project/config/instances/",
        "project/config/ot/",
    ];
    if !allowed.iter().any(|root| normalized.starts_with(root)) || !normalized.ends_with(".yaml") {
        return Err(TransactionError::Validation(format!(
            "source path {normalized} is outside editable roots"
        )));
    }
    Ok(PathBuf::from(normalized))
}

pub(super) fn validate_source(source: &str) -> Result<(), TransactionError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(TransactionError::Validation(format!(
            "document exceeds {MAX_DOCUMENT_BYTES} bytes"
        )));
    }
    if source.lines().any(|line| {
        line.chars()
            .take_while(|character| *character == ' ')
            .count()
            > MAX_YAML_INDENT
    }) {
        return Err(TransactionError::Validation(format!(
            "YAML indentation exceeds {MAX_YAML_INDENT} spaces"
        )));
    }
    validate_flow_nesting(source)?;
    let _: serde_yaml_ng::Value = serde_yaml_ng::from_str(source)
        .map_err(|error| TransactionError::Validation(format!("invalid YAML: {error}")))?;
    Ok(())
}

fn validate_flow_nesting(source: &str) -> Result<(), TransactionError> {
    let mut depth = 0usize;
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut escaped = false;
    for character in source.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if double_quoted && character == '\\' {
            escaped = true;
            continue;
        }
        if !double_quoted && character == '\'' {
            single_quoted = !single_quoted;
            continue;
        }
        if !single_quoted && character == '"' {
            double_quoted = !double_quoted;
            continue;
        }
        if single_quoted || double_quoted {
            continue;
        }
        match character {
            '[' | '{' => {
                depth = depth.saturating_add(1);
                if depth > MAX_YAML_INDENT {
                    return Err(TransactionError::Validation(format!(
                        "YAML flow nesting exceeds {MAX_YAML_INDENT} levels"
                    )));
                }
            }
            ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn reject_symlink_ancestors(
    root: &Path,
    relative: &Path,
) -> Result<(), TransactionError> {
    let mut cursor = root.to_path_buf();
    for component in relative.components() {
        cursor.push(component.as_os_str());
        if let Ok(metadata) = fs::symlink_metadata(&cursor)
            && metadata.file_type().is_symlink()
        {
            return Err(TransactionError::Validation(format!(
                "source path {} traverses a symbolic link",
                relative.display()
            )));
        }
    }
    Ok(())
}

pub(super) fn diagnostic_line(message: &str) -> Option<usize> {
    let start = message.find("line ")? + "line ".len();
    message[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

pub(super) fn capacity_deltas(
    current: &CapacityPlan,
    candidate: &CapacityPlan,
) -> Vec<CapacityDelta> {
    let current = current
        .assessments
        .iter()
        .map(|item| ((item.resource, item.scope.as_str()), item))
        .collect::<BTreeMap<_, _>>();
    let candidate = candidate
        .assessments
        .iter()
        .map(|item| ((item.resource, item.scope.as_str()), item))
        .collect::<BTreeMap<_, _>>();
    let keys = current
        .keys()
        .chain(candidate.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    keys.into_iter()
        .filter_map(|key| {
            let previous = current.get(&key);
            let next = candidate.get(&key);
            (previous.map(|item| item.demand) != next.map(|item| item.demand)).then(|| {
                CapacityDelta {
                    resource: key.0,
                    scope: key.1.into(),
                    previous_demand: previous.map(|item| item.demand),
                    candidate_demand: next.map(|item| item.demand),
                    candidate_status: next.map(|item| item.status),
                }
            })
        })
        .collect()
}

pub(super) fn affected_scenarios(
    current: &crate::CompiledProject,
    candidate: &crate::CompiledProject,
    changed: &BTreeSet<&str>,
) -> Vec<String> {
    current
        .scenarios()
        .scenarios()
        .chain(candidate.scenarios().scenarios())
        .filter(|scenario| {
            changed.contains(scenario.config.id.as_str())
                || scenario
                    .config
                    .participants
                    .iter()
                    .any(|participant| changed.contains(participant.as_str()))
        })
        .map(|scenario| scenario.config.id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) struct ObjectChanges {
    pub(super) added: Vec<String>,
    pub(super) removed: Vec<String>,
    pub(super) modified: Vec<String>,
}

pub(super) fn object_diff(current: &[SourceDigest], candidate: &[SourceDigest]) -> ObjectChanges {
    let current = current
        .iter()
        .map(|item| {
            (
                (item.kind.as_str(), item.path.as_str()),
                item.sha256.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let candidate = candidate
        .iter()
        .map(|item| {
            (
                (item.kind.as_str(), item.path.as_str()),
                item.sha256.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    ObjectChanges {
        added: candidate
            .keys()
            .filter(|key| !current.contains_key(*key))
            .map(|key| key.1.into())
            .collect(),
        removed: current
            .keys()
            .filter(|key| !candidate.contains_key(*key))
            .map(|key| key.1.into())
            .collect(),
        modified: current
            .iter()
            .filter(|(key, digest)| candidate.get(key).is_some_and(|next| next != *digest))
            .map(|(key, _)| key.1.into())
            .collect(),
    }
}

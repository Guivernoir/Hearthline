use std::fs;
use std::path::Path;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, ProcessViewConfig, ScenarioRepository,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::source::sha256_hex;
use crate::{ExpandedBlueprint, SourceDigest};

use super::{COMPILED_PROJECT_SCHEMA_VERSION, ProjectError};

pub(super) fn source_digests(root: &Path) -> Result<Vec<SourceDigest>, ProjectError> {
    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .map(|entry| entry.map_err(ProjectError::io))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("model.lock.json"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).map_err(ProjectError::io)?;
            Ok(SourceDigest {
                path: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
                kind: source_kind(&path).into(),
                sha256: sha256(&bytes),
            })
        })
        .collect()
}

pub(super) fn object_digests(
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
    scenarios: &ScenarioRepository,
    blueprints: &[ExpandedBlueprint],
) -> Result<Vec<SourceDigest>, ProjectError> {
    let mut objects = appliances
        .appliances()
        .map(|item| SourceDigest {
            path: item.config.id.clone(),
            kind: "appliance".into(),
            sha256: item.revision(),
        })
        .chain(connections.connections().map(|item| SourceDigest {
            path: item.config.id.clone(),
            kind: "connection".into(),
            sha256: item.revision(),
        }))
        .chain(scenarios.scenarios().map(|item| SourceDigest {
            path: item.config.id.clone(),
            kind: "scenario".into(),
            sha256: sha256(item.source_yaml.as_bytes()),
        }))
        .chain(blueprints.iter().flat_map(|blueprint| {
            let nodes = blueprint.nodes.iter().map(|node| SourceDigest {
                path: node.id.clone(),
                kind: "blueprint-appliance".into(),
                sha256: sha256(&serde_json::to_vec(node).expect("expanded node serialization")),
            });
            let connections = blueprint.connections.iter().map(|connection| SourceDigest {
                path: connection.id.clone(),
                kind: "blueprint-connection".into(),
                sha256: sha256(
                    &serde_json::to_vec(connection).expect("expanded connection serialization"),
                ),
            });
            nodes.chain(connections)
        }))
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        (left.kind.as_str(), left.path.as_str()).cmp(&(right.kind.as_str(), right.path.as_str()))
    });
    if objects
        .windows(2)
        .any(|items| items[0].kind == items[1].kind && items[0].path == items[1].path)
    {
        return Err(ProjectError::Configuration(
            "normalized object identifiers collide after blueprint expansion".into(),
        ));
    }
    Ok(objects)
}

pub(super) fn generated_catalog_digests(
    root: &Path,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
) -> Result<Vec<SourceDigest>, ProjectError> {
    let process = ProcessViewConfig::load(root.join("ot/process/architecture.yaml"), appliances)?;
    let catalogs = [
        (
            "packages/web/src/generated/appliance-configs.json",
            serde_json::to_string(&appliances.frontend_catalog(connections))
                .map(|source| format!("{source}\n").into_bytes()),
        ),
        (
            "packages/web/src/generated/process-view.json",
            serde_json::to_string(&process.into_frontend(appliances)?)
                .map(|source| format!("{source}\n").into_bytes()),
        ),
    ];
    catalogs
        .into_iter()
        .map(|(path, source)| {
            Ok(SourceDigest {
                path: path.into(),
                kind: "generated-catalog".into(),
                sha256: sha256(
                    &source.map_err(|error| ProjectError::Configuration(error.to_string()))?,
                ),
            })
        })
        .collect()
}

pub(super) fn project_digest(
    sources: &[SourceDigest],
    objects: &[SourceDigest],
    blueprints: &[ExpandedBlueprint],
) -> Result<String, ProjectError> {
    #[derive(Serialize)]
    struct Authority<'a> {
        schema: &'static str,
        sources: &'a [SourceDigest],
        objects: &'a [SourceDigest],
        blueprints: &'a [ExpandedBlueprint],
    }
    let normalized = serde_json::to_vec(&Authority {
        schema: COMPILED_PROJECT_SCHEMA_VERSION,
        sources,
        objects,
        blueprints,
    })
    .map_err(|error| ProjectError::Lock(error.to_string()))?;
    Ok(sha256(&normalized))
}

fn source_kind(path: &Path) -> &'static str {
    let value = path.to_string_lossy();
    if value.contains("/blueprints/") {
        "blueprint"
    } else if value.contains("/instances/") {
        "instance"
    } else if value.contains("/appliances/") {
        "appliance"
    } else if value.contains("/connections/") {
        "connection"
    } else if value.contains("/scenarios/") {
        "scenario"
    } else if value.ends_with(".st") || value.ends_with(".g") {
        "control-program"
    } else {
        "project"
    }
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    sha256_hex(Sha256::digest(bytes).as_slice())
}

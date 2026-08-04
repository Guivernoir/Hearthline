use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::appliance::{ConfigError, ConfigRepository, source_revision};

use super::{
    ConnectionConfig, FrontendConnection, collect_yaml_paths,
    redundancy::validate_redundancy_connections, validate_endpoint, validate_endpoint_port,
};

#[derive(Clone, Debug)]
pub struct LoadedConnection {
    pub config: ConnectionConfig,
    pub source_path: String,
    pub source_yaml: String,
    pub source_file: PathBuf,
}

impl LoadedConnection {
    pub fn revision(&self) -> String {
        source_revision(&self.source_yaml)
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionRepository {
    connections: BTreeMap<String, LoadedConnection>,
}

impl ConnectionRepository {
    pub fn load(
        root: impl AsRef<Path>,
        appliances: &ConfigRepository,
    ) -> Result<Self, ConfigError> {
        Self::load_with_override(root, appliances, None)
    }

    pub fn load_with_override(
        root: impl AsRef<Path>,
        appliances: &ConfigRepository,
        source_override: Option<(&Path, &str)>,
    ) -> Result<Self, ConfigError> {
        let root = root.as_ref();
        let mut paths = Vec::new();
        collect_yaml_paths(root, &mut paths)?;
        paths.sort();
        if paths.is_empty() {
            return Err(ConfigError::new(format!(
                "{} contains no connection YAML files",
                root.display()
            )));
        }
        let source_base = root
            .parent()
            .and_then(Path::parent)
            .or_else(|| root.parent())
            .unwrap_or(root);
        let mut connections = BTreeMap::new();
        let mut endpoint_pairs = BTreeSet::new();
        let mut point_to_point_endpoints = BTreeMap::new();

        for path in paths {
            let source_yaml = if source_override
                .as_ref()
                .is_some_and(|(override_path, _)| *override_path == path)
            {
                source_override
                    .as_ref()
                    .map(|(_, source)| (*source).to_owned())
                    .unwrap_or_default()
            } else {
                fs::read_to_string(&path).map_err(|error| {
                    ConfigError::new(format!("cannot read {}: {error}", path.display()))
                })?
            };
            let config = ConnectionConfig::from_yaml(&source_yaml)
                .map_err(|error| ConfigError::new(format!("{}: {error}", path.display())))?;
            let expected_file = format!("{}.yaml", config.id);
            if path.file_name().and_then(|name| name.to_str()) != Some(expected_file.as_str()) {
                return Err(ConfigError::new(format!(
                    "{} must be named {}",
                    path.display(),
                    expected_file
                )));
            }
            validate_endpoint(appliances, &config, &config.endpoints.a)?;
            validate_endpoint(appliances, &config, &config.endpoints.b)?;
            validate_endpoint_port(appliances, &config)?;

            let mut pair = [
                format!(
                    "{}:{}",
                    config.endpoints.a.appliance, config.endpoints.a.interface
                ),
                format!(
                    "{}:{}",
                    config.endpoints.b.appliance, config.endpoints.b.interface
                ),
            ];
            pair.sort();
            if !endpoint_pairs.insert(pair) {
                return Err(ConfigError::new(format!(
                    "connection {} duplicates an existing endpoint pair",
                    config.id
                )));
            }
            if config.medium.requires_exclusive_endpoints() {
                for endpoint in [&config.endpoints.a, &config.endpoints.b] {
                    let key = format!("{}:{}", endpoint.appliance, endpoint.interface);
                    if let Some(existing) =
                        point_to_point_endpoints.insert(key.clone(), config.id.clone())
                    {
                        return Err(ConfigError::new(format!(
                            "connection {} reuses point-to-point endpoint {} already assigned to {}",
                            config.id, key, existing
                        )));
                    }
                }
            }

            let id = config.id.clone();
            let source_path = path
                .strip_prefix(source_base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if connections
                .insert(
                    id.clone(),
                    LoadedConnection {
                        config,
                        source_path,
                        source_yaml,
                        source_file: path,
                    },
                )
                .is_some()
            {
                return Err(ConfigError::new(format!("duplicate connection id {id}")));
            }
        }
        validate_redundancy_connections(appliances, &connections)?;
        Ok(Self { connections })
    }

    pub fn get(&self, id: &str) -> Option<&LoadedConnection> {
        self.connections.get(id)
    }

    pub fn connections(&self) -> impl Iterator<Item = &LoadedConnection> {
        self.connections.values()
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    pub fn frontend_connections(&self, appliances: &ConfigRepository) -> Vec<FrontendConnection> {
        self.connections
            .values()
            .map(|connection| FrontendConnection::new(connection, appliances))
            .collect()
    }

    pub fn appliance_index(&self) -> BTreeMap<String, Vec<String>> {
        let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for connection in self.connections.values() {
            for endpoint in [
                &connection.config.endpoints.a,
                &connection.config.endpoints.b,
            ] {
                index
                    .entry(endpoint.appliance.clone())
                    .or_default()
                    .push(connection.config.id.clone());
            }
        }
        index
    }
}

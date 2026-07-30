use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::connection::ConnectionRepository;

use super::{
    ApplianceConfig, ConfigError, FRONTEND_CATALOG_SCHEMA_VERSION, FrontendAppliance,
    FrontendApplianceCatalog, collect_yaml_paths, source_revision,
};

#[derive(Clone, Debug)]
pub struct LoadedAppliance {
    pub config: ApplianceConfig,
    pub source_path: String,
    pub source_yaml: String,
    pub source_file: PathBuf,
}

impl LoadedAppliance {
    pub fn revision(&self) -> String {
        source_revision(&self.source_yaml)
    }
}

#[derive(Clone, Debug)]
pub struct ConfigRepository {
    appliances: BTreeMap<String, LoadedAppliance>,
}

impl ConfigRepository {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, ConfigError> {
        Self::load_with_override(root, None)
    }

    pub fn load_with_override(
        root: impl AsRef<Path>,
        source_override: Option<(&Path, &str)>,
    ) -> Result<Self, ConfigError> {
        let root = root.as_ref();
        let mut paths = Vec::new();
        collect_yaml_paths(root, &mut paths)?;
        paths.sort();
        if paths.is_empty() {
            return Err(ConfigError::new(format!(
                "{} contains no appliance YAML files",
                root.display()
            )));
        }

        let source_base = root
            .parent()
            .and_then(Path::parent)
            .or_else(|| root.parent())
            .unwrap_or(root);
        let mut appliances = BTreeMap::new();
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
            let config =
                ApplianceConfig::from_yaml(&source_yaml).map_err(|error| error.with_path(&path))?;
            let expected_file = format!("{}.yaml", config.id);
            let actual_file = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if actual_file != expected_file {
                return Err(ConfigError::new(format!(
                    "{} must be named {} to preserve one-file-per-appliance identity",
                    path.display(),
                    expected_file
                )));
            }
            let source_path = path
                .strip_prefix(source_base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let id = config.id.clone();
            if appliances
                .insert(
                    id.clone(),
                    LoadedAppliance {
                        config,
                        source_path,
                        source_yaml,
                        source_file: path,
                    },
                )
                .is_some()
            {
                return Err(ConfigError::new(format!("duplicate appliance id {id}")));
            }
        }

        Ok(Self { appliances })
    }

    pub fn appliances(&self) -> impl Iterator<Item = &LoadedAppliance> {
        self.appliances.values()
    }

    pub fn get(&self, id: &str) -> Option<&LoadedAppliance> {
        self.appliances.get(id)
    }

    pub fn len(&self) -> usize {
        self.appliances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.appliances.is_empty()
    }

    pub fn frontend_catalog(&self, connections: &ConnectionRepository) -> FrontendApplianceCatalog {
        let mut node_index: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let appliances = self
            .appliances
            .values()
            .map(|loaded| {
                for binding in &loaded.config.render {
                    let key = format!("{}:{}:{}", binding.view, binding.node, binding.mode);
                    node_index
                        .entry(key)
                        .or_default()
                        .push(loaded.config.id.clone());
                }
                FrontendAppliance::from(loaded)
            })
            .collect();

        FrontendApplianceCatalog {
            schema_version: FRONTEND_CATALOG_SCHEMA_VERSION,
            generation_status: "generated",
            generated_by: "hearthline-engine configuration pipeline",
            appliance_source_root: "config/appliances",
            connection_source_root: "config/connections",
            appliances,
            node_index,
            connections: connections.frontend_connections(self),
            appliance_connection_index: connections.appliance_index(),
        }
    }
}

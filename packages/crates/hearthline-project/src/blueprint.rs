use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use hearthline_model::ComponentId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::ProjectError;
use crate::source::sha256_hex;

pub const BLUEPRINT_SCHEMA_VERSION: &str = "0.1.0";
const PREVIOUS_BLUEPRINT_SCHEMA_VERSION: &str = "0.0.1";

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BlueprintParameterKind {
    Boolean,
    Integer {
        minimum: i64,
        maximum: i64,
    },
    Quantity {
        unit: String,
        scale: i64,
        minimum_raw: i64,
        maximum_raw: i64,
    },
    Identifier,
    Choice {
        values: Vec<String>,
    },
    IdentifierList {
        minimum: usize,
        maximum: usize,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BlueprintParameterValue {
    Boolean(bool),
    Integer(i64),
    Text(String),
    List(Vec<String>),
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintParameter {
    pub id: String,
    pub kind: BlueprintParameterKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<BlueprintParameterValue>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlueprintConnectionMode {
    Single,
    Pairwise,
    FanOut,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintNode {
    pub local_id: String,
    pub family: String,
    #[serde(default)]
    pub repeat_parameter: Option<String>,
    #[serde(default)]
    pub ports: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintConnection {
    pub local_id: String,
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
    pub mode: BlueprintConnectionMode,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintImport {
    pub blueprint: String,
    pub schema_version: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintNestedInstance {
    pub local_id: String,
    pub blueprint: String,
    #[serde(default)]
    pub repeat_parameter: Option<String>,
    #[serde(default)]
    pub values: BTreeMap<String, BlueprintParameterValue>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlueprintExportKind {
    Port,
    Signal,
    MaterialHandoff,
    NetworkConduit,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintExport {
    pub id: String,
    pub kind: BlueprintExportKind,
    pub node: String,
    pub port: String,
    pub contract: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintDefinition {
    pub schema_version: String,
    pub id: String,
    #[serde(default)]
    pub imports: Vec<BlueprintImport>,
    #[serde(default)]
    pub parameters: Vec<BlueprintParameter>,
    #[serde(default)]
    pub nodes: Vec<BlueprintNode>,
    #[serde(default)]
    pub connections: Vec<BlueprintConnection>,
    #[serde(default)]
    pub nested: Vec<BlueprintNestedInstance>,
    #[serde(default)]
    pub exports: Vec<BlueprintExport>,
}

impl BlueprintDefinition {
    pub fn from_yaml(source: &str) -> Result<Self, ProjectError> {
        let mut definition: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ProjectError::Blueprint(error.to_string()))?;
        migrate_schema(&mut definition.schema_version)?;
        validate_definition(&definition)?;
        Ok(definition)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintInstance {
    pub schema_version: String,
    pub id: String,
    pub blueprint: String,
    pub site: String,
    pub environment: String,
    #[serde(default)]
    pub values: BTreeMap<String, BlueprintParameterValue>,
}

impl BlueprintInstance {
    pub fn from_yaml(source: &str) -> Result<Self, ProjectError> {
        let mut instance: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ProjectError::Blueprint(error.to_string()))?;
        migrate_schema(&mut instance.schema_version)?;
        ComponentId::new(&instance.id).map_err(|error| {
            ProjectError::Blueprint(format!("instance {}: {error}", instance.id))
        })?;
        Ok(instance)
    }
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ExpandedBlueprintNode {
    pub id: String,
    pub family: String,
    pub site: String,
    pub environment: String,
    pub ports: Vec<String>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ExpandedBlueprintConnection {
    pub id: String,
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ExpandedBlueprint {
    pub instance: String,
    pub nodes: Vec<ExpandedBlueprintNode>,
    pub connections: Vec<ExpandedBlueprintConnection>,
}

#[derive(Clone, Debug, Default)]
pub struct BlueprintRepository {
    definitions: BTreeMap<String, (BlueprintDefinition, String)>,
    instances: BTreeMap<String, BlueprintInstance>,
}

impl BlueprintRepository {
    pub fn from_sources<'a>(
        definitions: impl IntoIterator<Item = &'a str>,
        instances: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, ProjectError> {
        let mut repository = Self::default();
        for source in definitions {
            let definition = BlueprintDefinition::from_yaml(source)?;
            if repository
                .definitions
                .insert(
                    definition.id.clone(),
                    (definition, sha256(source.as_bytes())),
                )
                .is_some()
            {
                return Err(ProjectError::Blueprint("duplicate blueprint".into()));
            }
        }
        for source in instances {
            let instance = BlueprintInstance::from_yaml(source)?;
            if repository
                .instances
                .insert(instance.id.clone(), instance)
                .is_some()
            {
                return Err(ProjectError::Blueprint(
                    "duplicate blueprint instance".into(),
                ));
            }
        }
        repository.validate_imports()?;
        Ok(repository)
    }

    pub fn load(blueprints: &Path, instances: &Path) -> Result<Self, ProjectError> {
        let mut repository = Self::default();
        for path in yaml_paths(blueprints)? {
            let source = fs::read_to_string(&path).map_err(ProjectError::io)?;
            let definition = BlueprintDefinition::from_yaml(&source)
                .map_err(|error| ProjectError::Blueprint(format!("{}: {error}", path.display())))?;
            let digest = sha256(source.as_bytes());
            if repository
                .definitions
                .insert(definition.id.clone(), (definition, digest))
                .is_some()
            {
                return Err(ProjectError::Blueprint(format!(
                    "duplicate blueprint in {}",
                    path.display()
                )));
            }
        }
        for path in yaml_paths(instances)? {
            let source = fs::read_to_string(&path).map_err(ProjectError::io)?;
            let instance = BlueprintInstance::from_yaml(&source)
                .map_err(|error| ProjectError::Blueprint(format!("{}: {error}", path.display())))?;
            if repository
                .instances
                .insert(instance.id.clone(), instance)
                .is_some()
            {
                return Err(ProjectError::Blueprint(format!(
                    "duplicate blueprint instance in {}",
                    path.display()
                )));
            }
        }
        repository.validate_imports()?;
        Ok(repository)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &BlueprintDefinition> {
        self.definitions.values().map(|(definition, _)| definition)
    }

    pub fn instances(&self) -> impl Iterator<Item = &BlueprintInstance> {
        self.instances.values()
    }

    pub fn expand_all(&self) -> Result<Vec<ExpandedBlueprint>, ProjectError> {
        self.instances
            .values()
            .map(|instance| self.expand(instance))
            .collect()
    }

    pub fn expand(&self, instance: &BlueprintInstance) -> Result<ExpandedBlueprint, ProjectError> {
        let mut active = BTreeSet::new();
        let mut expanded = ExpandedBlueprint {
            instance: instance.id.clone(),
            nodes: Vec::new(),
            connections: Vec::new(),
        };
        self.expand_definition(
            &instance.blueprint,
            &instance.id,
            &instance.site,
            &instance.environment,
            &instance.values,
            &mut active,
            &mut expanded,
        )?;
        expanded.nodes.sort_by(|left, right| left.id.cmp(&right.id));
        expanded
            .connections
            .sort_by(|left, right| left.id.cmp(&right.id));
        Ok(expanded)
    }

    #[allow(clippy::too_many_arguments)]
    fn expand_definition(
        &self,
        blueprint_id: &str,
        namespace: &str,
        site: &str,
        environment: &str,
        supplied: &BTreeMap<String, BlueprintParameterValue>,
        active: &mut BTreeSet<String>,
        output: &mut ExpandedBlueprint,
    ) -> Result<(), ProjectError> {
        if !active.insert(blueprint_id.into()) {
            return Err(ProjectError::Blueprint(format!(
                "blueprint import cycle at {blueprint_id}"
            )));
        }
        let definition = self.definition(blueprint_id)?;
        let values = validated_values(definition, supplied)?;
        let mut node_ids = BTreeMap::<String, Vec<String>>::new();
        for node in &definition.nodes {
            let count = repeat_count(node, &values)?;
            let ids = (0..count)
                .map(|index| {
                    if count == 1 {
                        format!("{namespace}-{}", node.local_id)
                    } else {
                        format!("{namespace}-{}-{:02}", node.local_id, index + 1)
                    }
                })
                .collect::<Vec<_>>();
            for id in &ids {
                ComponentId::new(id).map_err(|error| {
                    ProjectError::Blueprint(format!("expanded node {id}: {error}"))
                })?;
                output.nodes.push(ExpandedBlueprintNode {
                    id: id.clone(),
                    family: node.family.clone(),
                    site: site.into(),
                    environment: environment.into(),
                    ports: node.ports.clone(),
                });
            }
            node_ids.insert(node.local_id.clone(), ids);
        }
        expand_connections(namespace, definition, &node_ids, output)?;
        for nested in &definition.nested {
            let count = optional_repeat_count(
                &nested.local_id,
                nested.repeat_parameter.as_deref(),
                &values,
            )?;
            for index in 0..count {
                let nested_namespace = if count == 1 {
                    format!("{namespace}-{}", nested.local_id)
                } else {
                    format!("{namespace}-{}-{:02}", nested.local_id, index + 1)
                };
                self.expand_definition(
                    &nested.blueprint,
                    &nested_namespace,
                    site,
                    environment,
                    &nested.values,
                    active,
                    output,
                )?;
            }
        }
        active.remove(blueprint_id);
        Ok(())
    }

    fn definition(&self, id: &str) -> Result<&BlueprintDefinition, ProjectError> {
        self.definitions
            .get(id)
            .map(|(definition, _)| definition)
            .ok_or_else(|| ProjectError::Blueprint(format!("unknown blueprint {id}")))
    }

    fn validate_imports(&self) -> Result<(), ProjectError> {
        for (definition, _) in self.definitions.values() {
            for import in &definition.imports {
                self.definitions.get(&import.blueprint).ok_or_else(|| {
                    ProjectError::Blueprint(format!(
                        "blueprint {} imports unknown {}",
                        definition.id, import.blueprint
                    ))
                })?;
                if import.schema_version != BLUEPRINT_SCHEMA_VERSION {
                    return Err(ProjectError::Blueprint(format!(
                        "blueprint {} import {} pins unsupported schema {}",
                        definition.id, import.blueprint, import.schema_version
                    )));
                }
            }
            for nested in &definition.nested {
                if !definition
                    .imports
                    .iter()
                    .any(|import| import.blueprint == nested.blueprint)
                {
                    return Err(ProjectError::Blueprint(format!(
                        "blueprint {} nests {} without a pinned import",
                        definition.id, nested.blueprint
                    )));
                }
            }
        }
        for instance in self.instances.values() {
            self.definition(&instance.blueprint)?;
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for id in self.definitions.keys() {
            validate_import_graph(self, id, &mut visiting, &mut visited)?;
        }
        for (definition, _) in self.definitions.values() {
            for import in &definition.imports {
                let (_, digest) = &self.definitions[&import.blueprint];
                if digest != &import.sha256 {
                    return Err(ProjectError::Blueprint(format!(
                        "blueprint {} import {} digest differs from source",
                        definition.id, import.blueprint
                    )));
                }
            }
        }
        Ok(())
    }
}

mod validation;
use validation::*;

fn expand_connections(
    namespace: &str,
    definition: &BlueprintDefinition,
    node_ids: &BTreeMap<String, Vec<String>>,
    output: &mut ExpandedBlueprint,
) -> Result<(), ProjectError> {
    for connection in &definition.connections {
        let from = &node_ids[&connection.from_node];
        let to = &node_ids[&connection.to_node];
        let pairs = match connection.mode {
            BlueprintConnectionMode::Single if from.len() == 1 && to.len() == 1 => {
                vec![(&from[0], &to[0])]
            }
            BlueprintConnectionMode::Pairwise if from.len() == to.len() => {
                from.iter().zip(to).collect()
            }
            BlueprintConnectionMode::FanOut if from.len() == 1 => {
                to.iter().map(|target| (&from[0], target)).collect()
            }
            _ => {
                return Err(ProjectError::Blueprint(format!(
                    "blueprint {} connection {} has incompatible repetition",
                    definition.id, connection.local_id
                )));
            }
        };
        let multiple = pairs.len() > 1;
        for (index, (from_node, to_node)) in pairs.into_iter().enumerate() {
            output.connections.push(ExpandedBlueprintConnection {
                id: if multiple {
                    format!("{namespace}-{}-{:02}", connection.local_id, index + 1)
                } else {
                    format!("{namespace}-{}", connection.local_id)
                },
                from_node: from_node.clone(),
                from_port: connection.from_port.clone(),
                to_node: to_node.clone(),
                to_port: connection.to_port.clone(),
            });
        }
    }
    Ok(())
}

fn migrate_schema(version: &mut String) -> Result<(), ProjectError> {
    if version == PREVIOUS_BLUEPRINT_SCHEMA_VERSION {
        *version = BLUEPRINT_SCHEMA_VERSION.into();
    }
    if version == BLUEPRINT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ProjectError::Blueprint(format!(
            "unsupported blueprint schema {version}"
        )))
    }
}

fn yaml_paths(root: &Path) -> Result<Vec<PathBuf>, ProjectError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut paths = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .map(|entry| entry.map_err(|error| ProjectError::Io(error.to_string())))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("yaml" | "yml")
            )
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn sha256(bytes: &[u8]) -> String {
    sha256_hex(Sha256::digest(bytes).as_slice())
}

use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

use hearthline_config::{
    APPLIANCE_SCHEMA_VERSION, CONNECTION_SCHEMA_VERSION, ConfigRepository, ConnectionRepository,
    ProcessViewConfig, RUNTIME_CAPACITY_SCHEMA_VERSION, RuntimeCapacityManifest,
    SCENARIO_SCHEMA_VERSION, ScenarioRepository, source_revision,
};
use hearthline_engine::{
    MediaLink, RUNTIME_COMPONENT_HANDLE_BYTES, Simulator, appliance_contracts,
    appliance_family_contract, capacity_budget,
};
use hearthline_model::{BehaviorFamily, PartitionMessage};

use crate::{
    BLUEPRINT_SCHEMA_VERSION, BlueprintRepository, CapacityAssessment, CapacityEvidenceRecord,
    CapacityPlan, CapacityResource, ExpandedBlueprint, MODEL_LOCK_SCHEMA_VERSION, ModelLock,
    ObservedCapacityResource, PartitionAssignment, ProjectRuntimePlan, compile_runtime_plan,
};

mod digest;
use digest::*;

pub const COMPILED_PROJECT_SCHEMA_VERSION: &str = "0.1.0";

#[derive(Clone, Debug)]
pub struct CompiledProject {
    schema_version: &'static str,
    root: PathBuf,
    digest: String,
    appliances: ConfigRepository,
    connections: ConnectionRepository,
    scenarios: ScenarioRepository,
    runtime_plan: ProjectRuntimePlan,
    blueprints: BlueprintRepository,
    expanded_blueprints: Vec<ExpandedBlueprint>,
    capacity: CapacityPlan,
    model_lock: ModelLock,
}

impl CompiledProject {
    pub const fn schema_version(&self) -> &'static str {
        self.schema_version
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn appliances(&self) -> &ConfigRepository {
        &self.appliances
    }

    pub fn connections(&self) -> &ConnectionRepository {
        &self.connections
    }

    pub fn scenarios(&self) -> &ScenarioRepository {
        &self.scenarios
    }

    pub fn runtime_plan(&self) -> &ProjectRuntimePlan {
        &self.runtime_plan
    }

    pub fn blueprints(&self) -> &BlueprintRepository {
        &self.blueprints
    }

    pub fn expanded_blueprints(&self) -> &[ExpandedBlueprint] {
        &self.expanded_blueprints
    }

    pub fn capacity(&self) -> &CapacityPlan {
        &self.capacity
    }

    pub fn model_lock(&self) -> &ModelLock {
        &self.model_lock
    }

    pub fn generated_catalogs(&self) -> Result<Vec<(PathBuf, String)>, ProjectError> {
        let process = ProcessViewConfig::load(
            self.root.join("ot/process/architecture.yaml"),
            &self.appliances,
        )?;
        Ok(vec![
            (
                PathBuf::from("packages/web/src/generated/appliance-configs.json"),
                serde_json::to_string(&self.appliances.frontend_catalog(&self.connections))
                    .map_err(|error| ProjectError::Configuration(error.to_string()))?
                    + "\n",
            ),
            (
                PathBuf::from("packages/web/src/generated/process-view.json"),
                serde_json::to_string(&process.into_frontend(&self.appliances)?)
                    .map_err(|error| ProjectError::Configuration(error.to_string()))?
                    + "\n",
            ),
        ])
    }
}

#[derive(Clone, Debug)]
pub struct ProjectCompiler {
    config_root: PathBuf,
}

impl ProjectCompiler {
    pub fn new(config_root: impl Into<PathBuf>) -> Self {
        Self {
            config_root: config_root.into(),
        }
    }

    pub fn compile(
        &self,
        update_reason: impl Into<String>,
    ) -> Result<CompiledProject, ProjectError> {
        let root = fs::canonicalize(&self.config_root).map_err(ProjectError::io)?;
        let appliances = ConfigRepository::load(root.join("appliances"))?;
        let connections = ConnectionRepository::load(root.join("connections"), &appliances)?;
        let scenarios =
            ScenarioRepository::load(root.join("scenarios"), &appliances, &connections)?;
        ProcessViewConfig::load(root.join("ot/process/architecture.yaml"), &appliances)?;
        let manifest = RuntimeCapacityManifest::load(root.join("runtime/capacity.yaml"))?;
        let runtime_plan = compile_runtime_plan(&appliances, &connections, &manifest)?;
        let blueprints =
            BlueprintRepository::load(&root.join("blueprints"), &root.join("instances"))?;
        let expanded_blueprints = blueprints.expand_all()?;
        let capacity = capacity_plan(
            &runtime_plan,
            &expanded_blueprints,
            &manifest,
            &appliances,
            &connections,
        )?;
        if !capacity.accepted() {
            let detail = capacity
                .rejected()
                .map(|item| {
                    format!(
                        "{:?} in {} has {} of {}",
                        item.resource, item.scope, item.demand, item.reviewed_limit
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ProjectError::Capacity(detail));
        }
        let sources = source_digests(&root)?;
        let objects = object_digests(&appliances, &connections, &scenarios, &expanded_blueprints)?;
        let generated_catalogs = generated_catalog_digests(&root, &appliances, &connections)?;
        let digest = project_digest(&sources, &objects, &expanded_blueprints)?;
        let model_lock = ModelLock {
            schema_version: MODEL_LOCK_SCHEMA_VERSION.into(),
            project_digest: digest.clone(),
            compiler_version: env!("CARGO_PKG_VERSION").into(),
            schema_versions: [
                ("appliance", APPLIANCE_SCHEMA_VERSION),
                ("blueprint", BLUEPRINT_SCHEMA_VERSION),
                ("compiled-project", COMPILED_PROJECT_SCHEMA_VERSION),
                ("connection", CONNECTION_SCHEMA_VERSION),
                ("model-lock", MODEL_LOCK_SCHEMA_VERSION),
                ("runtime-capacity", RUNTIME_CAPACITY_SCHEMA_VERSION),
                ("scenario", SCENARIO_SCHEMA_VERSION),
            ]
            .into_iter()
            .map(|(schema, version)| (schema.into(), version.into()))
            .collect(),
            sources,
            object_digests: objects,
            partition_assignments: runtime_plan
                .partitions
                .iter()
                .map(|partition| PartitionAssignment {
                    site: "canonical-project".into(),
                    cell: partition.id.clone(),
                    source: "direct-graph".into(),
                })
                .chain(expanded_blueprints.iter().map(|blueprint| {
                    let instance = blueprints
                        .instances()
                        .find(|instance| instance.id == blueprint.instance)
                        .expect("expanded blueprint retains source instance");
                    PartitionAssignment {
                        site: instance.site.clone(),
                        cell: instance.id.clone(),
                        source: format!("blueprint:{}", instance.blueprint),
                    }
                }))
                .collect(),
            legacy_partitions: Vec::new(),
            generated_catalogs,
            capacity: capacity.clone(),
            update_reason: update_reason.into(),
        };
        Ok(CompiledProject {
            schema_version: COMPILED_PROJECT_SCHEMA_VERSION,
            root,
            digest,
            appliances,
            connections,
            scenarios,
            runtime_plan,
            blueprints,
            expanded_blueprints,
            capacity,
            model_lock,
        })
    }

    pub fn compile_locked(&self) -> Result<CompiledProject, ProjectError> {
        let compiled = self.compile("locked compilation")?;
        let lock_path = compiled.root.join("model.lock.json");
        let source = fs::read_to_string(&lock_path).map_err(|error| {
            ProjectError::Lock(format!("cannot read {}: {error}", lock_path.display()))
        })?;
        let lock = ModelLock::from_json(&source)?;
        let mut expected = compiled.model_lock.clone();
        expected.update_reason = lock.update_reason.clone();
        if lock != expected {
            let difference = lock
                .first_difference(&expected)
                .unwrap_or_else(|| "unknown field".into());
            return Err(ProjectError::Lock(format!(
                "{} is stale at {difference}; run model lock --update with a review reason",
                lock_path.display(),
            )));
        }
        compiled.verify_generated_catalogs()?;
        Ok(compiled)
    }
}

impl CompiledProject {
    fn verify_generated_catalogs(&self) -> Result<(), ProjectError> {
        let repository = self.root.parent().and_then(Path::parent).ok_or_else(|| {
            ProjectError::Lock("configuration root has no repository parent".into())
        })?;
        for expected in &self.model_lock.generated_catalogs {
            let path = repository.join(&expected.path);
            let source = fs::read_to_string(&path).map_err(|error| {
                ProjectError::Lock(format!(
                    "cannot read generated catalog {}: {error}",
                    path.display()
                ))
            })?;
            if source_revision(&source) != expected.sha256 {
                return Err(ProjectError::Lock(format!(
                    "generated catalog {} differs from compiled model",
                    expected.path
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectError {
    Io(String),
    Configuration(String),
    Blueprint(String),
    Capacity(String),
    Lock(String),
    Transaction(String),
}

impl ProjectError {
    pub(crate) fn io(error: impl Display) -> Self {
        Self::Io(error.to_string())
    }
}

impl Display for ProjectError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(detail) => write!(formatter, "project I/O error: {detail}"),
            Self::Configuration(detail) => {
                write!(formatter, "project configuration error: {detail}")
            }
            Self::Blueprint(detail) => write!(formatter, "blueprint error: {detail}"),
            Self::Capacity(detail) => write!(formatter, "capacity plan rejected: {detail}"),
            Self::Lock(detail) => write!(formatter, "model lock error: {detail}"),
            Self::Transaction(detail) => write!(formatter, "model transaction error: {detail}"),
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<hearthline_config::ConfigError> for ProjectError {
    fn from(error: hearthline_config::ConfigError) -> Self {
        Self::Configuration(error.to_string())
    }
}

fn capacity_plan(
    runtime: &ProjectRuntimePlan,
    blueprints: &[ExpandedBlueprint],
    manifest: &RuntimeCapacityManifest,
    appliances: &ConfigRepository,
    connections: &ConnectionRepository,
) -> Result<CapacityPlan, ProjectError> {
    let mut assessments = Vec::new();
    for observation in &runtime.observations {
        let budget = capacity_budget(observation.resource.engine_key()).ok_or_else(|| {
            ProjectError::Capacity(format!(
                "{:?} has no structural budget",
                observation.resource
            ))
        })?;
        assessments.push(CapacityAssessment::measured(
            map_resource(observation.resource),
            "canonical-project",
            observation.demand,
            budget.nominal_load,
            Some(budget.capacity),
            format!("{:?}", budget.saturation),
            CapacityEvidenceRecord {
                source: "compiled canonical graph".into(),
                detail: budget.rationale.into(),
            },
        ));
    }
    let component_budget =
        capacity_budget("runtime.partition-components").expect("cell component budget");
    let link_budget = capacity_budget("runtime.partition-links").expect("cell link budget");
    for partition in &runtime.partitions {
        for (resource, demand, budget) in [
            (
                CapacityResource::CellComponents,
                partition.component_demand,
                component_budget,
            ),
            (
                CapacityResource::CellLinks,
                partition.internal_link_demand,
                link_budget,
            ),
        ] {
            assessments.push(CapacityAssessment::measured(
                resource,
                partition.id.clone(),
                demand,
                budget.nominal_load,
                Some(budget.capacity),
                format!("{:?}", budget.saturation),
                CapacityEvidenceRecord {
                    source: "compiled canonical cell".into(),
                    detail: budget.rationale.into(),
                },
            ));
        }
    }
    for expanded in blueprints {
        for (resource, demand, budget) in [
            (
                CapacityResource::CellComponents,
                expanded.nodes.len(),
                component_budget,
            ),
            (
                CapacityResource::CellLinks,
                expanded.connections.len(),
                link_budget,
            ),
        ] {
            assessments.push(CapacityAssessment::measured(
                resource,
                expanded.instance.clone(),
                demand,
                budget.nominal_load,
                Some(budget.capacity),
                format!("{:?}", budget.saturation),
                CapacityEvidenceRecord {
                    source: "blueprint expansion".into(),
                    detail: "exact normalized instance demand".into(),
                },
            ));
        }
    }
    let workload = &manifest.workload;
    let reviewed = |limit: usize| {
        limit.saturating_mul(100usize.saturating_sub(manifest.reserve_percent)) / 100
    };
    for scope in [
        "runtime.immediate-events",
        "runtime.delayed-events",
        "runtime.trace",
    ] {
        let budget = capacity_budget(scope).expect("engine event capacity budget");
        let demand = match scope {
            "runtime.immediate-events" => workload.immediate_event_burst,
            "runtime.delayed-events" => workload.delayed_event_burst,
            _ => workload.trace_entry_limit,
        };
        assessments.push(CapacityAssessment::measured(
            CapacityResource::ScheduledEvents,
            scope,
            demand,
            budget.nominal_load,
            Some(budget.capacity),
            format!("{:?}", budget.saturation),
            CapacityEvidenceRecord {
                source: "reviewed workload envelope".into(),
                detail: budget.rationale.into(),
            },
        ));
    }
    for boundary in &runtime.boundaries {
        for direction in ["a-to-b", "b-to-a"] {
            assessments.push(CapacityAssessment::measured(
                CapacityResource::ConduitQueue,
                format!("{}:{direction}", boundary.connection_id),
                workload.conduit_reviewed_burst,
                reviewed(workload.conduit_queue_capacity),
                Some(workload.conduit_queue_capacity),
                workload.conduit_overflow_policy.clone(),
                CapacityEvidenceRecord {
                    source: "compiled boundary and workload envelope".into(),
                    detail: "one preallocated, logically bounded directional conduit".into(),
                },
            ));
        }
    }

    let direct_runtime_bytes = appliances
        .appliances()
        .map(|loaded| {
            appliance_contracts()
                .find(|contract| contract.kind == loaded.config.kind)
                .expect("component kind has family contract")
                .runtime_object_bytes
        })
        .sum::<usize>();
    let blueprint_runtime_bytes = blueprints
        .iter()
        .flat_map(|blueprint| &blueprint.nodes)
        .map(|node| {
            let family = BehaviorFamily::ALL
                .into_iter()
                .find(|family| family.to_string() == node.family)
                .ok_or_else(|| {
                    ProjectError::Capacity(format!(
                        "blueprint node {} names unknown behavior family {}",
                        node.id, node.family
                    ))
                })?;
            Ok(appliance_family_contract(family).runtime_object_bytes)
        })
        .collect::<Result<Vec<_>, ProjectError>>()?
        .into_iter()
        .sum::<usize>();
    let component_overhead = appliances
        .len()
        .saturating_mul(RUNTIME_COMPONENT_HANDLE_BYTES);
    let media_count = connections.len().saturating_add(
        blueprints
            .iter()
            .map(|blueprint| blueprint.connections.len())
            .sum::<usize>(),
    );
    let media_bytes = media_count.saturating_mul(core::mem::size_of::<MediaLink>());
    let directional_conduits = runtime.boundaries.len().saturating_mul(2);
    let envelope_bytes = core::mem::size_of::<PartitionMessage>().saturating_add(320);
    let conduit_bytes = directional_conduits
        .saturating_mul(workload.conduit_queue_capacity)
        .saturating_mul(envelope_bytes);
    let aggregate_memory = direct_runtime_bytes
        .saturating_add(blueprint_runtime_bytes)
        .saturating_add(component_overhead)
        .saturating_add(media_bytes)
        .saturating_add(conduit_bytes);
    let maximum_object = appliance_contracts()
        .map(|contract| contract.runtime_object_bytes)
        .max()
        .unwrap_or_default()
        .max(core::mem::size_of::<Simulator>());
    let stack_demand = maximum_object
        .saturating_add(core::mem::size_of::<Simulator>())
        .saturating_add(64 * 1024);
    assessments.push(CapacityAssessment::measured(
        CapacityResource::RuntimeObjectBytes,
        "largest-cell-runtime-object",
        maximum_object,
        reviewed(workload.loader_stack_limit_bytes),
        Some(workload.loader_stack_limit_bytes),
        "reject-model",
        CapacityEvidenceRecord {
            source: "compiler type layout".into(),
            detail: "maximum registered appliance family or fixed cell simulator object".into(),
        },
    ));
    assessments.push(CapacityAssessment::measured(
        CapacityResource::RuntimeStackBytes,
        "model-loader",
        stack_demand,
        reviewed(workload.loader_stack_limit_bytes),
        Some(workload.loader_stack_limit_bytes),
        "reject-model",
        CapacityEvidenceRecord {
            source: "compiler type layout".into(),
            detail: "largest runtime object, simulator frame, and 64 KiB call-frame allowance"
                .into(),
        },
    ));
    assessments.push(CapacityAssessment::measured(
        CapacityResource::RuntimeMemoryBytes,
        "sealed-session",
        aggregate_memory,
        reviewed(workload.session_memory_limit_bytes),
        Some(workload.session_memory_limit_bytes),
        "reject-model",
        CapacityEvidenceRecord {
            source: "compiled graph and Rust type layout".into(),
            detail: "appliance objects, media, handles, and preallocated conduit envelopes".into(),
        },
    ));
    assessments
        .sort_by(|left, right| (left.resource, &left.scope).cmp(&(right.resource, &right.scope)));
    Ok(CapacityPlan { assessments })
}

fn map_resource(resource: ObservedCapacityResource) -> CapacityResource {
    use ObservedCapacityResource as Observed;
    match resource {
        Observed::ProcessPorts => CapacityResource::ProcessPorts,
        Observed::ProcessTags => CapacityResource::ProcessTags,
        Observed::HmiCommandTags => CapacityResource::HmiCommandTags,
        Observed::AppliancePorts => CapacityResource::AppliancePorts,
        Observed::SwitchPorts => CapacityResource::SwitchPorts,
        Observed::Layer3SwitchPorts => CapacityResource::Layer3SwitchPorts,
        Observed::UnaddressedPorts => CapacityResource::UnaddressedPorts,
        Observed::CellComponents => CapacityResource::CellComponents,
        Observed::CellLinks => CapacityResource::CellLinks,
    }
}

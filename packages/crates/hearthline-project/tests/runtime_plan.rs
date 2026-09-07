use std::path::PathBuf;

use hearthline_config::{
    ConfigRepository, ConnectionRepository, RuntimeCapacityManifest, RuntimePartitionRule,
};
use hearthline_engine::RUNTIME_CAPACITY_BUDGETS;
use hearthline_project::{ObservedCapacityResource, compile_runtime_plan};

fn project_config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config")
}

fn repositories() -> (
    ConfigRepository,
    ConnectionRepository,
    RuntimeCapacityManifest,
) {
    let config = project_config();
    let appliances =
        ConfigRepository::load(config.join("appliances")).expect("appliance repository");
    let connections = ConnectionRepository::load(config.join("connections"), &appliances)
        .expect("connection repository");
    let manifest = RuntimeCapacityManifest::load(config.join("runtime/capacity.yaml"))
        .expect("runtime capacity manifest");
    (appliances, connections, manifest)
}

#[test]
fn canonical_project_graph_fits_its_reviewed_runtime_capacity() {
    let (appliances, connections, manifest) = repositories();
    let plan = compile_runtime_plan(&appliances, &connections, &manifest)
        .expect("feasible project runtime plan");
    assert_eq!(
        plan.partitions
            .iter()
            .map(|partition| partition.component_demand)
            .sum::<usize>(),
        appliances.len()
    );
    assert_eq!(
        plan.partitions
            .iter()
            .map(|partition| partition.internal_link_demand)
            .sum::<usize>()
            + plan.boundaries.len(),
        connections.len()
    );
    assert!(!plan.boundaries.is_empty());
}

#[test]
fn every_runtime_capacity_has_unique_evidence_headroom_and_saturation_behavior() {
    let mut resources = std::collections::BTreeSet::new();
    for budget in RUNTIME_CAPACITY_BUDGETS {
        assert!(
            resources.insert(budget.resource),
            "duplicate capacity budget"
        );
        assert!(
            budget.has_required_headroom(),
            "{} headroom",
            budget.resource
        );
        assert!(budget.reserve() > 0, "{} reserve", budget.resource);
    }
}

#[test]
fn canonical_capacity_observations_are_compiler_generated() {
    let (appliances, connections, manifest) = repositories();
    let plan =
        compile_runtime_plan(&appliances, &connections, &manifest).expect("capacity measurements");
    assert_eq!(
        plan.observations
            .iter()
            .find(|item| item.resource == ObservedCapacityResource::ProcessTags)
            .expect("process tag measurement")
            .demand,
        26
    );
}

#[test]
fn runtime_plan_rejects_overlapping_unmatched_and_invalid_partition_rules() {
    let (appliances, connections, canonical) = repositories();

    let mut overlapping = canonical.clone();
    let mut duplicate_match = overlapping.partitioning.rules[0].clone();
    duplicate_match.id = "second-water-treatment-match".into();
    overlapping.partitioning.rules.push(duplicate_match);
    let error = compile_runtime_plan(&appliances, &connections, &overlapping)
        .expect_err("overlapping partition rules");
    assert!(
        error
            .to_string()
            .contains("matches multiple runtime partition rules")
    );

    let mut unmatched = canonical.clone();
    unmatched.partitioning.rules.push(RuntimePartitionRule {
        id: "missing-appliance-rule".into(),
        site: "Factory".into(),
        environment: "Body Preparation".into(),
        appliance_ids: vec!["missing-appliance".into()],
        appliance_prefixes: Vec::new(),
    });
    let error = compile_runtime_plan(&appliances, &connections, &unmatched)
        .expect_err("unmatched partition rule");
    assert!(error.to_string().contains("matches no canonical appliance"));

    let mut invalid_id = canonical;
    invalid_id.partitioning.rules[0].id = "INVALID CELL ID".into();
    let error = compile_runtime_plan(&appliances, &connections, &invalid_id)
        .expect_err("invalid generated cell ID");
    assert!(
        error
            .to_string()
            .contains("is not a valid model identifier")
    );
}

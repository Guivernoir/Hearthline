use hearthline_config::physics::capacity::{
    AuxiliaryResource, AuxiliaryStorage, CapacityKernel, CapacityProfile, PhysicsCapacityConfig,
    PhysicsPopulation, PopulationOverflow, PopulationWorkload, StorageOverflow,
};
use hearthline_project::physics::capacity::{PhysicsCapacityReport, population_demand};
use hearthline_project::physics::{DEFAULT_PHYSICS_BUDGET_BYTES, PhysicsStorageLayout};

fn config() -> PhysicsCapacityConfig {
    PhysicsCapacityConfig {
        schema_version: "0.1.0".into(),
        id: "factory-study".into(),
        allocation_budget_bytes: DEFAULT_PHYSICS_BUDGET_BYTES,
        reserve_percent: 25,
        snapshot_copies: 2,
        profiles: vec![CapacityProfile {
            id: "specimen".into(),
            shape: [3; 3],
            kernel: CapacityKernel::DryingElasticity,
            model: None,
            basis: "Verification specimen, not a qualified production mesh".into(),
        }],
        populations: vec![PhysicsPopulation {
            id: "drying-products".into(),
            area: "controlled-drying".into(),
            profile: "specimen".into(),
            capacity: 1600,
            workload: PopulationWorkload::Production {
                sources: 4,
                minimum_cycle_us: 900_000_000,
                maximum_residence_us: 259_200_000_000,
                simultaneous_burst: 4,
            },
            overflow: PopulationOverflow::Backpressure,
            basis: "Synthetic 15-minute cycle, 72-hour residence, four-piece burst".into(),
        }],
        auxiliary: AuxiliaryResource::ALL
            .into_iter()
            .map(|resource| AuxiliaryStorage {
                resource,
                peak_entries: 3,
                capacity_entries: 4,
                bytes_per_entry: 1024,
                overflow: match resource {
                    AuxiliaryResource::TransferQueues | AuxiliaryResource::Historian => {
                        StorageOverflow::Backpressure
                    }
                    AuxiliaryResource::Projections => StorageOverflow::DropOldest,
                    _ => StorageOverflow::RejectStart,
                },
                basis: "Synthetic test envelope".into(),
            })
            .collect(),
    }
}

#[test]
fn storage_layouts_match_implemented_array_payloads() {
    let scalar = PhysicsStorageLayout::compile([3; 3], CapacityKernel::ScalarTransport).unwrap();
    assert_eq!(scalar.cells, 27);
    assert_eq!(scalar.state_bytes, 27 * 16);
    assert_eq!(scalar.checkpoint_field_bytes, scalar.state_bytes);
    assert_eq!(scalar.instance_buffer_bytes().unwrap(), 27 * 96);
    let flow = PhysicsStorageLayout::compile([6, 2, 2], CapacityKernel::CompressibleFlow).unwrap();
    assert_eq!(flow.state_bytes, 24 * 40);
    assert_eq!(flow.checkpoint_field_bytes, 24 * 56);
    assert_eq!(flow.instance_buffer_bytes().unwrap(), 24 * 232);
    let solid = PhysicsStorageLayout::compile([3; 3], CapacityKernel::DryingElasticity).unwrap();
    assert_eq!((solid.nodes, solid.elements), (64, 162));
    assert_eq!(solid.state_bytes, 27 * 16 + 64 * 3 * 8);
    assert_eq!(
        solid.checkpoint_field_bytes,
        solid.state_bytes + 162 * 6 * 8
    );
    assert_eq!(
        solid.instance_buffer_bytes().unwrap(),
        27 * 96 + 64 * 3 * 9 * 8 + 162 * 13 * 8
    );
    assert!(solid.immutable_geometry_bytes > 0);
}

#[test]
fn production_demand_counts_synchronized_sources_and_multiday_residence() {
    let report = PhysicsCapacityReport::compile(config()).unwrap();
    assert_eq!(report.digest.len(), 64);
    assert_eq!(report.populations[0].peak_instances, 1156);
    assert_eq!(report.populations[0].reviewed_limit, 1200);
    report.require_fit().unwrap();
    let fractional = PopulationWorkload::Production {
        sources: 4,
        minimum_cycle_us: 10,
        maximum_residence_us: 11,
        simultaneous_burst: 1,
    };
    assert_eq!(population_demand(fractional).unwrap(), 9);
    assert_eq!(
        population_demand(PopulationWorkload::Fixed { count: 4 }).unwrap(),
        4
    );
}

#[test]
fn geometry_is_shared_but_products_scratch_and_snapshots_are_independent() {
    let first = PhysicsCapacityReport::compile(config()).unwrap();
    let mut input = config();
    let mut second = input.populations[0].clone();
    second.id = "dryer-products".into();
    second.area = "industrial-dryer".into();
    input.populations.push(second);
    let doubled = PhysicsCapacityReport::compile(input).unwrap();
    assert_eq!(
        first.immutable_geometry_bytes,
        doubled.immutable_geometry_bytes
    );
    assert_eq!(
        first.allocated_instance_bytes * 2,
        doubled.allocated_instance_bytes
    );
    assert_eq!(first.snapshot_field_bytes * 2, doubled.snapshot_field_bytes);
    assert_eq!(
        first.auxiliary_envelope_bytes,
        doubled.auxiliary_envelope_bytes
    );
    assert!(!first.qualification_gaps.is_empty());
}

#[test]
fn input_order_does_not_change_report_or_digest() {
    let mut input = config();
    let mut profile = input.profiles[0].clone();
    profile.id = "receiver".into();
    profile.kernel = CapacityKernel::CompressibleFlow;
    input.profiles.push(profile);
    let mut population = input.populations[0].clone();
    population.id = "receivers".into();
    population.profile = "receiver".into();
    input.populations.push(population);
    let original = PhysicsCapacityReport::compile(input.clone()).unwrap();
    input.profiles.reverse();
    input.populations.reverse();
    input.auxiliary.reverse();
    assert_eq!(
        original,
        PhysicsCapacityReport::compile(input.clone()).unwrap()
    );
    input.populations[0].basis.push_str(" Updated review");
    assert_ne!(
        original.digest,
        PhysicsCapacityReport::compile(input).unwrap().digest
    );
}

#[test]
fn reserve_failures_report_without_reducing_meshes_or_populations() {
    let mut input = config();
    input.populations[0].capacity = 1156;
    let report = PhysicsCapacityReport::compile(input).unwrap();
    assert!(!report.within_population_capacities);
    assert_eq!(report.populations[0].peak_instances, 1156);
    assert!(report.require_fit().is_err());

    let mut input = config();
    input.allocation_budget_bytes = DEFAULT_PHYSICS_BUDGET_BYTES + 1;
    assert!(PhysicsCapacityReport::compile(input.clone()).is_err());
    assert!(
        PhysicsCapacityReport::compile_with_budget(input, DEFAULT_PHYSICS_BUDGET_BYTES + 1).is_ok()
    );
    let mut input = config();
    input.allocation_budget_bytes = 10_000;
    let report = PhysicsCapacityReport::compile(input).unwrap();
    assert!(!report.within_budget);
    assert!(report.require_fit().is_err());
    assert_eq!(report.profiles[0].profile.shape, [3; 3]);
    let mut input = config();
    input.auxiliary[0].peak_entries = 4;
    let report = PhysicsCapacityReport::compile(input).unwrap();
    assert!(!report.within_auxiliary_capacities);
    assert!(report.require_fit().is_err());
}

#[test]
fn missing_evidence_duplicate_ids_and_unsafe_policies_are_rejected() {
    let check = |edit: fn(&mut PhysicsCapacityConfig)| {
        let mut input = config();
        edit(&mut input);
        assert!(PhysicsCapacityReport::compile(input).is_err());
    };
    for edit in [
        (|c: &mut PhysicsCapacityConfig| {
            c.auxiliary.pop();
        }) as fn(&mut PhysicsCapacityConfig),
        |c| c.auxiliary.push(c.auxiliary[0].clone()),
        |c| c.profiles.push(c.profiles[0].clone()),
        |c| {
            let mut unused = c.profiles[0].clone();
            unused.id = "unused".into();
            c.profiles.push(unused);
        },
        |c| c.populations.push(c.populations[0].clone()),
        |c| c.profiles.clear(),
        |c| c.populations.clear(),
        |c| c.profiles[0].basis.clear(),
        |c| c.populations[0].basis.clear(),
        |c| c.auxiliary[0].basis.clear(),
        |c| c.populations[0].profile = "missing".into(),
        |c| c.profiles[0].id = "invalid/id".into(),
        |c| c.id = "invalid/id".into(),
        |c| c.populations[0].area.clear(),
        |c| c.schema_version = "99.0.0".into(),
        |c| c.reserve_percent = 24,
        |c| c.reserve_percent = 100,
        |c| c.snapshot_copies = 0,
        |c| c.allocation_budget_bytes = 0,
        |c| c.allocation_budget_bytes = DEFAULT_PHYSICS_BUDGET_BYTES + 1,
        |c| c.populations[0].capacity = 0,
        |c| c.auxiliary[0].bytes_per_entry = 0,
        |c| c.auxiliary[0].peak_entries = 0,
        |c| c.auxiliary[0].capacity_entries = 0,
        |c| c.auxiliary[0].overflow = StorageOverflow::DropOldest,
    ] {
        check(edit);
    }
}

#[test]
fn overflow_is_an_error_not_a_saturated_memory_estimate() {
    assert!(
        PhysicsStorageLayout::compile([u32::MAX; 3], CapacityKernel::CompressibleFlow).is_err()
    );
    assert!(PhysicsStorageLayout::compile([0; 3], CapacityKernel::ScalarTransport).is_err());
    assert!(
        population_demand(PopulationWorkload::Production {
            sources: 4,
            minimum_cycle_us: 1,
            maximum_residence_us: u64::MAX,
            simultaneous_burst: 0,
        })
        .is_err()
    );
    assert!(
        population_demand(PopulationWorkload::Production {
            sources: 1,
            minimum_cycle_us: 0,
            maximum_residence_us: 1,
            simultaneous_burst: 0,
        })
        .is_err()
    );
    assert!(population_demand(PopulationWorkload::Fixed { count: 0 }).is_err());
    let mut input = config();
    input.populations[0].capacity = u64::MAX;
    assert!(PhysicsCapacityReport::compile(input).is_err());
    let mut input = config();
    input.auxiliary[0].bytes_per_entry = u64::MAX;
    assert!(PhysicsCapacityReport::compile(input).is_err());
}

#[test]
fn strict_schema_rejects_unknown_resource_fields() {
    let mut value = serde_json::to_value(config()).unwrap();
    value["populations"][0]["auto_reduce_fidelity"] = true.into();
    assert!(serde_json::from_value::<PhysicsCapacityConfig>(value).is_err());
}

#[test]
fn full_factory_baseline_exposes_the_current_memory_failure() {
    let input: PhysicsCapacityConfig = serde_yaml_ng::from_str(include_str!(
        "../../../../project/physics/capacity/factory-baseline.yaml"
    ))
    .unwrap();
    let report = PhysicsCapacityReport::compile(input).unwrap();
    assert!(report.within_population_capacities);
    assert!(report.within_auxiliary_capacities);
    assert!(!report.within_budget);
    assert!(report.require_fit().is_err());
    assert!(report.total_planned_bytes > DEFAULT_PHYSICS_BUDGET_BYTES);
    assert_eq!(report.immutable_geometry_bytes, 1_544_040);
    assert_eq!(report.allocated_instance_bytes, 4_309_216_176);
    assert_eq!(report.snapshot_field_bytes, 2_877_994_080);
    assert_eq!(report.auxiliary_envelope_bytes, 184_549_376);
    assert_eq!(report.total_planned_bytes, 7_373_303_672);
    assert_eq!(
        report
            .populations
            .iter()
            .find(|p| p.population.id == "controlled-drying-products")
            .unwrap()
            .peak_instances,
        1156
    );
    assert_eq!(report.profiles.len(), 5);
    assert_eq!(report.populations.len(), 21);
}

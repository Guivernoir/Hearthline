use hearthline_model::ComponentId;
use hearthline_physics::Scalar;
use hearthline_physics::flow::connected::{
    CellInventory, ConnectedFlow, ConnectedWorkspace, FlowCell,
};
use hearthline_physics::flow::{FlowBoundary, IdealGas, Primitive, ViscousTransport};
use hearthline_physics::transport::{Grid3d, SolverLimits};
use hearthline_project::physics::flow::mesh::{
    CompiledFlowMesh, FlowPatch, FlowPatchConnection, FlowRegion,
};
use hearthline_project::physics::schedule::PhysicsInstanceIdentity;
use sha2::{Digest, Sha256};

const BUDGET: u64 = 16 * 1024 * 1024;
fn s(value: &str) -> Scalar {
    value.parse().unwrap()
}
fn identity(name: &str) -> PhysicsInstanceIdentity {
    PhysicsInstanceIdentity::new("factory", "air", name).unwrap()
}
fn region(name: &str, width: Scalar) -> FlowRegion {
    FlowRegion {
        identity: identity(name),
        grid: Grid3d::new([2, 2, 2], [width, Scalar::ONE, Scalar::ONE]).unwrap(),
        boundaries: [FlowBoundary::AdiabaticSlipWall; 6],
    }
}
fn connection(id: &str, a: &str, b: &str, axis: u8) -> FlowPatchConnection {
    FlowPatchConnection {
        id: ComponentId::new(id).unwrap(),
        first: FlowPatch {
            region: identity(a),
            face: 2 * axis + 1,
        },
        second: FlowPatch {
            region: identity(b),
            face: 2 * axis,
        },
    }
}
fn inputs() -> (Vec<FlowRegion>, Vec<FlowPatchConnection>) {
    (
        vec![
            region("supply", s("0.37")),
            region("pipe", s("0.71")),
            region("receiver", s("1.13")),
        ],
        vec![
            connection("supply-pipe", "supply", "pipe", 0),
            connection("pipe-receiver", "pipe", "receiver", 0),
        ],
    )
}

#[test]
fn source_order_and_endpoint_spelling_do_not_change_mesh_or_digest() {
    let (mut regions, mut connections) = inputs();
    let a = CompiledFlowMesh::compile(regions.clone(), connections.clone(), BUDGET).unwrap();
    regions.reverse();
    connections.reverse();
    for connection in &mut connections {
        core::mem::swap(&mut connection.first, &mut connection.second);
    }
    let b = CompiledFlowMesh::compile(regions, connections, BUDGET).unwrap();
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.mesh().unwrap().cells(), b.mesh().unwrap().cells());
    assert_eq!(
        a.mesh().unwrap().boundaries(),
        b.mesh().unwrap().boundaries()
    );
    assert_eq!(a.regions().len(), 3);
    assert_eq!(a.connections().len(), 2);
    assert_eq!(a.mesh().unwrap().cells().len(), 24);
    let (regions, connections) = inputs();
    let smaller_budget = CompiledFlowMesh::compile(regions, connections, BUDGET / 2).unwrap();
    assert_eq!(
        a.digest(),
        smaller_budget.digest(),
        "topology identity does not include host resource accounting"
    );
}

#[test]
fn invalid_connections_are_rejected_instead_of_becoming_implicit_supplies() {
    let (regions, connections) = inputs();
    let mut duplicate = regions.clone();
    duplicate.push(regions[0].clone());
    assert!(CompiledFlowMesh::compile(duplicate, connections.clone(), BUDGET).is_err());
    let mut duplicate = connections.clone();
    duplicate.push(connections[0].clone());
    assert!(CompiledFlowMesh::compile(regions.clone(), duplicate, BUDGET).is_err());
    for mutate in 0..5 {
        let mut bad = connections.clone();
        match mutate {
            0 => bad[0].first.region = identity("missing"),
            1 => bad[0].first.face = 6,
            2 => bad[0].first.face = bad[0].second.face,
            3 => {
                let mut extra = bad[0].clone();
                extra.id = ComponentId::new("duplicate-patch").unwrap();
                bad.push(extra);
            }
            _ => bad[0].second.face = 4,
        }
        assert!(CompiledFlowMesh::compile(regions.clone(), bad, BUDGET).is_err());
    }
    for shape in [[2, 3, 2], [2, 2, 3]] {
        let mut bad = regions.clone();
        bad[0].grid = Grid3d::new(shape, [Scalar::ONE; 3]).unwrap();
        assert!(CompiledFlowMesh::compile(bad, connections.clone(), BUDGET).is_err());
    }
    let mut bad = regions.clone();
    bad[0].grid = Grid3d::new([2; 3], [Scalar::ONE, s("2"), Scalar::ONE]).unwrap();
    assert!(CompiledFlowMesh::compile(bad, connections, BUDGET).is_err());
}

#[test]
fn capacity_preflights_fields_and_compiler_inputs_before_mesh_allocation() {
    let (regions, connections) = inputs();
    assert!(CompiledFlowMesh::compile(regions.clone(), connections.clone(), 1).is_err());
    let model = CompiledFlowMesh::compile(regions, connections, BUDGET).unwrap();
    let plan = model.memory();
    assert_eq!(plan.field_bytes, 24 * 3 * size_of::<CellInventory>() as u64);
    assert!(plan.immutable_bytes >= 24 * size_of::<FlowCell>() as u64);
    assert_eq!(
        plan.peak_planned_bytes,
        plan.immutable_bytes + plan.field_bytes.max(plan.compiler_input_bytes)
    );
    assert!(plan.peak_planned_bytes * 4 <= plan.allocation_budget_bytes * 3);
    let mut huge = region("huge", Scalar::ONE);
    huge.grid = Grid3d::new([1_000_000_000, 1, 1], [Scalar::ONE; 3]).unwrap();
    assert!(CompiledFlowMesh::compile(vec![huge], vec![], BUDGET).is_err());
}

#[test]
fn measured_compilation_and_field_storage_fit_the_declared_peak() {
    let mut planned = 0;
    let allocation = allocation_counter::measure(|| {
        let (regions, connections) = inputs();
        let model = CompiledFlowMesh::compile(regions, connections, BUDGET).unwrap();
        planned = model.memory().peak_planned_bytes;
        let fields = [
            vec![CellInventory::default(); model.memory().cells as usize],
            vec![CellInventory::default(); model.memory().cells as usize],
            vec![CellInventory::default(); model.memory().cells as usize],
        ];
        std::hint::black_box((&model, &fields));
    });
    assert!(
        allocation.bytes_max <= planned,
        "measured {} > planned {planned}",
        allocation.bytes_max
    );
}

#[test]
fn each_axis_connects_the_correct_exterior_patch_and_uniform_state_stays_stationary() {
    let gas = IdealGas::new(s("1.4")).unwrap();
    for axis in 0..3 {
        let regions = vec![region("a", Scalar::ONE), region("b", Scalar::ONE)];
        let model =
            CompiledFlowMesh::compile(regions, vec![connection("link", "a", "b", axis)], BUDGET)
                .unwrap();
        let mesh = model.mesh().unwrap();
        let limits = SolverLimits {
            iterations: 100,
            residual: Scalar::from_raw(1),
        };
        let solver = ConnectedFlow::new(mesh, gas, limits, None).unwrap();
        let value = gas
            .conserved(Primitive {
                density: Scalar::ONE,
                pressure: Scalar::ONE,
                velocity: [Scalar::ZERO; 3],
            })
            .unwrap();
        let mut state: Vec<_> = mesh
            .cells()
            .iter()
            .map(|cell| CellInventory::from_density(value, cell.volume()).unwrap())
            .collect();
        let original = state.clone();
        let mut trial = state.clone();
        let mut next = state.clone();
        solver
            .solve(
                s("0.01"),
                &mut state,
                &mut ConnectedWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
        assert_eq!(state, original);
        let first = &model.regions()[0];
        let mut crossing = 0;
        for local in 0..first.grid.count() {
            if first
                .grid
                .face_neighbor(local, usize::from(axis) * 2 + 1)
                .unwrap()
                .is_none()
            {
                let hearthline_physics::flow::connected::FaceLink::Cell(peer) =
                    mesh.cells()[first.offset + local].faces[usize::from(axis) * 2 + 1]
                else {
                    panic!("unconnected patch");
                };
                assert!(peer as usize >= model.regions()[1].offset);
                assert_eq!(
                    peer as usize,
                    model.regions()[1].offset + local - [1, 2, 4][usize::from(axis)]
                );
                crossing += 1;
            }
        }
        assert_eq!(crossing, 4);
    }
}

#[test]
fn named_three_dimensional_regions_exchange_finite_inventory_and_recompute_exactly() {
    let (regions, connections) = inputs();
    let model = CompiledFlowMesh::compile(regions, connections, BUDGET).unwrap();
    let mesh = model.mesh().unwrap();
    let gas = IdealGas::new(s("1.4")).unwrap();
    let transport = ViscousTransport::new(s("0.01"), s("0.02"), Scalar::ONE).unwrap();
    let solver = ConnectedFlow::new(
        mesh,
        gas,
        SolverLimits {
            iterations: 400,
            residual: s("0.00000005"),
        },
        Some(transport),
    )
    .unwrap();
    let mut state = vec![CellInventory::default(); mesh.cells().len()];
    for region in model.regions() {
        let pressure = if region.identity.instance.as_str() == "supply" {
            s("2")
        } else {
            Scalar::ONE
        };
        let value = gas
            .conserved(Primitive {
                density: pressure,
                pressure,
                velocity: [Scalar::ZERO; 3],
            })
            .unwrap();
        for cell in &mut state[region.offset..region.offset + region.grid.count()] {
            *cell = CellInventory::from_density(value, region.grid.volume().unwrap()).unwrap();
        }
    }
    let original = state.clone();
    let inventory = solver.inventory(&state).unwrap();
    let mut trial = state.clone();
    let mut next = state.clone();
    let mut checkpoint = Vec::new();
    let dt = s("0.001");
    for event in 0..20 {
        let report = solver
            .solve(
                dt,
                &mut state,
                &mut ConnectedWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
        assert_eq!(report.balance_error, CellInventory::default());
        assert_eq!(solver.inventory(&state).unwrap().mass, inventory.mass);
        assert_eq!(solver.inventory(&state).unwrap().energy, inventory.energy);
        if event == 7 {
            checkpoint = state.clone();
        }
    }
    for _ in 8..20 {
        solver
            .solve(
                dt,
                &mut checkpoint,
                &mut ConnectedWorkspace {
                    trial: &mut trial,
                    next: &mut next,
                },
            )
            .unwrap();
    }
    assert_eq!(state, checkpoint);
    for region in model.regions() {
        let range = region.offset..region.offset + region.grid.count();
        let before: i64 = original[range.clone()].iter().map(|v| v.mass.raw()).sum();
        let after: i64 = state[range].iter().map(|v| v.mass.raw()).sum();
        if region.identity.instance.as_str() == "supply" {
            assert!(after < before);
        } else {
            assert!(after > before);
        }
    }
    let mut hash = Sha256::new();
    hash.update(model.digest());
    for cell in state {
        for value in cell.components() {
            hash.update(value.raw().to_be_bytes());
        }
    }
    let digest: String = hash.finalize().iter().map(|v| format!("{v:02x}")).collect();
    assert_eq!(
        digest,
        "1c85d0ca8ceef5b02d4b4b9ae8c0ca0dccef2c486e92e55f97ccd934e4bbb3dd"
    );
}

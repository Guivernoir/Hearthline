use hearthline_config::physics::geometry::{
    GeometryReference, SolidGeometry, SolidGeometryDocument,
};
use hearthline_config::physics::{PhysicsProofCase, PhysicsProofConfig};
use hearthline_physics::Scalar;
use hearthline_physics::transport::TransportGrid;
use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_project::physics::geometry::geometry_digest;

const DOCUMENT: &str = include_str!("../../../../project/physics/geometry/original-basin.yaml");
const PROOF: &str = include_str!("../../../../project/physics/proofs/drying-deformation.yaml");

fn inputs() -> (PhysicsProofConfig, SolidGeometryDocument) {
    let geometry: SolidGeometryDocument = serde_yaml_ng::from_str(DOCUMENT).unwrap();
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(PROOF).unwrap();
    config.shape = [24, 18, 9];
    config.geometry = Some(GeometryReference {
        id: geometry.id.clone(),
        sha256: geometry_digest(&geometry).unwrap(),
    });
    (config, geometry)
}

#[test]
fn original_basin_has_exact_volume_surface_area_and_only_active_solid_nodes() {
    let (config, geometry) = inputs();
    let model = CompiledPhysicsProof::compile_with_geometry(config, Some(geometry)).unwrap();
    let domain = model.solid_domain().unwrap();
    assert_eq!(model.cell_count(), 1072);
    assert_eq!(domain.plan().nodes, 2230);
    let volume = model.grid().volume().unwrap();
    assert_eq!(
        volume.checked_mul(Scalar::integer(1072).unwrap()).unwrap(),
        "0.008576".parse().unwrap()
    );
    let mut area = Scalar::ZERO;
    for cell in 0..model.cell_count() {
        for (face, exposed) in domain.exposed_faces(cell).unwrap().into_iter().enumerate() {
            if exposed {
                area = area
                    .checked_add(
                        volume
                            .checked_div(model.grid().spacing()[face / 2])
                            .unwrap(),
                    )
                    .unwrap();
            }
        }
    }
    assert_eq!(area, "0.8912".parse().unwrap());
    let mechanics = model.mechanics().unwrap();
    assert_eq!(mechanics.positions().len(), 2230);
    assert_eq!(mechanics.elements().len(), 6432);
    let mut used = vec![false; mechanics.positions().len()];
    for element in mechanics.elements() {
        assert!(element.volume() > Scalar::ZERO);
        for node in element.nodes() {
            used[node] = true;
        }
    }
    assert!(used.into_iter().all(|used| used));
    assert!(matches!(
        model.transport_grid().unwrap(),
        TransportGrid::Sparse(_)
    ));
    assert!(model.memory().compiler_scratch_bytes >= 6432 * 144 * 20);
    assert!(model.memory().compiler_scratch_bytes > model.memory().field_buffer_bytes);
    assert_eq!(
        mechanics.stiffness().unwrap().unwrap().dimension(),
        2230 * 3
    );
    assert_eq!(
        model.memory().reserve_bytes
            + model.memory().compiler_scratch_bytes
            + model.memory().immutable_mesh_bytes,
        model.memory().allocation_budget_bytes
    );
}

#[test]
fn refinement_preserves_geometry_and_stable_cell_numbering() {
    let (mut config, geometry) = inputs();
    if let PhysicsProofCase::DryingSpecimen { mechanics, .. } = &mut config.case {
        *mechanics = None;
    }
    let first = CompiledPhysicsProof::compile_with_geometry(config.clone(), Some(geometry.clone()))
        .unwrap();
    config.shape = config.shape.map(|axis| axis * 2);
    config.spacing_m = ["0.01".into(), "0.01".into(), "0.01".into()];
    let second = CompiledPhysicsProof::compile_with_geometry(config, Some(geometry)).unwrap();
    assert_eq!(second.cell_count(), 8 * first.cell_count());
    for model in [&first, &second] {
        let ids = model.solid_domain().unwrap().lattice_ids();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        let mass_volume = model
            .grid()
            .volume()
            .unwrap()
            .checked_mul(Scalar::integer(model.cell_count() as i64).unwrap())
            .unwrap();
        assert_eq!(mass_volume, "0.008576".parse().unwrap());
    }
}

#[test]
fn mismatched_geometry_and_unresolved_walls_are_rejected() {
    let (config, geometry) = inputs();
    assert!(CompiledPhysicsProof::compile(config.clone()).is_err());
    let mut missing = config.clone();
    missing.geometry = None;
    assert!(CompiledPhysicsProof::compile_with_geometry(missing, Some(geometry.clone())).is_err());
    let mut bad = geometry.clone();
    bad.id.push_str("-different");
    assert!(
        CompiledPhysicsProof::compile_with_geometry(config.clone(), Some(bad))
            .unwrap_err()
            .contains("digest mismatch")
    );
    for wall in ["0", "-1", "0.03", "0.24", "1000000000"] {
        let mut bad = geometry.clone();
        let SolidGeometry::OpenBasin {
            wall_thickness_m, ..
        } = &mut bad.solid;
        wall_thickness_m[0] = wall.into();
        let mut config = config.clone();
        config.geometry.as_mut().unwrap().sha256 = geometry_digest(&bad).unwrap();
        assert!(
            CompiledPhysicsProof::compile_with_geometry(config, Some(bad)).is_err(),
            "accepted {wall}"
        );
    }
    let mut bad = config;
    bad.shape[0] -= 1;
    assert!(
        CompiledPhysicsProof::compile_with_geometry(bad, Some(geometry))
            .unwrap_err()
            .contains("extent")
    );
}

#[test]
fn normalized_geometry_digest_is_format_independent_and_budget_is_preflighted() {
    let (mut config, geometry) = inputs();
    let reordered = "id: original-basin\r\nschema_version: 0.1.0\r\nsolid:\r\n  floor_thickness_m: '0.02'\r\n  wall_thickness_m: ['0.02', '0.02']\r\n  outer_extent_m: ['0.48', '0.36', '0.18']\r\n  kind: open-basin\r\n";
    let reordered: SolidGeometryDocument = serde_yaml_ng::from_str(reordered).unwrap();
    assert_eq!(geometry_digest(&geometry).unwrap().len(), 64);
    assert_eq!(geometry_digest(&geometry), geometry_digest(&reordered));
    let layout = CompiledPhysicsProof::planned_storage(&config, Some(&geometry)).unwrap();
    assert_eq!(layout.cells, 1072);
    assert!(layout.checkpoint_field_bytes > layout.state_bytes);
    config.allocation_budget_bytes =
        layout.immutable_geometry_bytes + layout.instance_buffer_bytes().unwrap();
    assert!(
        CompiledPhysicsProof::compile_with_geometry(config, Some(geometry))
            .unwrap_err()
            .contains("25% reserve")
    );
}

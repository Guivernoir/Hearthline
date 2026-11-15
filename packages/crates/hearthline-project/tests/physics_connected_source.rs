use hearthline_config::physics::connected::{
    CONNECTED_GAS_SOURCE_LIMIT, ConnectedGasDocument, GasBoundaryConfig, GasFace,
};
use hearthline_project::physics::DEFAULT_PHYSICS_BUDGET_BYTES;
use hearthline_project::physics::flow::connected::CompiledConnectedGas;

const SOURCE: &str = include_str!("../../../../project/physics/runtime/connected-source.yaml");
fn document() -> ConnectedGasDocument {
    ConnectedGasDocument::from_yaml(SOURCE).unwrap()
}
fn compile(document: &ConnectedGasDocument) -> Result<CompiledConnectedGas, String> {
    CompiledConnectedGas::compile_document(document, DEFAULT_PHYSICS_BUDGET_BYTES)
}

#[test]
fn yaml_source_reproduces_existing_numerical_model_without_regoldening() {
    let golden: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../project/physics/runtime/connected-retry.json"
    ))
    .unwrap();
    let source = document();
    let mut compiled = None;
    let measured = allocation_counter::measure(|| compiled = Some(compile(&source).unwrap()));
    let compiled = compiled.unwrap();
    assert_eq!(compiled.digest(), golden["model_digest"]);
    assert_eq!(compiled.mesh().memory().cells, 24);
    assert_eq!(compiled.evaluation_bound(), 3556);
    assert!(measured.bytes_max as u64 <= compiled.memory().compiler_peak_bytes);
    assert!(compiled.memory().compiler_peak_bytes > compiled.memory().runtime_bytes);
}

#[test]
fn source_capacity_is_charged_even_when_unused_and_cannot_consume_the_reserve() {
    let mut source = document();
    let baseline = compile(&source).unwrap();
    let previous = source.regions.capacity();
    source.regions.reserve_exact(1024);
    let extra = (source.regions.capacity() - previous)
        * size_of::<hearthline_config::physics::connected::GasRegionConfig>();
    let expanded = compile(&source).unwrap();
    assert_eq!(expanded.digest(), baseline.digest());
    assert_eq!(
        expanded.memory().compiler_peak_bytes,
        baseline.memory().compiler_peak_bytes + extra as u64,
    );
    source.allocation_budget_bytes = (expanded.memory().compiler_peak_bytes * 4).div_ceil(3) - 1;
    assert!(compile(&source).unwrap_err().contains("reserve"));
}

#[test]
fn source_order_endpoint_orientation_and_decimal_spelling_do_not_change_physics() {
    let baseline = compile(&document()).unwrap();
    let crlf = ConnectedGasDocument::from_yaml(&SOURCE.replace('\n', "\r\n")).unwrap();
    assert_eq!(compile(&crlf).unwrap().digest(), baseline.digest());
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut source = document();
        let previous = source.regions.clone();
        source.regions = order.map(|index| previous[index].clone()).into();
        source.connections.reverse();
        for connection in &mut source.connections {
            std::mem::swap(&mut connection.first, &mut connection.second);
        }
        source.reference.length_m = "1.000000000".into();
        assert_eq!(compile(&source).unwrap().digest(), baseline.digest());
    }
    let mut changed = document();
    changed.regions[0].initial.pressure_absolute_pa = "2.1".into();
    assert_ne!(compile(&changed).unwrap().digest(), baseline.digest());
}

#[test]
fn every_coupled_face_requires_exactly_one_link_and_cannot_hide_a_reservoir() {
    let mut missing = document();
    missing.connections.pop();
    assert!(compile(&missing).unwrap_err().contains("unlinked coupled"));
    let mut duplicate = document();
    duplicate.connections.push(duplicate.connections[0].clone());
    assert!(compile(&duplicate).unwrap_err().contains("unused coupled"));
    let mut wrong_face = document();
    wrong_face.connections[0].first.face = GasFace::YPlus;
    assert!(compile(&wrong_face).unwrap_err().contains("unused coupled"));
    let mut hidden_supply = document();
    hidden_supply.regions[0].boundaries.x_plus = GasBoundaryConfig::Reservoir {
        state: hidden_supply.regions[0].initial.clone(),
    };
    assert!(
        compile(&hidden_supply)
            .unwrap_err()
            .contains("unused coupled")
    );
    let mut unknown = document();
    unknown.connections[0].first.region.instance = "missing".into();
    assert!(compile(&unknown).unwrap_err().contains("unused coupled"));
    let mut duplicate = document();
    duplicate.regions.push(duplicate.regions[0].clone());
    assert!(compile(&duplicate).unwrap_err().contains("duplicate"));
    let mut mismatched = document();
    mismatched.regions[1].shape[1] = 3;
    assert!(compile(&mismatched).unwrap_err().contains("nonconforming"));
}

#[test]
fn coefficient_schema_and_memory_errors_fail_before_field_allocation() {
    let mut source = document();
    source.schema_version = "99.0.0".into();
    assert!(compile(&source).unwrap_err().contains("unsupported"));
    source = document();
    source.id = "invalid id".into();
    assert!(compile(&source).is_err());
    source = document();
    source.allocation_budget_bytes = DEFAULT_PHYSICS_BUDGET_BYTES + 1;
    assert!(compile(&source).unwrap_err().contains("host allocation"));
    source = document();
    source.allocation_budget_bytes = 100;
    assert!(compile(&source).unwrap_err().contains("source metadata"));
    source = document();
    source.regions[0].shape = [u32::MAX; 3];
    assert!(compile(&source).is_err());
    source = document();
    source.gas.gas_constant_j_kg_k = "NaN".into();
    assert!(compile(&source).unwrap_err().contains("physical decimal"));
    source = document();
    source.regions[0].initial.pressure_absolute_pa = "0".into();
    assert!(compile(&source).is_err());
    source = document();
    source.regions[0].boundaries.x_minus = GasBoundaryConfig::NoSlipWall {
        velocity_m_s: ["0.1".into(), "0".into(), "0".into()],
        temperature_k: None,
    };
    assert!(
        compile(&source)
            .unwrap_err()
            .contains("moving normal walls")
    );
}

#[test]
fn bounded_yaml_rejects_duplicates_depth_and_alias_amplification_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let roundtrip = serde_yaml_ng::to_string(&document()).unwrap();
            assert_eq!(
                ConnectedGasDocument::from_yaml(&roundtrip).unwrap(),
                document()
            );
            for source in [
                format!("{SOURCE}\nid: duplicate\n"),
                format!("{SOURCE}\nunknown: 1\n"),
                SOURCE.replace("pressure_absolute_pa", "pressure_gauge_pa"),
                SOURCE.replace("kind: coupled", "kind: unknown"),
                SOURCE.replace("\"1\"", "1"),
                SOURCE.replace("instance: supply", "instance: 123"),
                format!("{SOURCE}\n---\n{SOURCE}"),
            ] {
                assert!(ConnectedGasDocument::from_yaml(&source).is_err());
            }
            let numeric_alias = SOURCE
                .replace(
                    "length_m: \"1\"",
                    &format!("length_m: &numeric 0.{}", "0".repeat(16_384)),
                )
                .replace("\"1\"", "*numeric");
            assert!(
                ConnectedGasDocument::from_yaml(&numeric_alias)
                    .unwrap_err()
                    .contains("YAML string")
            );
            let source = " ".repeat(CONNECTED_GAS_SOURCE_LIMIT + 1);
            assert!(
                ConnectedGasDocument::from_yaml(&source)
                    .unwrap_err()
                    .contains("1 MiB")
            );
            let deep = format!("{}0{}", "[".repeat(40), "]".repeat(40));
            assert!(
                ConnectedGasDocument::from_yaml(&deep)
                    .unwrap_err()
                    .contains("nesting")
            );
            let recursive = "&cycle [*cycle]";
            assert!(ConnectedGasDocument::from_yaml(recursive).is_err());
            assert!(ConnectedGasDocument::from_yaml("!unsupported [1]").is_err());
            let alias = format!(
                "- &value '{}'\n{}",
                "x".repeat(16_384),
                "- *value\n".repeat(100)
            );
            assert!(
                ConnectedGasDocument::from_yaml(&alias)
                    .unwrap_err()
                    .contains("size budget")
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

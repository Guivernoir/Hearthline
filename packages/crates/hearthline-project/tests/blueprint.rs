use std::fs;

use hearthline_project::{BlueprintDefinition, BlueprintInstance, BlueprintRepository};
use sha2::{Digest, Sha256};

#[test]
fn generated_multi_site_model_expands_repeatably_beyond_global_partition_limits() {
    let root = tempfile::tempdir().expect("temporary acceptance project");
    let blueprints = root.path().join("blueprints");
    let instances = root.path().join("instances");
    fs::create_dir_all(&blueprints).expect("blueprint folder");
    fs::create_dir_all(&instances).expect("instance folder");
    fs::write(
        blueprints.join("process-cell.yaml"),
        r#"schema_version: 0.1.0
id: process-cell
parameters:
  - { id: equipment-count, kind: { type: integer, minimum: 1, maximum: 32 }, required: true }
  - { id: sensor-count, kind: { type: integer, minimum: 1, maximum: 32 }, required: true }
nodes:
  - { local_id: controller, family: virtual-controller, ports: [control, telemetry] }
  - { local_id: equipment, family: field-actuator, repeat_parameter: equipment-count, ports: [control] }
  - { local_id: sensor, family: field-sensor, repeat_parameter: sensor-count, ports: [measurement] }
connections:
  - { local_id: control, from_node: controller, from_port: control, to_node: equipment, to_port: control, mode: fan-out }
  - { local_id: telemetry, from_node: controller, from_port: telemetry, to_node: sensor, to_port: measurement, mode: fan-out }
"#,
    )
    .expect("blueprint source");

    for (id, site, environment) in [
        ("office-core", "central-office", "enterprise-core"),
        ("customer-lan", "customer-edge", "customer-lan"),
    ] {
        fs::write(
            instances.join(format!("{id}.yaml")),
            format!(
                "schema_version: 0.1.0\nid: {id}\nblueprint: process-cell\nsite: {site}\nenvironment: {environment}\nvalues:\n  equipment-count: 8\n  sensor-count: 8\n"
            ),
        )
        .expect("site instance source");
    }

    for factory in 1..=3 {
        for area in 1..=10 {
            for cell in 1..=3 {
                let id = format!("factory-{factory:02}-area-{area:02}-cell-{cell:02}");
                fs::write(
                    instances.join(format!("{id}.yaml")),
                    format!(
                        "schema_version: 0.1.0\nid: {id}\nblueprint: process-cell\nsite: factory-{factory:02}\nenvironment: area-{area:02}\nvalues:\n  equipment-count: 8\n  sensor-count: 8\n"
                    ),
                )
                .expect("instance source");
            }
        }
    }

    let first = BlueprintRepository::load(&blueprints, &instances)
        .expect("acceptance blueprint repository")
        .expand_all()
        .expect("acceptance expansion");
    let second = BlueprintRepository::load(&blueprints, &instances)
        .expect("repeat blueprint repository")
        .expand_all()
        .expect("repeat expansion");

    assert_eq!(first, second);
    assert_eq!(first.len(), 92);
    assert!(first.iter().any(|item| item.instance == "office-core"));
    assert!(first.iter().any(|item| item.instance == "customer-lan"));
    assert!(first.iter().map(|item| item.nodes.len()).sum::<usize>() >= 1_200);
    assert!(
        first
            .iter()
            .map(|item| item.connections.len())
            .sum::<usize>()
            >= 1_400
    );
}

#[test]
fn typed_parameters_connection_modes_and_namespaces_expand_without_templates() {
    let definition = r#"schema_version: 0.1.0
id: typed-cell
parameters:
  - { id: enabled, kind: { type: boolean }, required: true }
  - { id: count, kind: { type: integer, minimum: 1, maximum: 4 }, required: true }
  - { id: pressure, kind: { type: quantity, unit: bar, scale: 1000, minimum_raw: 1000, maximum_raw: 8000 }, required: true }
  - { id: asset, kind: { type: identifier }, required: true }
  - { id: mode, kind: { type: choice, values: [auto, manual] }, required: true }
  - { id: members, kind: { type: identifier-list, minimum: 1, maximum: 3 }, required: true }
  - { id: optional, kind: { type: boolean }, default: true }
nodes:
  - { local_id: controller, family: virtual-controller, ports: [single, pair, fan] }
  - { local_id: endpoint, family: field-actuator, ports: [single] }
  - { local_id: pair-source, family: field-sensor, repeat_parameter: count, ports: [data] }
  - { local_id: pair-target, family: remote-io, repeat_parameter: count, ports: [data] }
  - { local_id: fan-target, family: field-sensor, repeat_parameter: count, ports: [data] }
connections:
  - { local_id: single-link, from_node: controller, from_port: single, to_node: endpoint, to_port: single, mode: single }
  - { local_id: pair-link, from_node: pair-source, from_port: data, to_node: pair-target, to_port: data, mode: pairwise }
  - { local_id: fan-link, from_node: controller, from_port: fan, to_node: fan-target, to_port: data, mode: fan-out }
exports:
  - { id: telemetry, kind: signal, node: controller, port: fan, contract: process-telemetry }
"#;
    let instance = r#"schema_version: 0.1.0
id: typed-instance
blueprint: typed-cell
site: factory-01
environment: forming
values:
  enabled: true
  count: 2
  pressure: "4.250 bar"
  asset: mould-01
  mode: auto
  members: [sensor-01, sensor-02]
"#;
    let repository = BlueprintRepository::from_sources([definition], [instance])
        .expect("typed blueprint repository");
    assert_eq!(repository.definitions().count(), 1);
    assert_eq!(repository.instances().count(), 1);
    let expanded = repository.expand_all().expect("typed expansion");
    assert_eq!(expanded[0].nodes.len(), 8);
    assert_eq!(expanded[0].connections.len(), 5);
    assert!(expanded[0].nodes.iter().all(|node| {
        node.id.starts_with("typed-instance-")
            && node.site == "factory-01"
            && node.environment == "forming"
    }));
    assert!(
        expanded[0]
            .connections
            .iter()
            .any(|connection| connection.id == "typed-instance-pair-link-02")
    );
}

#[test]
fn blueprint_and_instance_schemas_migrate_only_the_previous_version() {
    let definition = "schema_version: 0.0.1\nid: migrated-cell\n";
    assert_eq!(
        BlueprintDefinition::from_yaml(definition)
            .expect("previous blueprint")
            .schema_version,
        "0.1.0"
    );
    let instance = "schema_version: 0.0.1\nid: migrated-instance\nblueprint: migrated-cell\nsite: factory\nenvironment: test\n";
    assert_eq!(
        BlueprintInstance::from_yaml(instance)
            .expect("previous instance")
            .schema_version,
        "0.1.0"
    );
    for source in [
        "schema_version: 9.9.9\nid: unsupported\n",
        "schema_version: 0.1.0\nid: Invalid_ID\n",
        "not: [valid",
    ] {
        assert!(BlueprintDefinition::from_yaml(source).is_err(), "{source}");
    }
    assert!(
        BlueprintInstance::from_yaml(
            "schema_version: 0.1.0\nid: Invalid_ID\nblueprint: cell\nsite: factory\nenvironment: test\n"
        )
        .is_err()
    );
}

#[test]
fn definition_validation_rejects_ambiguous_or_unbound_topology() {
    let invalid = [
        r#"schema_version: 0.1.0
id: duplicate-parameter
parameters:
  - { id: count, kind: { type: integer, minimum: 1, maximum: 2 } }
  - { id: count, kind: { type: integer, minimum: 1, maximum: 2 } }
"#,
        r#"schema_version: 0.1.0
id: duplicate-node
nodes:
  - { local_id: sensor, family: field-sensor }
  - { local_id: sensor, family: field-sensor }
"#,
        r#"schema_version: 0.1.0
id: duplicate-connection
nodes: [{ local_id: a, family: field-sensor }, { local_id: b, family: field-actuator }]
connections:
  - { local_id: link, from_node: a, from_port: p, to_node: b, to_port: p, mode: single }
  - { local_id: link, from_node: a, from_port: p, to_node: b, to_port: p, mode: single }
"#,
        r#"schema_version: 0.1.0
id: duplicate-export
nodes: [{ local_id: node, family: field-sensor, ports: [data] }]
exports:
  - { id: data, kind: signal, node: node, port: data, contract: sample }
  - { id: data, kind: signal, node: node, port: data, contract: sample }
"#,
        r#"schema_version: 0.1.0
id: unknown-repeat
nodes: [{ local_id: sensor, family: field-sensor, repeat_parameter: missing }]
"#,
        r#"schema_version: 0.1.0
id: unknown-node
nodes: [{ local_id: source, family: field-sensor }]
connections: [{ local_id: link, from_node: source, from_port: data, to_node: missing, to_port: data, mode: single }]
"#,
        r#"schema_version: 0.1.0
id: bad-local-id
nodes: [{ local_id: Invalid_ID, family: field-sensor }]
"#,
        r#"schema_version: 0.1.0
id: unknown-export-node
exports: [{ id: output, kind: signal, node: missing, port: data, contract: sample }]
"#,
        r#"schema_version: 0.1.0
id: unknown-export-port
nodes: [{ local_id: node, family: field-sensor, ports: [data] }]
exports: [{ id: output, kind: signal, node: node, port: missing, contract: sample }]
"#,
        r#"schema_version: 0.1.0
id: empty-export-contract
nodes: [{ local_id: node, family: field-sensor, ports: [data] }]
exports: [{ id: output, kind: signal, node: node, port: data, contract: "" }]
"#,
    ];
    for source in invalid {
        assert!(BlueprintDefinition::from_yaml(source).is_err(), "{source}");
    }
}

#[test]
fn parameter_validation_rejects_missing_unknown_out_of_range_and_wrong_types() {
    let definition = r#"schema_version: 0.1.0
id: parameter-cell
parameters:
  - { id: required-value, kind: { type: boolean }, required: true }
  - { id: count, kind: { type: integer, minimum: 1, maximum: 3 }, required: true }
  - { id: asset, kind: { type: identifier }, required: true }
  - { id: mode, kind: { type: choice, values: [auto, manual] }, required: true }
  - { id: members, kind: { type: identifier-list, minimum: 1, maximum: 2 }, required: true }
  - { id: pressure, kind: { type: quantity, unit: bar, scale: 1000, minimum_raw: -1000, maximum_raw: 8000 }, required: true }
nodes: [{ local_id: sensor, family: field-sensor, repeat_parameter: count }]
"#;
    let values = [
        "count: 2\n  asset: asset-01\n  mode: auto\n  members: [one]\n  pressure: '4.000 bar'",
        "required-value: true\n  count: 9\n  asset: asset-01\n  mode: auto\n  members: [one]\n  pressure: '4.000 bar'",
        "required-value: true\n  count: 2\n  asset: Invalid_ID\n  mode: auto\n  members: [one]\n  pressure: '4.000 bar'",
        "required-value: true\n  count: 2\n  asset: asset-01\n  mode: invalid\n  members: [one]\n  pressure: '4.000 bar'",
        "required-value: true\n  count: 2\n  asset: asset-01\n  mode: auto\n  members: []\n  pressure: '4.000 bar'",
        "required-value: true\n  count: 2\n  asset: asset-01\n  mode: auto\n  members: [Invalid_ID]\n  pressure: '4.000 bar'",
        "required-value: true\n  count: 2\n  asset: asset-01\n  mode: auto\n  members: [one]\n  pressure: '4.0000 bar'",
        "required-value: true\n  count: 2\n  asset: asset-01\n  mode: auto\n  members: [one]\n  pressure: '9.000 bar'",
        "required-value: true\n  count: 2\n  asset: asset-01\n  mode: auto\n  members: [one]\n  pressure: '-0.500 bar'\n  extra: true",
    ];
    for (index, values) in values.into_iter().enumerate() {
        let instance = format!(
            "schema_version: 0.1.0\nid: invalid-{index:02}\nblueprint: parameter-cell\nsite: factory\nenvironment: test\nvalues:\n  {values}\n"
        );
        let repository = BlueprintRepository::from_sources([definition], [instance.as_str()])
            .expect("invalid values remain syntactically valid");
        assert!(repository.expand_all().is_err(), "{instance}");
    }
}

#[test]
fn imports_are_pinned_acyclic_and_required_for_nested_instances() {
    let child = "schema_version: 0.1.0\nid: child-cell\nnodes: [{ local_id: sensor, family: field-sensor, ports: [data] }]\n";
    let digest = hex_digest(child.as_bytes());
    let parent = format!(
        "schema_version: 0.1.0\nid: parent-cell\nimports:\n  - {{ blueprint: child-cell, schema_version: 0.1.0, sha256: {digest} }}\nnested:\n  - {{ local_id: child, blueprint: child-cell }}\n"
    );
    let instance = "schema_version: 0.1.0\nid: parent-instance\nblueprint: parent-cell\nsite: factory\nenvironment: area\n";
    let repository = BlueprintRepository::from_sources([child, parent.as_str()], [instance])
        .expect("pinned nested blueprint");
    assert_eq!(
        repository.expand_all().expect("nested expansion")[0]
            .nodes
            .len(),
        1
    );

    let failures = [
        "schema_version: 0.1.0\nid: unknown-import\nimports: [{ blueprint: missing, schema_version: 0.1.0, sha256: deadbeef }]\n",
        "schema_version: 0.1.0\nid: missing-pin\nnested: [{ local_id: child, blueprint: child-cell }]\n",
        "schema_version: 0.1.0\nid: self-cycle\nimports: [{ blueprint: self-cycle, schema_version: 0.1.0, sha256: deadbeef }]\n",
    ];
    assert!(BlueprintRepository::from_sources([failures[0]], []).is_err());
    assert!(BlueprintRepository::from_sources([child, failures[1]], []).is_err());
    assert!(
        BlueprintRepository::from_sources([failures[2]], [])
            .expect_err("cycle")
            .to_string()
            .contains("cycle")
    );

    let wrong_schema = parent.replace(
        "schema_version: 0.1.0, sha256",
        "schema_version: 9.9.9, sha256",
    );
    assert!(BlueprintRepository::from_sources([child, wrong_schema.as_str()], []).is_err());
    let wrong_digest = parent.replace(&digest, "deadbeef");
    assert!(BlueprintRepository::from_sources([child, wrong_digest.as_str()], []).is_err());
    let unknown_instance = "schema_version: 0.1.0\nid: unknown-instance\nblueprint: missing\nsite: factory\nenvironment: area\n";
    assert!(BlueprintRepository::from_sources([], [unknown_instance]).is_err());
    assert!(BlueprintRepository::from_sources([child, child], []).is_err());
    assert!(BlueprintRepository::from_sources([child], [instance, instance]).is_err());
}

#[test]
fn expansion_rejects_incompatible_repetition_and_oversized_namespaces() {
    let definition = r#"schema_version: 0.1.0
id: incompatible-cell
parameters: [{ id: count, kind: { type: integer, minimum: 1, maximum: 4 }, required: true }]
nodes:
  - { local_id: source, family: field-sensor, repeat_parameter: count, ports: [data] }
  - { local_id: target, family: field-actuator, ports: [data] }
connections:
  - { local_id: link, from_node: source, from_port: data, to_node: target, to_port: data, mode: single }
"#;
    let instance = "schema_version: 0.1.0\nid: incompatible-instance\nblueprint: incompatible-cell\nsite: factory\nenvironment: area\nvalues: { count: 2 }\n";
    let repository = BlueprintRepository::from_sources([definition], [instance]).unwrap();
    assert!(repository.expand_all().is_err());

    for (mode, source_count, target_count) in
        [("single", 1, 2), ("pairwise", 2, 1), ("fan-out", 2, 1)]
    {
        let definition = format!(
            "schema_version: 0.1.0\nid: mode-cell\nparameters:\n  - {{ id: source-count, kind: {{ type: integer, minimum: 1, maximum: 2 }}, required: true }}\n  - {{ id: target-count, kind: {{ type: integer, minimum: 1, maximum: 2 }}, required: true }}\nnodes:\n  - {{ local_id: source, family: field-sensor, repeat_parameter: source-count, ports: [data] }}\n  - {{ local_id: target, family: field-actuator, repeat_parameter: target-count, ports: [data] }}\nconnections:\n  - {{ local_id: link, from_node: source, from_port: data, to_node: target, to_port: data, mode: {mode} }}\n"
        );
        let instance = format!(
            "schema_version: 0.1.0\nid: mode-instance\nblueprint: mode-cell\nsite: factory\nenvironment: area\nvalues: {{ source-count: {source_count}, target-count: {target_count} }}\n"
        );
        let repository =
            BlueprintRepository::from_sources([definition.as_str()], [instance.as_str()])
                .expect("connection mode fixture");
        assert!(repository.expand_all().is_err(), "{mode}");
    }

    let definition = "schema_version: 0.1.0\nid: long-cell\nnodes: [{ local_id: component-with-a-long-name, family: field-sensor }]\n";
    let instance = format!(
        "schema_version: 0.1.0\nid: {}\nblueprint: long-cell\nsite: factory\nenvironment: area\n",
        "a".repeat(60)
    );
    let repository = BlueprintRepository::from_sources([definition], [instance.as_str()]).unwrap();
    assert!(repository.expand_all().is_err());
}

#[test]
fn nested_blueprints_expand_bounded_repetition_with_unique_namespaces() {
    let child = "schema_version: 0.1.0\nid: repeated-child\nnodes: [{ local_id: sensor, family: field-sensor, ports: [data] }]\n";
    let digest = hex_digest(child.as_bytes());
    let parent = format!(
        "schema_version: 0.1.0\nid: repeated-parent\nparameters: [{{ id: child-count, kind: {{ type: integer, minimum: 1, maximum: 3 }}, required: true }}]\nimports: [{{ blueprint: repeated-child, schema_version: 0.1.0, sha256: {digest} }}]\nnested: [{{ local_id: child, blueprint: repeated-child, repeat_parameter: child-count }}]\n"
    );
    let instance = "schema_version: 0.1.0\nid: repeated-instance\nblueprint: repeated-parent\nsite: factory\nenvironment: area\nvalues: { child-count: 2 }\n";
    let expanded = BlueprintRepository::from_sources([child, parent.as_str()], [instance])
        .expect("nested repetition repository")
        .expand_all()
        .expect("nested repetition expansion");
    assert_eq!(expanded[0].nodes.len(), 2);
    assert_eq!(expanded[0].nodes[0].id, "repeated-instance-child-01-sensor");
    assert_eq!(expanded[0].nodes[1].id, "repeated-instance-child-02-sensor");
}

#[test]
fn repository_loader_handles_empty_and_duplicate_source_directories() {
    let root = tempfile::tempdir().expect("temporary loader project");
    let missing_blueprints = root.path().join("missing-blueprints");
    let missing_instances = root.path().join("missing-instances");
    let empty = BlueprintRepository::load(&missing_blueprints, &missing_instances)
        .expect("missing optional blueprint roots");
    assert_eq!(empty.definitions().count(), 0);

    let blueprints = root.path().join("blueprints");
    let instances = root.path().join("instances");
    fs::create_dir_all(&blueprints).unwrap();
    fs::create_dir_all(&instances).unwrap();
    let source = "schema_version: 0.1.0\nid: duplicate-cell\n";
    fs::write(blueprints.join("a.yaml"), source).unwrap();
    fs::write(blueprints.join("b.yml"), source).unwrap();
    fs::write(blueprints.join("ignored.txt"), source).unwrap();
    assert!(BlueprintRepository::load(&blueprints, &instances).is_err());
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

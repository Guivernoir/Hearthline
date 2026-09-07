use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hearthline_model::Text;
use hearthline_project::ProjectCompiler;
use hearthline_sim::{SessionCapacityPolicy, SimulationSession, SnapshotValue};

const FACTORY_COUNT: usize = 3;
const AREAS_PER_FACTORY: usize = 10;
const CELLS_PER_AREA: usize = 3;
const COMPONENTS_PER_CELL: usize = 17;
const CONNECTIONS_PER_CELL: usize = 16;

#[derive(Clone)]
struct AcceptanceCell {
    site: Text<64>,
    id: Text<64>,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn acceptance_cells() -> Vec<AcceptanceCell> {
    let mut cells = vec![
        AcceptanceCell {
            site: "acceptance-central-office".into(),
            id: "acceptance-office-core".into(),
        },
        AcceptanceCell {
            site: "acceptance-customer-edge".into(),
            id: "acceptance-customer-lan".into(),
        },
    ];
    for factory in 1..=FACTORY_COUNT {
        for area in 1..=AREAS_PER_FACTORY {
            for cell in 1..=CELLS_PER_AREA {
                cells.push(AcceptanceCell {
                    site: Text::try_new(&format!("acceptance-factory-{factory:02}")).unwrap(),
                    id: Text::try_new(&format!("acceptance-f{factory:02}-a{area:02}-c{cell:02}"))
                        .unwrap(),
                });
            }
        }
    }
    cells
}

fn acceptance_project() -> (tempfile::TempDir, Arc<hearthline_project::CompiledProject>) {
    let temporary = tempfile::tempdir().expect("temporary acceptance repository");
    let config = temporary.path().join("project/config");
    copy_tree(&repository_root().join("project/config"), &config);
    fs::write(
        config.join("blueprints/acceptance-process-cell.yaml"),
        r#"schema_version: 0.1.0
id: acceptance-process-cell
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
    .expect("acceptance blueprint");
    for cell in acceptance_cells() {
        let environment = format!("acceptance-{}", cell.id);
        fs::write(
            config
                .join("instances")
                .join(format!("{}.yaml", cell.id)),
            format!(
                "schema_version: 0.1.0\nid: {}\nblueprint: acceptance-process-cell\nsite: {}\nenvironment: {environment}\nvalues:\n  equipment-count: 8\n  sensor-count: 8\n",
                cell.id, cell.site
            ),
        )
        .expect("acceptance instance");
    }
    let project = ProjectCompiler::new(config)
        .compile("generated scale acceptance")
        .expect("compiled acceptance project");
    (temporary, Arc::new(project))
}

#[test]
fn generated_multi_site_project_runs_twenty_four_hours_without_post_seal_allocation() {
    let (_temporary, project) = acceptance_project();
    let generated = project
        .expanded_blueprints()
        .iter()
        .filter(|blueprint| blueprint.instance.starts_with("acceptance-"))
        .collect::<Vec<_>>();
    assert_eq!(generated.len(), 92);
    assert_eq!(
        generated
            .iter()
            .map(|blueprint| blueprint.nodes.len())
            .sum::<usize>(),
        generated.len() * COMPONENTS_PER_CELL
    );
    assert_eq!(
        generated
            .iter()
            .map(|blueprint| blueprint.connections.len())
            .sum::<usize>(),
        generated.len() * CONNECTIONS_PER_CELL
    );

    let mut session = SimulationSession::build(
        Arc::clone(&project),
        SessionCapacityPolicy::reviewed_default(),
    )
    .expect("compiled scale session");
    assert!(session.component_count() >= 1_200);
    assert!(session.link_count() >= 1_400);
    assert!(session.scheduler().cells().len() >= 75);

    let cells = acceptance_cells();
    let faulted = [cells[12].clone(), cells[42].clone(), cells[72].clone()];
    let run = allocation_counter::measure(|| {
        for minute in 1_u64..=24 * 60 {
            if minute == 360 {
                for cell in &faulted {
                    session
                        .scheduler_mut()
                        .unwrap()
                        .set_operational(&cell.site, &cell.id, false)
                        .unwrap();
                }
            }
            if minute == 370 {
                for cell in &faulted {
                    session
                        .scheduler_mut()
                        .unwrap()
                        .set_operational(&cell.site, &cell.id, true)
                        .unwrap();
                }
            }
            session
                .advance_to(minute * 60 * 1_000_000)
                .expect("monotonic acceptance step");
        }
    });

    assert_eq!(
        run.count_total, 0,
        "sealed acceptance run allocated: {run:?}"
    );
    assert_eq!(session.scheduler().pending_len(), 0);
    assert_eq!(session.scheduler().metrics().saturation_stops, 0);

    let snapshot = session.snapshot().expect("full acceptance snapshot");
    let nominal = snapshot
        .cells
        .iter()
        .find(|cell| cell.cell.as_str() == "acceptance-office-core")
        .expect("Central Office acceptance cell");
    let interrupted = snapshot
        .cells
        .iter()
        .find(|cell| cell.cell == faulted[0].id)
        .expect("faulted acceptance cell");
    assert_eq!(counter(nominal, "ticks"), 1_440);
    assert_eq!(counter(interrupted, "ticks"), 1_430);
    assert!(
        nominal
            .components
            .iter()
            .all(|component| counter_value(component, "elapsed-us") > 0)
    );
}

fn counter(cell: &hearthline_sim::CellSnapshot, field: &str) -> i64 {
    cell.components
        .first()
        .map(|component| counter_value(component, field))
        .unwrap_or_default()
}

fn counter_value(component: &hearthline_sim::ComponentSnapshot, field: &str) -> i64 {
    match component.state.get(field) {
        Some(SnapshotValue::Integer(value)) => *value,
        value => panic!(
            "component {} field {field} is {value:?}",
            component.component
        ),
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create copied directory");
    for entry in fs::read_dir(source).expect("read source directory") {
        let entry = entry.expect("source entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("source file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy source file");
        }
    }
}

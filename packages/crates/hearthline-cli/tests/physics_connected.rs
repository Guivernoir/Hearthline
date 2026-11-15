use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;

use hearthline_config::physics::connected::ConnectedGasDocument;
use hearthline_project::physics::DEFAULT_PHYSICS_BUDGET_BYTES;
use hearthline_project::physics::flow::connected::CompiledConnectedGas;
use hearthline_sim::physics::flow::connected::ConnectedGasSession;

fn source_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../project/physics/runtime/connected-source.yaml")
}
fn compile_model(source: &str) -> Arc<CompiledConnectedGas> {
    Arc::new(
        CompiledConnectedGas::compile_document(
            &ConnectedGasDocument::from_yaml(source).unwrap(),
            DEFAULT_PHYSICS_BUDGET_BYTES,
        )
        .unwrap(),
    )
}
fn run(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hearthline"))
        .args(["physics", "gas", "run"])
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}
fn successful(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn golden() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../project/physics/runtime/connected-retry.json"
    ))
    .unwrap()
}

#[test]
fn source_cli_reports_bounded_work_and_runs_all_pacing_modes_to_the_existing_golden() {
    let path = source_path();
    let budget = Command::new(env!("CARGO_BIN_EXE_hearthline"))
        .args(["physics", "gas", "budget"])
        .arg(&path)
        .output()
        .unwrap();
    let budget: serde_json::Value = serde_json::from_str(&successful(budget)).unwrap();
    assert_eq!(budget["model_digest"], golden()["model_digest"]);
    assert_eq!(budget["cells"], 24);
    assert_eq!(budget["maximum_evaluations_per_horizon"], 3556);
    assert!(
        budget["worker_planned_bytes"].as_u64().unwrap() * 4
            <= budget["allocation_budget_bytes"].as_u64().unwrap() * 3
    );
    for args in [
        vec!["--headless"],
        vec!["--rate", "1"],
        vec!["--rate", "2"],
        vec!["--rate", "5"],
        vec!["--rate", "10"],
    ] {
        let text = successful(run(&path, &args));
        assert!(text.contains(golden()["states"][3].as_str().unwrap()));
        assert!(text.contains("3 physical steps, 3003 us"));
        assert!(text.contains("achieved") && text.contains("not plant qualification"));
    }
    assert!(!run(&path, &["--rate", "3"]).status.success());
    assert!(!run(&path, &["--rate", "2", "--headless"]).status.success());
}

#[test]
fn cli_continues_a_prefix_snapshot_and_streams_the_final_state_without_clobbering() {
    let directory = tempfile::tempdir().unwrap();
    let config = source_path();
    let model = compile_model(&std::fs::read_to_string(&config).unwrap());
    let mut session = ConnectedGasSession::new(Arc::clone(&model)).unwrap();
    session.step_next().unwrap();
    let prefix = directory.path().join("prefix.ndjson");
    session
        .write_snapshot(File::create(&prefix).unwrap())
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hearthline"))
        .current_dir(directory.path())
        .args(["physics", "gas", "run"])
        .arg(&config)
        .arg("--resume")
        .arg(&prefix)
        .args(["--snapshot", "final.ndjson", "--headless"])
        .output()
        .unwrap();
    assert!(successful(output).contains(golden()["states"][3].as_str().unwrap()));
    let path = directory.path().join("final.ndjson");
    let bytes = std::fs::read(&path).unwrap();
    let restored =
        ConnectedGasSession::read_snapshot(Arc::clone(&model), Cursor::new(&bytes)).unwrap();
    assert!(restored.complete());
    assert_eq!(
        restored.state_digest().unwrap(),
        golden()["states"][3].as_str().unwrap().as_bytes()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_hearthline"))
        .args(["physics", "gas", "run"])
        .arg(&config)
        .arg("--snapshot")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("already exists"));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    let mut changed =
        ConnectedGasDocument::from_yaml(&std::fs::read_to_string(config).unwrap()).unwrap();
    changed.regions[0].spacing_m[0] = "0.38".into();
    let changed = compile_model(&serde_yaml_ng::to_string(&changed).unwrap());
    assert!(ConnectedGasSession::read_snapshot(changed, Cursor::new(bytes)).is_err());
}

#[test]
fn a_faulted_run_writes_only_accepted_fields_and_resume_does_not_clear_the_fault() {
    let directory = tempfile::tempdir().unwrap();
    let source = std::fs::read_to_string(source_path())
        .unwrap()
        .replace("maximum_iterations: 14", "maximum_iterations: 1");
    let config = directory.path().join("fault.yaml");
    std::fs::write(&config, &source).unwrap();
    let path = directory.path().join("fault.ndjson");
    let output = Command::new(env!("CARGO_BIN_EXE_hearthline"))
        .args(["physics", "gas", "run"])
        .arg(&config)
        .arg("--snapshot")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("0 committed steps") && error.contains("attempts: 7"),
        "{error}"
    );
    let restored = ConnectedGasSession::read_snapshot(
        compile_model(&source),
        BufReader::new(File::open(&path).unwrap()),
    )
    .unwrap();
    assert_eq!(
        restored.state_digest().unwrap(),
        golden()["failure_digest"].as_str().unwrap().as_bytes()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_hearthline"))
        .args(["physics", "gas", "run"])
        .arg(&config)
        .arg("--resume")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains(golden()["failure_digest"].as_str().unwrap())
    );
}

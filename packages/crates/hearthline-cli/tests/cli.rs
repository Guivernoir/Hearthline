use std::path::PathBuf;
use std::process::Command;

fn hearthline(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_hearthline"))
        .args(args)
        .output()
        .expect("hearthline CLI starts");
    assert!(
        output.status.success(),
        "command {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("CLI help is UTF-8")
}

#[test]
fn public_command_surface_is_available() {
    let root = hearthline(&["--help"]);
    for command in ["model", "capacity", "run", "replay"] {
        assert!(root.contains(command), "root help omits {command}");
    }

    let model = hearthline(&["model", "--help"]);
    for command in ["validate", "compile", "lock", "expand"] {
        assert!(model.contains(command), "model help omits {command}");
    }

    let capacity = hearthline(&["capacity", "report", "--help"]);
    assert!(capacity.contains("--format"));
    assert!(capacity.contains("text"));
    assert!(capacity.contains("json"));

    let replay = hearthline(&["replay", "--help"]);
    assert!(replay.contains("verify"));

    let lock = hearthline(&["model", "lock", "--help"]);
    assert!(lock.contains("--update"));
    assert!(lock.contains("--reason"));
}

#[test]
fn binary_reports_workspace_version() {
    let output = hearthline(&["--version"]);
    assert_eq!(output.trim(), "hearthline 0.3.2");
}

#[test]
fn public_model_capacity_run_and_replay_workflows_execute() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let packages = repository.join("packages");
    let temporary = tempfile::tempdir().expect("temporary CLI artifacts");

    let invoke = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_hearthline"))
            .args(args)
            .current_dir(&packages)
            .output()
            .expect("hearthline CLI starts")
    };

    for (args, expected) in [
        (vec!["model", "validate"], "validated"),
        (
            vec!["model", "compile", "--locked"],
            "compiled immutable model",
        ),
        (vec!["capacity", "report", "--format", "text"], "RESOURCE"),
        (
            vec!["capacity", "report", "--format", "json"],
            "assessments",
        ),
    ] {
        let output = invoke(&args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
    }
    assert!(
        !packages.join("packages").exists(),
        "generated catalogs must be anchored to the repository"
    );

    let expanded = temporary.path().join("expanded");
    let expanded_arg = expanded.to_string_lossy().into_owned();
    let output = invoke(&["model", "expand", "--output", &expanded_arg]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        expanded
            .join("body-preparation-industrial-water.json")
            .is_file()
    );

    let replay = temporary.path().join("overload.json");
    let replay_arg = replay.to_string_lossy().into_owned();
    let output = invoke(&[
        "run",
        "foundation-conduit-overload",
        "--record",
        &replay_arg,
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(replay.is_file());
    assert_recorded_runtime_contract(&replay, 4);
    let output = invoke(&["replay", "verify", &replay_arg]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let scenario_replay = temporary.path().join("scenario.json");
    let scenario_arg = scenario_replay.to_string_lossy().into_owned();
    let output = invoke(&["run", "customer-dns-lookup", "--record", &scenario_arg]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_recorded_runtime_contract(&scenario_replay, 1);
    let output = invoke(&["replay", "verify", &scenario_arg]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = invoke(&["run", "unknown-scenario"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown scenario"));
}

#[test]
fn every_representative_golden_replay_verifies_through_the_public_cli() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let packages = repository.join("packages");
    for artifact in [
        "body-preparation-local-autonomy.json",
        "conduit-overload.json",
        "forming-historian.json",
        "network-customer-dns.json",
        "recovery-firewall-session.json",
        "safety-firewall-isolation.json",
    ] {
        let path = repository.join("project/replays").join(artifact);
        let output = Command::new(env!("CARGO_BIN_EXE_hearthline"))
            .args(["replay", "verify", &path.to_string_lossy()])
            .current_dir(&packages)
            .output()
            .expect("hearthline replay verifier starts");
        assert!(
            output.status.success(),
            "{artifact}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn project_root_discovery_supports_explicit_ancestor_and_error_paths() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let binary = env!("CARGO_BIN_EXE_hearthline");

    let explicit = Command::new(binary)
        .args(["model", "validate"])
        .env("HEARTHLINE_PROJECT_ROOT", &repository)
        .current_dir(repository.join("packages"))
        .output()
        .expect("explicit-root validation starts");
    assert!(explicit.status.success());

    let unlocked = Command::new(binary)
        .args(["model", "compile"])
        .env_remove("HEARTHLINE_PROJECT_ROOT")
        .current_dir(repository.join("packages"))
        .output()
        .expect("unlocked compilation starts");
    assert!(unlocked.status.success());

    let missing_root = tempfile::tempdir().expect("missing project root");
    let invalid_explicit = Command::new(binary)
        .args(["model", "validate"])
        .env("HEARTHLINE_PROJECT_ROOT", missing_root.path())
        .current_dir(missing_root.path())
        .output()
        .expect("invalid explicit-root validation starts");
    assert!(!invalid_explicit.status.success());
    assert!(String::from_utf8_lossy(&invalid_explicit.stderr).contains("does not contain"));

    let undiscoverable = Command::new(binary)
        .args(["model", "validate"])
        .env_remove("HEARTHLINE_PROJECT_ROOT")
        .current_dir(missing_root.path())
        .output()
        .expect("ancestor discovery failure starts");
    assert!(!undiscoverable.status.success());
    assert!(String::from_utf8_lossy(&undiscoverable.stderr).contains("cannot find"));
}

fn assert_recorded_runtime_contract(path: &std::path::Path, minimum_inputs: usize) {
    let source = std::fs::read_to_string(path).expect("recorded replay");
    let artifact: serde_json::Value = serde_json::from_str(&source).expect("replay JSON");
    let manifest = &artifact["manifest"];
    assert_ne!(
        manifest["initial_state_digest"], manifest["model_digest"],
        "initial runtime state must not alias the immutable model digest"
    );
    assert!(
        manifest["inputs"].as_array().expect("ordered inputs").len() >= minimum_inputs,
        "replay omitted ordered run inputs"
    );
    let checkpoints = artifact["checkpoints"]
        .as_array()
        .expect("replay checkpoints");
    assert_eq!(checkpoints.len(), 2);
    assert_eq!(checkpoints[0]["event_index"], 0);
    for checkpoint in checkpoints {
        assert!(
            checkpoint["snapshot"]["cells"][0]["components"]
                .as_array()
                .is_some_and(|components| !components.is_empty())
        );
    }
}

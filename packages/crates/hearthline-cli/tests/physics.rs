use std::path::PathBuf;
use std::process::Command;

#[test]
fn physics_commands_expose_capacity_without_claiming_factory_qualification() {
    let binary = env!("CARGO_BIN_EXE_hearthline");
    let help = Command::new(binary)
        .args(["physics", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    for command in [
        "prove",
        "replay",
        "budget",
        "capacity",
        "geometry",
        "material",
        "proof-reference",
    ] {
        assert!(help.contains(command));
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let path = repository.join("project/physics/capacity/factory-baseline.yaml");
    let report = Command::new(binary)
        .args(["physics", "capacity"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(report["within_budget"], false);
    assert_eq!(report["qualification_gaps"].as_array().unwrap().len(), 5);
    let enforced = Command::new(binary)
        .args(["physics", "capacity"])
        .arg(&path)
        .arg("--require-fit")
        .output()
        .unwrap();
    assert!(!enforced.status.success());
    assert!(String::from_utf8_lossy(&enforced.stderr).contains("exceeds a reviewed"));
    let enforced_report: serde_json::Value = serde_json::from_slice(&enforced.stdout).unwrap();
    assert_eq!(report, enforced_report);

    let pooled = Command::new(binary)
        .args(["physics", "capacity"])
        .arg(&path)
        .arg("--pooled")
        .output()
        .unwrap();
    assert!(pooled.status.success());
    let pooled: serde_json::Value = serde_json::from_slice(&pooled.stdout).unwrap();
    assert_eq!(pooled["baseline"], report);
    assert_eq!(pooled["total_planned_bytes"], 411_911_842_u64);
    assert_eq!(pooled["within_budget"], true);
    assert_eq!(
        pooled["baseline"]["qualification_gaps"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    let enforced = Command::new(binary)
        .args(["physics", "capacity"])
        .arg(&path)
        .args(["--pooled", "--require-fit"])
        .output()
        .unwrap();
    assert!(enforced.status.success());
    assert_eq!(
        pooled,
        serde_json::from_slice::<serde_json::Value>(&enforced.stdout).unwrap()
    );
}

#[test]
fn source_backed_capacity_cli_pins_models_and_distinguishes_allocation_strategies() {
    let binary = env!("CARGO_BIN_EXE_hearthline");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let study = root.join("project/physics/capacity/factory-basin-study.yaml");
    let proof = root.join("project/physics/proofs/basin-wet-drying.yaml");
    let geometry = root.join("project/physics/geometry/original-basin.yaml");
    let material = root.join("project/physics/materials/synthetic-wet-drying.yaml");
    let reference = Command::new(binary)
        .args(["physics", "proof-reference"])
        .arg(&proof)
        .output()
        .unwrap();
    assert!(reference.status.success());
    let reference: serde_json::Value = serde_json::from_slice(&reference.stdout).unwrap();
    assert_eq!(reference["id"], "basin-wet-drying");
    assert_eq!(
        reference["sha256"],
        "b7372102f125870303c93a52b2d771b689e89a4b5b44e7f98d07c64be8efeb7a"
    );
    let missing = Command::new(binary)
        .args(["physics", "capacity"])
        .arg(&study)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing physics capacity proof"));
    for pooled in [false, true] {
        let mut command = Command::new(binary);
        command
            .args(["physics", "capacity"])
            .arg(&study)
            .arg("--proof")
            .arg(&proof)
            .arg("--geometry")
            .arg(&geometry)
            .arg("--material")
            .arg(&material)
            .arg("--require-fit");
        if pooled {
            command.arg("--pooled");
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.success(),
            pooled,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["within_budget"], pooled);
        assert_eq!(report["immutable_material_bytes"], 208);
        assert_eq!(
            report["total_planned_bytes"],
            if pooled {
                407_568_900_u64
            } else {
                5_935_411_196
            }
        );
        assert_eq!(report["compiler_peak_bytes"], 210_176_244);
    }
}

#[test]
fn capacity_source_files_share_a_bounded_document_budget() {
    let binary = env!("CARGO_BIN_EXE_hearthline");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("material.yaml");
    let text =
        std::fs::read_to_string(root.join("project/physics/materials/synthetic-wet-drying.yaml"))
            .unwrap();
    let text = text + "\n#" + &"padding ".repeat(1024 * 1024) + "\n";
    std::fs::write(&path, text).unwrap();
    let output = Command::new(binary)
        .args(["physics", "capacity"])
        .arg(root.join("project/physics/capacity/factory-baseline.yaml"))
        .arg("--material")
        .arg(&path)
        .arg("--material")
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exceed 16 MiB in total"));
}

#[test]
fn geometry_references_are_required_for_geometry_aware_proofs() {
    let binary = env!("CARGO_BIN_EXE_hearthline");
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let geometry = repository.join("project/physics/geometry/original-basin.yaml");
    let config = repository.join("project/physics/proofs/basin-drying.yaml");
    let reference = Command::new(binary)
        .args(["physics", "geometry"])
        .arg(&geometry)
        .output()
        .unwrap();
    assert!(reference.status.success());
    let reference: serde_json::Value = serde_json::from_slice(&reference.stdout).unwrap();
    assert_eq!(reference["id"], "original-basin");
    assert_eq!(
        reference["sha256"],
        "6c0ba190b3917bf87f1fc9ef41a09ffbfb5d425c94cf3fa7577943fa8ab6a829"
    );
    let missing = Command::new(binary)
        .args(["physics", "budget"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("digest reference"));
    let budget = Command::new(binary)
        .args(["physics", "budget"])
        .arg(config)
        .arg("--geometry")
        .arg(geometry)
        .output()
        .unwrap();
    assert!(
        budget.status.success(),
        "{}",
        String::from_utf8_lossy(&budget.stderr)
    );
    let budget: serde_json::Value = serde_json::from_slice(&budget.stdout).unwrap();
    assert_eq!(budget["cells"], 1072);
    assert_eq!(budget["compiler_scratch_bytes"], 22_237_912);
    assert_eq!(budget["immutable_mesh_bytes"], 3_388_748);
}

#[test]
fn material_commands_compile_budget_record_and_recompute_the_declared_law() {
    for (material, case, digest, bytes, final_digest) in [
        (
            "synthetic-moisture-diffusion",
            "variable-drying",
            "bd9f75a90ab0b44dfde4b6e584d0a16c65fc39f6670cf39d037432db34fc30fe",
            64,
            "8394f8def96e5faabbb99c1bd683937a3653a69068dfc9237682f3f7f689f25a",
        ),
        (
            "synthetic-drying-enthalpy",
            "enthalpy-drying",
            "782ffbe8b5a4746fa18867710fcdd6fd7053946309c741d8a3ea68fe0c023870",
            136,
            "b07cde3bd5592659d9af47bd7198e31e042137b1908bb81415055fb0e8463e6b",
        ),
        (
            "synthetic-wet-drying",
            "wet-drying",
            "8a86e1765f553c92e05c647a24ee91f454dfa094efec9057d9f34511b3e356c6",
            208,
            "e1d4685d0586e30f4a36867041bd4b5d04922c041ada1bd2c63af77e00cf9a07",
        ),
    ] {
        check_material_commands(material, case, digest, bytes, final_digest);
    }
}

fn check_material_commands(
    material_name: &str,
    case_name: &str,
    material_digest: &str,
    material_bytes: u64,
    final_digest: &str,
) {
    let binary = env!("CARGO_BIN_EXE_hearthline");
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let material = repository.join(format!("project/physics/materials/{material_name}.yaml"));
    let config = repository.join(format!("project/physics/proofs/{case_name}.yaml"));
    let reference = Command::new(binary)
        .args(["physics", "material"])
        .arg(&material)
        .output()
        .unwrap();
    assert!(reference.status.success());
    let reference: serde_json::Value = serde_json::from_slice(&reference.stdout).unwrap();
    assert_eq!(reference["sha256"], material_digest);
    let missing = Command::new(binary)
        .args(["physics", "prove"])
        .arg(&config)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("digest reference"));
    let budget = Command::new(binary)
        .args(["physics", "budget"])
        .arg(&config)
        .arg("--material")
        .arg(&material)
        .output()
        .unwrap();
    assert!(
        budget.status.success(),
        "{}",
        String::from_utf8_lossy(&budget.stderr)
    );
    let budget: serde_json::Value = serde_json::from_slice(&budget.stdout).unwrap();
    assert_eq!(budget["field_buffer_bytes"], 3456);
    assert_eq!(budget["immutable_material_bytes"], material_bytes);
    let temp = tempfile::tempdir().unwrap();
    let artifact = temp.path().join("recorded.json");
    let recorded = Command::new(binary)
        .args(["physics", "prove"])
        .arg(&config)
        .arg("--material")
        .arg(&material)
        .arg("--record")
        .arg(&artifact)
        .output()
        .unwrap();
    assert!(
        recorded.status.success(),
        "{}",
        String::from_utf8_lossy(&recorded.stderr)
    );
    assert!(String::from_utf8_lossy(&recorded.stdout).contains(final_digest));
    let replay = Command::new(binary)
        .args(["physics", "replay"])
        .arg(artifact)
        .output()
        .unwrap();
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
}

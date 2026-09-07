use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use hearthline_config::{ConfigRepository, ConnectionRepository, ScenarioRepository};
use serde_yaml_ng::Value;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

struct TempScenarioRoot(PathBuf);

impl TempScenarioRoot {
    fn new(label: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "hearthline-scenario-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary scenario root");
        Self(path)
    }

    fn write(&self, relative: &str, value: &Value) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("scenario parent")).expect("scenario directory");
        fs::write(
            path,
            serde_yaml_ng::to_string(value).expect("scenario YAML"),
        )
        .expect("write scenario");
    }
}

impl Drop for TempScenarioRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../project/config")
}

fn infrastructure() -> (ConfigRepository, ConnectionRepository) {
    let root = project_root();
    let appliances = ConfigRepository::load(root.join("appliances")).expect("appliances");
    let connections =
        ConnectionRepository::load(root.join("connections"), &appliances).expect("connections");
    (appliances, connections)
}

fn source(path: impl AsRef<Path>) -> Value {
    serde_yaml_ng::from_str(&fs::read_to_string(path).expect("scenario source"))
        .expect("scenario document")
}

fn customer_dns() -> Value {
    source(project_root().join("scenarios/customer/customer-dns-lookup.yaml"))
}

fn expect_load_error(value: &Value, expected: &str) {
    let (appliances, connections) = infrastructure();
    let root = TempScenarioRoot::new("invalid");
    let id = value["id"].as_str().expect("scenario ID");
    root.write(&format!("{id}.yaml"), value);
    let error = ScenarioRepository::load(&root.0, &appliances, &connections)
        .expect_err("invalid scenario repository");
    assert!(
        error.to_string().contains(expected),
        "expected {expected:?}, got {error}"
    );
}

#[test]
fn scenario_repository_enforces_filesystem_identity_and_duplicate_ids() {
    let (appliances, connections) = infrastructure();
    let empty = TempScenarioRoot::new("empty");
    assert!(
        ScenarioRepository::load(&empty.0, &appliances, &connections)
            .expect_err("empty repository")
            .to_string()
            .contains("contains no scenario YAML files")
    );

    let document = customer_dns();
    let wrong_name = TempScenarioRoot::new("wrong-name");
    wrong_name.write("wrong.yaml", &document);
    assert!(
        ScenarioRepository::load(&wrong_name.0, &appliances, &connections)
            .expect_err("wrong source filename")
            .to_string()
            .contains("must be named customer-dns-lookup.yaml")
    );

    let duplicate = TempScenarioRoot::new("duplicate");
    duplicate.write("a/customer-dns-lookup.yaml", &document);
    duplicate.write("b/customer-dns-lookup.yaml", &document);
    assert!(
        ScenarioRepository::load(&duplicate.0, &appliances, &connections)
            .expect_err("duplicate scenario ID")
            .to_string()
            .contains("duplicate scenario id customer-dns-lookup")
    );

    let valid = TempScenarioRoot::new("valid");
    valid.write("customer-dns-lookup.yaml", &document);
    let repository =
        ScenarioRepository::load(&valid.0, &appliances, &connections).expect("one scenario");
    assert_eq!(repository.len(), 1);
    assert!(!repository.is_empty());
    assert_eq!(repository.scenarios().count(), 1);
    assert_eq!(repository.summaries().len(), 1);
    assert!(repository.get("missing").is_none());
}

#[test]
fn scenario_repository_rejects_unknown_disconnected_and_misaddressed_participants() {
    let mut unknown = customer_dns();
    unknown["participants"]
        .as_sequence_mut()
        .expect("participants")
        .push(Value::String("missing-appliance".into()));
    expect_load_error(&unknown, "references unknown appliance missing-appliance");

    let mut disconnected = customer_dns();
    disconnected["participants"]
        .as_sequence_mut()
        .expect("participants")
        .push(Value::String("area-01-hmi-01".into()));
    expect_load_error(
        &disconnected,
        "participants disconnected from its execution roots",
    );

    let mut source_address = customer_dns();
    source_address["packet"]["source_ip"] = Value::String("192.168.0.99".into());
    expect_load_error(&source_address, "is not assigned to customer-pc-01");
}

#[test]
fn scenario_repository_rejects_security_defender_and_noop_recovery_drift() {
    let security_path =
        project_root().join("scenarios/security/customer-public-web-path-traversal-detected.yaml");
    let mut unknown_defender = source(&security_path);
    unknown_defender["security"]["defender"] = Value::String("missing-console".into());
    expect_load_error(
        &unknown_defender,
        "unknown security defender missing-console",
    );

    let mut wrong_kind = source(&security_path);
    wrong_kind["security"]["defender"] = Value::String("customer-pc-01".into());
    expect_load_error(&wrong_kind, "is not an operations console");

    let outage_path = project_root().join("scenarios/resilience/customer-wan-access-outage.yaml");
    let mut no_op_recovery = source(&outage_path);
    no_op_recovery["recovery"]["connection_overrides"][0]["operational"] = Value::Bool(false);
    expect_load_error(
        &no_op_recovery,
        "recovery does not change any selected topology state",
    );
}

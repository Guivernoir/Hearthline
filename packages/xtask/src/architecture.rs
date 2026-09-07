use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

const CRATES: &[&str] = &[
    "hearthline-model",
    "hearthline-engine",
    "hearthline-config",
    "hearthline-project",
    "hearthline-sim",
    "hearthline-operator",
    "hearthline-cli",
    "hearthline-api",
];

pub fn verify() -> Result<(), Box<dyn Error>> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&workspace)
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
    for package in metadata["packages"]
        .as_array()
        .ok_or("metadata packages are missing")?
    {
        let Some(name) = package["name"].as_str() else {
            continue;
        };
        if !CRATES.contains(&name) {
            continue;
        }
        let local = package["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|dependency| dependency["source"].is_null())
            .filter_map(|dependency| dependency["name"].as_str())
            .filter(|dependency| CRATES.contains(dependency))
            .map(str::to_owned)
            .collect();
        dependencies.insert(name.into(), local);
    }
    let rank = CRATES
        .iter()
        .enumerate()
        .map(|(index, name)| (*name, index))
        .collect::<BTreeMap<_, _>>();
    let adapters = BTreeSet::from(["hearthline-cli", "hearthline-api"]);
    let mut failures = Vec::new();
    for (crate_name, crate_dependencies) in &dependencies {
        let crate_rank = rank[crate_name.as_str()];
        for dependency in crate_dependencies {
            let dependency_rank = rank[dependency.as_str()];
            let upward = dependency_rank >= crate_rank;
            if upward && !adapters.contains(crate_name.as_str()) {
                failures.push(format!("{crate_name} depends upward on {dependency}"));
            }
            if crate_name == "hearthline-model" {
                failures.push(format!("hearthline-model depends on {dependency}"));
            }
        }
    }
    for required in CRATES {
        if !dependencies.contains_key(*required) {
            failures.push(format!("workspace is missing {required}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "architecture policy violations:\n- {}",
            failures.join("\n- ")
        )
        .into())
    }
}

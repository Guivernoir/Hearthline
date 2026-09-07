use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use syn::{Expr, ExprLit, Item, Lit};

pub fn verify() -> Result<(), Box<dyn Error>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let policy_path = repository.join("project/standards/engine-capacities.json");
    let policy: Value = serde_json::from_str(&fs::read_to_string(&policy_path)?)?;
    if policy["schemaVersion"] != "0.1.0" {
        return Err("engine capacity policy has an unsupported schema".into());
    }
    let expected = policy["capacities"]
        .as_object()
        .ok_or("engine capacity policy omits capacities")?
        .iter()
        .map(|(name, value)| {
            let value = value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| format!("capacity {name} is not a positive integer"))?;
            if value == 0 {
                return Err(format!("capacity {name} is zero"));
            }
            Ok((name.clone(), value))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let actual =
        engine_capacities(&repository.join("packages/crates/hearthline-engine/src/capacity.rs"))?;
    if actual != expected {
        return Err(format!(
            "engine structural capacities differ from project/standards/engine-capacities.json\nexpected: {expected:?}\nactual: {actual:?}\nrecord the reviewed ADR and evidence before updating the registry"
        )
        .into());
    }

    for field in ["decision", "benchmark"] {
        require_evidence_path(&repository, &policy, field)?;
    }
    let tests = policy["tests"]
        .as_array()
        .ok_or("engine capacity policy omits saturation tests")?;
    if tests.is_empty() {
        return Err("engine capacity policy requires saturation tests".into());
    }
    for test in tests {
        let relative = test
            .as_str()
            .ok_or("engine capacity test evidence must be a path")?;
        require_file(&repository, relative, "test")?;
    }
    let decision = policy["decision"]
        .as_str()
        .ok_or("capacity decision must be a path")?;
    let decision_source = fs::read_to_string(repository.join(decision))?;
    for required in ["Status: Accepted", "Owner:", "Review date:"] {
        if !decision_source.contains(required) {
            return Err(format!("capacity ADR {decision} omits {required}").into());
        }
    }
    Ok(())
}

fn engine_capacities(path: &Path) -> Result<BTreeMap<String, usize>, Box<dyn Error>> {
    let syntax = syn::parse_file(&fs::read_to_string(path)?)?;
    let mut capacities = BTreeMap::new();
    for item in syntax.items {
        let Item::Const(item) = item else { continue };
        let name = item.ident.to_string();
        if !name.ends_with("_CAPACITY") {
            continue;
        }
        let Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) = *item.expr
        else {
            return Err(format!("structural capacity {name} must be an integer literal").into());
        };
        capacities.insert(name, value.base10_parse()?);
    }
    if capacities.is_empty() {
        return Err("engine declares no structural capacities".into());
    }
    Ok(capacities)
}

fn require_evidence_path(
    repository: &Path,
    policy: &Value,
    field: &str,
) -> Result<(), Box<dyn Error>> {
    let relative = policy[field]
        .as_str()
        .ok_or_else(|| format!("engine capacity policy omits {field}"))?;
    require_file(repository, relative, field)
}

fn require_file(repository: &Path, relative: &str, kind: &str) -> Result<(), Box<dyn Error>> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        })
        || !repository.join(path).is_file()
    {
        return Err(format!("engine capacity {kind} evidence {relative} is invalid").into());
    }
    Ok(())
}

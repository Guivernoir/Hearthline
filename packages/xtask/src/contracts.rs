use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use hearthline_api::contracts::generated_contract_files;

pub fn write() -> Result<(), Box<dyn Error>> {
    let root = repository_root();
    for (relative, source) in generated_contract_files()? {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("hearthline-contract-tmp");
        fs::write(&temporary, source)?;
        fs::rename(temporary, path)?;
    }
    println!("Generated OpenAPI, JSON Schema, and TypeScript contracts.");
    Ok(())
}

pub fn check() -> Result<(), Box<dyn Error>> {
    let root = repository_root();
    let mut drift = Vec::new();
    for (relative, expected) in generated_contract_files()? {
        let path = root.join(&relative);
        match fs::read_to_string(&path) {
            Ok(actual) if actual == expected => {}
            Ok(_) => drift.push(format!("{} differs", relative.display())),
            Err(_) => drift.push(format!("{} is missing", relative.display())),
        }
    }
    if drift.is_empty() {
        println!("Generated API contracts are current.");
        Ok(())
    } else {
        Err(format!(
            "generated contract drift:\n- {}\nrun `cargo xtask contracts --write`",
            drift.join("\n- ")
        )
        .into())
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

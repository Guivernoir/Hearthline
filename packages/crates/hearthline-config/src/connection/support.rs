use std::fs;
use std::path::{Path, PathBuf};

use crate::appliance::ConfigError;

pub(super) fn collect_yaml_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), ConfigError> {
    let entries = fs::read_dir(root)
        .map_err(|error| ConfigError::new(format!("cannot read {}: {error}", root.display())))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ConfigError::new(format!(
                "cannot read entry under {}: {error}",
                root.display()
            ))
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_paths(&path, paths)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("yaml") {
            paths.push(path);
        }
    }
    Ok(())
}

pub(super) const fn default_capacity() -> u64 {
    1_000
}

pub(super) const fn default_true() -> bool {
    true
}

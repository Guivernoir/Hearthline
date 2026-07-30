use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use hearthline_model::ComponentKind;
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(super) fn with_path(self, path: &Path) -> Self {
        Self::new(format!("{}: {}", path.display(), self.message))
    }
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ConfigError {}

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

pub(super) fn deserialize_component_kind<'de, D>(deserializer: D) -> Result<ComponentKind, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    ComponentKind::from_str(&value).map_err(serde::de::Error::custom)
}

pub(super) fn require_text(field: &str, value: &str) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        Err(ConfigError::new(format!("{field} cannot be empty")))
    } else {
        Ok(())
    }
}

pub(super) const fn default_true() -> bool {
    true
}

pub(super) fn join_numbers(values: &[u16]) -> String {
    values
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn source_revision(source: &str) -> String {
    let digest = Sha256::digest(source.as_bytes());
    let mut revision = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut revision, "{byte:02x}").expect("writing to a String cannot fail");
    }
    revision
}

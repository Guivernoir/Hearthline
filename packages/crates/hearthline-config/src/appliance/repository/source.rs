use std::fs;
use std::path::{Component, Path, PathBuf};

use super::ConfigRepository;
use crate::ConfigError;

impl ConfigRepository {
    pub(crate) fn read_project_source(
        &self,
        reference: &str,
    ) -> Result<(PathBuf, String), ConfigError> {
        let relative = Path::new(reference);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ConfigError::new(format!(
                "project source reference {reference} must be a normalized relative path"
            )));
        }
        let project_root = fs::canonicalize(&self.project_root).map_err(|error| {
            ConfigError::new(format!(
                "cannot resolve project root {}: {error}",
                self.project_root.display()
            ))
        })?;
        let path = fs::canonicalize(self.project_root.join(relative)).map_err(|error| {
            ConfigError::new(format!(
                "cannot resolve project source {reference}: {error}"
            ))
        })?;
        if !path.starts_with(&project_root) {
            return Err(ConfigError::new(format!(
                "project source reference {reference} escapes the project root"
            )));
        }
        let source = fs::read_to_string(&path).map_err(|error| {
            ConfigError::new(format!("cannot read project source {reference}: {error}"))
        })?;
        Ok((path, source))
    }
}

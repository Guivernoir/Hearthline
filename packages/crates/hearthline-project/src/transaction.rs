use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::{
    CapacityPlan, CapacityResource, CapacityStatus, ProjectCompiler, ProjectError, SourceDigest,
};

const MAX_DOCUMENT_BYTES: usize = 512 * 1024;
const MAX_YAML_INDENT: usize = 64;
const TRANSACTION_SCHEMA_VERSION: &str = "0.1.0";

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct DraftId(pub u64);

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftChange {
    pub path: PathBuf,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftDiagnostic {
    pub severity: String,
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftPreview {
    pub draft: DraftId,
    pub base_revision: String,
    pub candidate_revision: Option<String>,
    pub diagnostics: Vec<DraftDiagnostic>,
    pub added_objects: Vec<String>,
    pub removed_objects: Vec<String>,
    pub modified_objects: Vec<String>,
    pub capacity: Option<CapacityPlan>,
    pub capacity_deltas: Vec<CapacityDelta>,
    pub affected_scenarios: Vec<String>,
    pub preview_catalogs: Vec<SourceDigest>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapacityDelta {
    pub resource: CapacityResource,
    pub scope: String,
    pub previous_demand: Option<usize>,
    pub candidate_demand: Option<usize>,
    pub candidate_status: Option<CapacityStatus>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftCommit {
    pub draft: DraftId,
    pub previous_revision: String,
    pub revision: String,
    pub changed_paths: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct ModelDraft {
    pub id: DraftId,
    pub base_revision: String,
    pub changes: BTreeMap<PathBuf, Option<String>>,
}

#[derive(Debug)]
pub struct ModelTransactionStore {
    repository_root: PathBuf,
    config_root: PathBuf,
    drafts: BTreeMap<DraftId, ModelDraft>,
    next_id: u64,
}

impl ModelTransactionStore {
    pub fn new(repository_root: impl Into<PathBuf>) -> Result<Self, TransactionError> {
        let repository_root =
            fs::canonicalize(repository_root.into()).map_err(TransactionError::io)?;
        let config_root = repository_root.join("project/config");
        let mut store = Self {
            repository_root,
            config_root,
            drafts: BTreeMap::new(),
            next_id: 1,
        };
        store.recover()?;
        Ok(store)
    }

    pub fn create(&mut self) -> Result<ModelDraft, TransactionError> {
        let compiled = ProjectCompiler::new(&self.config_root).compile("draft base")?;
        let id = DraftId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let draft = ModelDraft {
            id,
            base_revision: compiled.digest().into(),
            changes: BTreeMap::new(),
        };
        self.drafts.insert(id, draft.clone());
        Ok(draft)
    }

    pub fn update(&mut self, id: DraftId, change: DraftChange) -> Result<(), TransactionError> {
        let path = validate_source_path(&change.path)?;
        reject_symlink_ancestors(&self.repository_root, &path)?;
        if let Some(source) = &change.source {
            validate_source(source)?;
        }
        self.drafts
            .get_mut(&id)
            .ok_or(TransactionError::UnknownDraft(id))?
            .changes
            .insert(path, change.source);
        Ok(())
    }

    pub fn discard(&mut self, id: DraftId) -> bool {
        self.drafts.remove(&id).is_some()
    }

    pub fn preview(&self, id: DraftId) -> Result<DraftPreview, TransactionError> {
        let draft = self
            .drafts
            .get(&id)
            .ok_or(TransactionError::UnknownDraft(id))?;
        let current = ProjectCompiler::new(&self.config_root).compile("draft comparison")?;
        let staging = self.stage_overlay(draft)?;
        match ProjectCompiler::new(staging.path().join("project/config")).compile("draft preview") {
            Ok(candidate) => {
                let object_changes = object_diff(
                    &current.model_lock().object_digests,
                    &candidate.model_lock().object_digests,
                );
                let changed_objects = object_changes
                    .added
                    .iter()
                    .chain(&object_changes.removed)
                    .chain(&object_changes.modified)
                    .map(String::as_str)
                    .collect::<std::collections::BTreeSet<_>>();
                let affected_scenarios = affected_scenarios(&current, &candidate, &changed_objects);
                Ok(DraftPreview {
                    draft: id,
                    base_revision: draft.base_revision.clone(),
                    candidate_revision: Some(candidate.digest().into()),
                    diagnostics: Vec::new(),
                    added_objects: object_changes.added,
                    removed_objects: object_changes.removed,
                    modified_objects: object_changes.modified,
                    capacity: Some(candidate.capacity().clone()),
                    capacity_deltas: capacity_deltas(current.capacity(), candidate.capacity()),
                    affected_scenarios,
                    preview_catalogs: candidate.model_lock().generated_catalogs.clone(),
                })
            }
            Err(error) => Ok(DraftPreview {
                draft: id,
                base_revision: draft.base_revision.clone(),
                candidate_revision: None,
                diagnostics: vec![DraftDiagnostic {
                    severity: "error".into(),
                    path: draft.changes.keys().next().cloned(),
                    line: diagnostic_line(&error.to_string()),
                    message: error.to_string(),
                }],
                added_objects: Vec::new(),
                removed_objects: Vec::new(),
                modified_objects: Vec::new(),
                capacity: None,
                capacity_deltas: Vec::new(),
                affected_scenarios: Vec::new(),
                preview_catalogs: Vec::new(),
            }),
        }
    }

    pub fn commit(
        &mut self,
        id: DraftId,
        expected_revision: &str,
        reason: &str,
    ) -> Result<DraftCommit, TransactionError> {
        if reason.trim().is_empty() {
            return Err(TransactionError::Validation(
                "commit reason cannot be empty".into(),
            ));
        }
        let draft = self
            .drafts
            .get(&id)
            .cloned()
            .ok_or(TransactionError::UnknownDraft(id))?;
        let current = ProjectCompiler::new(&self.config_root).compile("transaction current")?;
        if draft.base_revision != current.digest() || expected_revision != current.digest() {
            return Err(TransactionError::StaleRevision {
                expected: expected_revision.into(),
                actual: current.digest().into(),
            });
        }
        let staging = self.stage_overlay(&draft)?;
        let candidate =
            ProjectCompiler::new(staging.path().join("project/config")).compile(reason)?;
        let mut writes = draft
            .changes
            .iter()
            .filter_map(|(path, source)| {
                source.as_ref().map(|source| (path.clone(), source.clone()))
            })
            .collect::<Vec<_>>();
        writes.push((
            PathBuf::from("project/config/model.lock.json"),
            candidate.model_lock().to_json()?,
        ));
        writes.extend(candidate.generated_catalogs()?);
        let deletes = draft
            .changes
            .iter()
            .filter_map(|(path, source)| source.is_none().then_some(path.clone()))
            .collect::<Vec<_>>();
        let changed_paths = writes
            .iter()
            .map(|(path, _)| path.clone())
            .chain(deletes.iter().cloned())
            .collect::<Vec<_>>();
        self.apply_transaction(id, &writes, &deletes)?;
        self.drafts.remove(&id);
        Ok(DraftCommit {
            draft: id,
            previous_revision: current.digest().into(),
            revision: candidate.digest().into(),
            changed_paths,
        })
    }

    fn stage_overlay(&self, draft: &ModelDraft) -> Result<TempDir, TransactionError> {
        let temporary = tempfile::tempdir().map_err(TransactionError::io)?;
        copy_tree(
            &self.repository_root.join("project/config"),
            &temporary.path().join("project/config"),
        )?;
        copy_tree(
            &self.repository_root.join("project/control"),
            &temporary.path().join("project/control"),
        )?;
        for (path, source) in &draft.changes {
            let target = temporary.path().join(path);
            if let Some(source) = source {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(TransactionError::io)?;
                }
                fs::write(target, source).map_err(TransactionError::io)?;
            } else if target.exists() {
                fs::remove_file(target).map_err(TransactionError::io)?;
            }
        }
        Ok(temporary)
    }

    fn apply_transaction(
        &self,
        id: DraftId,
        writes: &[(PathBuf, String)],
        deletes: &[PathBuf],
    ) -> Result<(), TransactionError> {
        let journal_path = self.repository_root.join(".hearthline-transaction.json");
        let mut entries = Vec::new();
        for (index, (relative, source)) in writes.iter().enumerate() {
            let target = self.repository_root.join(relative);
            let staged = target.with_extension(format!("hearthline-stage-{}-{index}", id.0));
            let backup = target.with_extension(format!("hearthline-backup-{}-{index}", id.0));
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(TransactionError::io)?;
            }
            write_synced(&staged, source.as_bytes())?;
            entries.push(JournalEntry {
                target,
                staged: Some(staged),
                backup,
            });
        }
        for (index, relative) in deletes.iter().enumerate() {
            let target = self.repository_root.join(relative);
            entries.push(JournalEntry {
                staged: None,
                backup: target.with_extension(format!("hearthline-backup-{}-delete-{index}", id.0)),
                target,
            });
        }
        let journal = Journal {
            schema_version: TRANSACTION_SCHEMA_VERSION.into(),
            entries,
        };
        validate_journal(&self.repository_root, &journal)?;
        write_synced(
            &journal_path,
            serde_json::to_vec_pretty(&journal)
                .map_err(TransactionError::json)?
                .as_slice(),
        )?;
        sync_directory(&self.repository_root)?;
        if let Err(error) = install(&journal) {
            let _ = rollback(&journal);
            return Err(error);
        }
        cleanup(&journal)?;
        fs::remove_file(journal_path).map_err(TransactionError::io)?;
        sync_directory(&self.repository_root)?;
        Ok(())
    }

    fn recover(&mut self) -> Result<(), TransactionError> {
        let journal_path = self.repository_root.join(".hearthline-transaction.json");
        if !journal_path.exists() {
            return Ok(());
        }
        let journal: Journal =
            serde_json::from_slice(&fs::read(&journal_path).map_err(TransactionError::io)?)
                .map_err(TransactionError::json)?;
        validate_journal(&self.repository_root, &journal)?;
        rollback(&journal)?;
        cleanup(&journal)?;
        fs::remove_file(journal_path).map_err(TransactionError::io)?;
        sync_directory(&self.repository_root)
    }
}

#[derive(Debug)]
pub enum TransactionError {
    UnknownDraft(DraftId),
    StaleRevision { expected: String, actual: String },
    Validation(String),
    Io(String),
    Project(ProjectError),
}

impl TransactionError {
    fn io(error: impl Display) -> Self {
        Self::Io(error.to_string())
    }
    fn json(error: impl Display) -> Self {
        Self::Io(error.to_string())
    }
}

impl Display for TransactionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDraft(id) => write!(formatter, "unknown model draft {}", id.0),
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "stale model revision {expected}; current revision is {actual}"
            ),
            Self::Validation(detail) => write!(formatter, "draft validation failed: {detail}"),
            Self::Io(detail) => write!(formatter, "model transaction I/O failed: {detail}"),
            Self::Project(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for TransactionError {}

impl From<ProjectError> for TransactionError {
    fn from(error: ProjectError) -> Self {
        Self::Project(error)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Journal {
    schema_version: String,
    entries: Vec<JournalEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JournalEntry {
    target: PathBuf,
    staged: Option<PathBuf>,
    backup: PathBuf,
}

fn validate_journal(root: &Path, journal: &Journal) -> Result<(), TransactionError> {
    if journal.schema_version != TRANSACTION_SCHEMA_VERSION {
        return Err(TransactionError::Validation(format!(
            "unsupported transaction journal schema {}",
            journal.schema_version
        )));
    }
    for entry in &journal.entries {
        for path in [
            Some(&entry.target),
            entry.staged.as_ref(),
            Some(&entry.backup),
        ]
        .into_iter()
        .flatten()
        {
            let relative = path.strip_prefix(root).map_err(|_| {
                TransactionError::Validation(format!(
                    "transaction journal path {} escapes the repository",
                    path.display()
                ))
            })?;
            if relative.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            }) {
                return Err(TransactionError::Validation(format!(
                    "transaction journal path {} is not normalized",
                    path.display()
                )));
            }
            if !relative.starts_with("project/config")
                && !relative.starts_with("packages/web/src/generated")
            {
                return Err(TransactionError::Validation(format!(
                    "transaction journal path {} is outside an allowed transaction root",
                    path.display()
                )));
            }
        }
        if entry.backup.parent() != entry.target.parent()
            || entry
                .staged
                .as_ref()
                .is_some_and(|staged| staged.parent() != entry.target.parent())
        {
            return Err(TransactionError::Validation(
                "transaction staging and backup files must share the target directory".into(),
            ));
        }
    }
    Ok(())
}

mod preview;
use preview::*;

fn copy_tree(source: &Path, destination: &Path) -> Result<(), TransactionError> {
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(TransactionError::io)?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(TransactionError::io)?;
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(target).map_err(TransactionError::io)?;
        } else if entry.file_type().is_file() {
            fs::copy(entry.path(), target).map_err(TransactionError::io)?;
        }
    }
    Ok(())
}

fn write_synced(path: &Path, source: &[u8]) -> Result<(), TransactionError> {
    let mut file = File::create(path).map_err(TransactionError::io)?;
    file.write_all(source).map_err(TransactionError::io)?;
    file.sync_all().map_err(TransactionError::io)
}

fn install(journal: &Journal) -> Result<(), TransactionError> {
    for entry in &journal.entries {
        if entry.target.exists() {
            fs::rename(&entry.target, &entry.backup).map_err(TransactionError::io)?;
        }
        if let Some(staged) = &entry.staged {
            fs::rename(staged, &entry.target).map_err(TransactionError::io)?;
        }
        if let Some(parent) = entry.target.parent() {
            sync_directory(parent)?;
        }
    }
    Ok(())
}

fn rollback(journal: &Journal) -> Result<(), TransactionError> {
    for entry in journal.entries.iter().rev() {
        if entry.target.exists() {
            fs::remove_file(&entry.target).map_err(TransactionError::io)?;
        }
        if entry.backup.exists() {
            fs::rename(&entry.backup, &entry.target).map_err(TransactionError::io)?;
        }
        if let Some(staged) = &entry.staged
            && staged.exists()
        {
            fs::remove_file(staged).map_err(TransactionError::io)?;
        }
        if let Some(parent) = entry.target.parent() {
            sync_directory(parent)?;
        }
    }
    Ok(())
}

fn cleanup(journal: &Journal) -> Result<(), TransactionError> {
    for entry in &journal.entries {
        if entry.backup.exists() {
            fs::remove_file(&entry.backup).map_err(TransactionError::io)?;
        }
        if let Some(staged) = &entry.staged
            && staged.exists()
        {
            fs::remove_file(staged).map_err(TransactionError::io)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), TransactionError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(TransactionError::io)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), TransactionError> {
    Ok(())
}

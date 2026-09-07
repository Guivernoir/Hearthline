//! Host-side compilation of Hearthline source documents into immutable models.

mod blueprint;
mod capacity;
mod compiled;
mod locking;
mod runtime_plan;
mod source;
mod transaction;

pub use blueprint::{
    BLUEPRINT_SCHEMA_VERSION, BlueprintDefinition, BlueprintExport, BlueprintExportKind,
    BlueprintInstance, BlueprintNode, BlueprintParameter, BlueprintParameterKind,
    BlueprintParameterValue, BlueprintRepository, ExpandedBlueprint, ExpandedBlueprintConnection,
    ExpandedBlueprintNode,
};
pub use capacity::{
    CapacityAssessment, CapacityEvidenceRecord, CapacityPlan, CapacityResource, CapacityStatus,
};
pub use compiled::{
    COMPILED_PROJECT_SCHEMA_VERSION, CompiledProject, ProjectCompiler, ProjectError,
};
pub use locking::{
    MODEL_LOCK_SCHEMA_VERSION, ModelLock, ModelLockStatus, PartitionAssignment, SourceDigest,
};
pub use runtime_plan::{
    CapacityObservation, ObservedCapacityResource, ProjectRuntimePlan, RuntimeBoundaryPlan,
    RuntimePartitionPlan, compile_runtime_plan,
};
pub use source::{MODEL_SOURCE_SCHEMA_VERSION, ModelSourceDocument, ModelSourceKind};
pub use transaction::{
    CapacityDelta, DraftChange, DraftCommit, DraftDiagnostic, DraftId, DraftPreview, ModelDraft,
    ModelTransactionStore, TransactionError,
};

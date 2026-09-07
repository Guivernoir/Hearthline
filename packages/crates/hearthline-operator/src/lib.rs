//! Operator-facing state that never owns authoritative plant simulation state.

mod command;
mod gateway;
mod projection;
mod session;
mod workstation;

pub use command::{OperatorCommand, OperatorCommandEnvelope, OperatorCommandError};
pub use gateway::PlantOperatorGateway;
pub use projection::{OperatorProjection, ProjectionStatus};
pub use session::{OperatorSession, OperatorSessionError, OperatorSessionStore};
pub use workstation::{
    BrowserNavigation, WORKSTATION_DNS_TTL_MS, WORKSTATION_SCHEMA_VERSION, WorkstationAction,
    WorkstationActionKind, WorkstationActionReport, WorkstationActionStatus, WorkstationArpEntry,
    WorkstationDnsCacheEntry, WorkstationInterface, WorkstationNetworkState, WorkstationProfile,
    WorkstationSession, run_workstation_action, run_workstation_action_with_session,
    workstation_profile,
};

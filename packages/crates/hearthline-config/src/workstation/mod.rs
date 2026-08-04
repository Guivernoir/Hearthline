mod curl;
mod executor;
mod http;
mod schema;
mod shell;
mod support;

pub use executor::run_workstation_action;
pub use schema::{
    BrowserNavigation, WORKSTATION_SCHEMA_VERSION, WorkstationAction, WorkstationActionKind,
    WorkstationActionReport, WorkstationActionStatus, WorkstationInterface, WorkstationProfile,
    workstation_profile,
};

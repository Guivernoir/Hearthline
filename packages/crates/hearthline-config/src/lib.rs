//! Host-side configuration adapters for Hearthline.
//!
//! Filesystem access, YAML parsing, generated frontend catalogs, and editable
//! source documents live here so the deterministic runtime remains independent
//! from an allocator and operating-system services.

mod appliance;
mod connection;

pub use appliance::{
    APPLIANCE_SCHEMA_VERSION, ApplianceConfig, BehaviorConfig, ConfigError, ConfigRepository,
    FRONTEND_CATALOG_SCHEMA_VERSION, FrontendAppliance, FrontendApplianceCatalog,
    FrontendInterface, InterfaceConfig, InterfaceMode, Lifecycle, ListenerConfig, LoadedAppliance,
    NatTranslationConfig, PolicyAction, PolicyRuleConfig, RenderBinding, RenderMode, RouteConfig,
};
pub use connection::{
    CONNECTION_SCHEMA_VERSION, ConnectionConfig, ConnectionDirection, ConnectionEndpoint,
    ConnectionEndpoints, ConnectionProperties, ConnectionRepository, ConnectorDropReason,
    ConnectorPortProfile, ConnectorTransit, FrontendConnection, FrontendConnectionEndpoint,
    LoadedConnection, SimulatedConnector, TransportKind,
};

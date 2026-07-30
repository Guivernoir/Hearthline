//! Deterministic component behavior for Hearthline.
//!
//! The engine models appliance-level decisions and produces explained effects.
//! It is intentionally not a vendor firmware emulator or bit-level packet
//! simulator.

mod catalog;
mod component;
mod config;
mod connection;
mod firewall;
mod link;
mod media;
mod nat;
mod port;
mod process;
mod router;
mod service;
mod simulator;
mod switch;

pub use catalog::{
    ApplianceContract, RENDERED_ROLE_CONTRACTS, RenderedRoleContract, appliance_contracts,
};
pub use component::{
    DropReason, Effect, NetworkIngress, ProcessEffect, SimulatedComponent, SimulationEvent,
};
pub use config::{
    APPLIANCE_SCHEMA_VERSION, ApplianceConfig, ConfigError, ConfigRepository,
    FRONTEND_CATALOG_SCHEMA_VERSION, FrontendAppliance, FrontendApplianceCatalog, LoadedAppliance,
};
pub use connection::{
    CONNECTION_SCHEMA_VERSION, ConnectionConfig, ConnectionDirection, ConnectionEndpoint,
    ConnectionProperties, ConnectionRepository, ConnectorDropReason, ConnectorTransit,
    FrontendConnection, FrontendConnectionEndpoint, LoadedConnection, SimulatedConnector,
    TransportKind,
};
pub use firewall::{FirewallAction, FirewallRule, StatefulFirewall};
pub use link::{LinkAppliance, LinkMode};
pub use media::{
    CarrierMedium, ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, FiberMedium,
    FiberMode, FieldWiringMedium, MediumKind, RadioMedium, SimulatedMedium, TelephoneMedium,
    VirtualMedium,
};
pub use nat::{NatRouter, StaticNat};
pub use port::{
    PortDuplex, PortHardwareKind, PortSettings, PortState, PortStateConfig, SimulatedPort,
    appliance_supports_port,
};
pub use process::{
    Actuator, Comparison, FieldSensor, IoDirection, LogicRule, OperatorInterface, RemoteIo,
    SafetyInterface, VirtualPlc,
};
pub use router::{Router, RoutingTable};
pub use service::{DnsServer, PassiveSensor, ReverseProxyWaf, ServiceNode};
pub use simulator::{SimulationError, Simulator, TraceEntry};
pub use switch::{LearningSwitch, SwitchPort, WirelessAccessPoint};

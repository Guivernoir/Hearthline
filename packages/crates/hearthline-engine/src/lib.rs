//! Deterministic component behavior for Hearthline.
//!
//! The engine models appliance-level decisions and produces explained effects.
//! It is intentionally not a vendor firmware emulator or bit-level packet
//! simulator.

#![no_std]

mod catalog;
mod industrial;
mod network;
mod physical;
mod runtime;

pub use catalog::{
    ApplianceContract, RENDERED_ROLE_CONTRACTS, RenderedRoleContract, appliance_contracts,
};
pub use industrial::{
    Actuator, Comparison, FieldSensor, IoDirection, LogicRule, OperatorInterface, RemoteIo,
    SafetyInterface, VirtualPlc,
};
pub use network::{
    DnsServer, FirewallAction, FirewallHaRuntimeConfig, FirewallHaStatus, FirewallRule,
    FirstHopAddress, HttpInspectionRule, HttpInspectionTarget, Layer3Switch, LearningSwitch,
    LinkAppliance, LinkMode, MacTableEntry, NatRouter, NeighborEntry, NeighborState, PassiveSensor,
    ReverseProxyWaf, RoutedInterface, Router, RoutingTable, ServiceNode, StatefulFirewall,
    StaticNat, StaticNatError, SwitchAggregationGroup, SwitchPort, WirelessAccessPoint,
};
pub use physical::{
    CarrierMedium, ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, FiberMedium,
    FiberMode, FieldWiringMedium, LinkDirection, LinkEndpoint, MediaDropReason, MediaError,
    MediaFacts, MediaLink, MediaLinkConfig, MediaLinkError, MediaText, MediaTransit, MediumKind,
    PortDuplex, PortHardwareKind, PortSettings, PortState, PortStateConfig, RadioMedium,
    SimulatedMedium, SimulatedPort, TelephoneMedium, VirtualMedium, appliance_supports_port,
};
pub use runtime::{
    DropReason, EFFECT_CAPACITY, Effect, EffectList, FirewallHaControl, Ipv4Egress, NetworkIngress,
    ProcessEffect, SimulatedComponent, SimulationError, SimulationEvent, Simulator, TraceEntry,
};

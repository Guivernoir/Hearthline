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
    DnsServer, FirewallAction, FirewallRule, LearningSwitch, LinkAppliance, LinkMode, NatRouter,
    PassiveSensor, ReverseProxyWaf, Router, RoutingTable, ServiceNode, StatefulFirewall, StaticNat,
    SwitchPort, WirelessAccessPoint,
};
pub use physical::{
    CarrierMedium, ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, FiberMedium,
    FiberMode, FieldWiringMedium, MediaError, MediaFacts, MediaText, MediumKind, PortDuplex,
    PortHardwareKind, PortSettings, PortState, PortStateConfig, RadioMedium, SimulatedMedium,
    SimulatedPort, TelephoneMedium, VirtualMedium, appliance_supports_port,
};
pub use runtime::{
    DropReason, EFFECT_CAPACITY, Effect, EffectList, NetworkIngress, ProcessEffect,
    SimulatedComponent, SimulationError, SimulationEvent, Simulator, TraceEntry,
};

#![no_std]

//! Shared, deterministic domain contracts for Hearthline.
//!
//! This crate contains no simulation policy. It defines stable identifiers,
//! appliance categories, network messages, routes, services, and process
//! signals consumed by the engine and future configuration parser.

mod application;
mod component;
mod network;
mod process;
mod storage;

pub use application::{ApplicationData, HttpDocument, HttpMethod};
pub use component::{
    BehaviorFamily, ComponentId, ComponentKind, ComponentKindParseError, IdentifierError, PortId,
    ServiceKind,
};
pub use network::{
    ArpOperation, ArpPacket, EthernetFrame, FirewallHaMessage, FlowKey, IcmpMessage, Ipv4Cidr,
    Ipv4InterfaceAddress, Ipv4Packet, MacAddress, NetworkAddressParseError, NetworkPayload, Route,
    TcpFlags, TcpSegment, Transport, TransportProtocol, UdpDatagram, VlanId,
};
pub use process::{ProcessCommand, ProcessEvent, ProcessSignal, SignalValue};
pub use storage::{CapacityError, Text};

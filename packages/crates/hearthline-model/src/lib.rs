#![no_std]

//! Shared, deterministic domain contracts for Hearthline.
//!
//! This crate contains no simulation policy. It defines stable identifiers,
//! appliance categories, network messages, routes, services, and process
//! signals consumed by the engine and future configuration parser.

mod component;
mod network;
mod process;
mod storage;

pub use component::{
    BehaviorFamily, ComponentId, ComponentKind, ComponentKindParseError, IdentifierError, PortId,
    ServiceKind,
};
pub use network::{
    ApplicationData, ArpOperation, ArpPacket, EthernetFrame, FlowKey, HttpMethod, IcmpMessage,
    Ipv4Cidr, Ipv4Packet, MacAddress, NetworkPayload, Route, TcpFlags, TcpSegment, Transport,
    TransportProtocol, UdpDatagram, VlanId,
};
pub use process::{ProcessCommand, ProcessEvent, ProcessSignal, SignalValue};
pub use storage::{CapacityError, Text};

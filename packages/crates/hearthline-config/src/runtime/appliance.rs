use hearthline_engine::{
    Actuator, DnsServer, EffectList, FieldSensor, FirewallHaStatus, FirewallSessionSnapshot,
    Layer3Switch, LearningSwitch, LinkAppliance, MacTableEntry, NatRouter, NeighborEntry,
    OperatorInterface, PatTranslation, RemoteIo, ReverseProxyWaf, Router, SafetyInterface,
    ServiceNode, SimulatedComponent, SimulationEvent, StatefulFirewall, VirtualPlc,
};
use hearthline_model::{ComponentId, ComponentKind, PortId, TransportProtocol, VlanId};
use serde::Serialize;

#[derive(Clone, Debug)]
pub enum ConfiguredAppliance {
    Endpoint(Box<ServiceNode>),
    Dns(Box<DnsServer>),
    Switch(Box<LearningSwitch>),
    Layer3Switch(Box<Layer3Switch>),
    Router(Box<Router>),
    NatRouter(Box<NatRouter>),
    Firewall(Box<StatefulFirewall>),
    WebGateway(Box<ReverseProxyWaf>),
    Link(Box<LinkAppliance>),
    Hmi(Box<OperatorInterface>),
    VirtualPlc(Box<VirtualPlc>),
    RemoteIo(Box<RemoteIo>),
    FieldSensor(Box<FieldSensor>),
    FieldActuator(Box<Actuator>),
    Safety(Box<SafetyInterface>),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDeviceSnapshot {
    pub id: String,
    pub kind: String,
    pub supports_mac_table: bool,
    pub supports_neighbors: bool,
    pub supports_pat: bool,
    pub supports_firewall_sessions: bool,
    pub mac_table: Vec<RuntimeMacEntry>,
    pub neighbors: Vec<RuntimeNeighborEntry>,
    pub pat_translations: Vec<RuntimePatEntry>,
    pub firewall_sessions: Vec<RuntimeFirewallSessionEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMacEntry {
    pub vlan: u16,
    pub mac_address: String,
    pub interface: String,
    pub remaining_ttl_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeNeighborEntry {
    pub address: String,
    pub mac_address: String,
    pub interface: String,
    pub state: &'static str,
    pub remaining_ttl_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePatEntry {
    pub protocol: String,
    pub internal_address: String,
    pub internal_token: u16,
    pub external_address: String,
    pub external_token: u16,
    pub remote_address: String,
    pub remote_port: Option<u16>,
    pub remaining_ttl_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFirewallSessionEntry {
    pub protocol: String,
    pub source_address: String,
    pub source_port: Option<u16>,
    pub destination_address: String,
    pub destination_port: Option<u16>,
    pub remaining_ttl_ms: u64,
}

impl SimulatedComponent for ConfiguredAppliance {
    fn id(&self) -> &ComponentId {
        match self {
            Self::Endpoint(appliance) => appliance.id(),
            Self::Dns(appliance) => appliance.id(),
            Self::Switch(appliance) => appliance.id(),
            Self::Layer3Switch(appliance) => appliance.id(),
            Self::Router(appliance) => appliance.id(),
            Self::NatRouter(appliance) => appliance.id(),
            Self::Firewall(appliance) => appliance.id(),
            Self::WebGateway(appliance) => appliance.id(),
            Self::Link(appliance) => appliance.id(),
            Self::Hmi(appliance) => appliance.id(),
            Self::VirtualPlc(appliance) => appliance.id(),
            Self::RemoteIo(appliance) => appliance.id(),
            Self::FieldSensor(appliance) => appliance.id(),
            Self::FieldActuator(appliance) => appliance.id(),
            Self::Safety(appliance) => appliance.id(),
        }
    }

    fn kind(&self) -> ComponentKind {
        match self {
            Self::Endpoint(appliance) => appliance.kind(),
            Self::Dns(appliance) => appliance.kind(),
            Self::Switch(appliance) => appliance.kind(),
            Self::Layer3Switch(appliance) => appliance.kind(),
            Self::Router(appliance) => appliance.kind(),
            Self::NatRouter(appliance) => appliance.kind(),
            Self::Firewall(appliance) => appliance.kind(),
            Self::WebGateway(appliance) => appliance.kind(),
            Self::Link(appliance) => appliance.kind(),
            Self::Hmi(appliance) => appliance.kind(),
            Self::VirtualPlc(appliance) => appliance.kind(),
            Self::RemoteIo(appliance) => appliance.kind(),
            Self::FieldSensor(appliance) => appliance.kind(),
            Self::FieldActuator(appliance) => appliance.kind(),
            Self::Safety(appliance) => appliance.kind(),
        }
    }

    fn has_port(&self, port: &PortId) -> bool {
        match self {
            Self::Endpoint(appliance) => appliance.has_port(port),
            Self::Dns(appliance) => appliance.has_port(port),
            Self::Switch(appliance) => appliance.has_port(port),
            Self::Layer3Switch(appliance) => appliance.has_port(port),
            Self::Router(appliance) => appliance.has_port(port),
            Self::NatRouter(appliance) => appliance.has_port(port),
            Self::Firewall(appliance) => appliance.has_port(port),
            Self::WebGateway(appliance) => appliance.has_port(port),
            Self::Link(appliance) => appliance.has_port(port),
            Self::Hmi(appliance) => appliance.has_port(port),
            Self::VirtualPlc(appliance) => appliance.has_port(port),
            Self::RemoteIo(appliance) => appliance.has_port(port),
            Self::FieldSensor(appliance) => appliance.has_port(port),
            Self::FieldActuator(appliance) => appliance.has_port(port),
            Self::Safety(appliance) => appliance.has_port(port),
        }
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match self {
            Self::Endpoint(appliance) => appliance.handle(event),
            Self::Dns(appliance) => appliance.handle(event),
            Self::Switch(appliance) => appliance.handle(event),
            Self::Layer3Switch(appliance) => appliance.handle(event),
            Self::Router(appliance) => appliance.handle(event),
            Self::NatRouter(appliance) => appliance.handle(event),
            Self::Firewall(appliance) => appliance.handle(event),
            Self::WebGateway(appliance) => appliance.handle(event),
            Self::Link(appliance) => appliance.handle(event),
            Self::Hmi(appliance) => appliance.handle(event),
            Self::VirtualPlc(appliance) => appliance.handle(event),
            Self::RemoteIo(appliance) => appliance.handle(event),
            Self::FieldSensor(appliance) => appliance.handle(event),
            Self::FieldActuator(appliance) => appliance.handle(event),
            Self::Safety(appliance) => appliance.handle(event),
        }
    }
}

impl ConfiguredAppliance {
    pub(super) fn endpoint_neighbors(&self, now_us: u64) -> Option<Vec<NeighborEntry>> {
        let Self::Endpoint(appliance) = self else {
            return None;
        };
        Some(appliance.neighbors(now_us).cloned().collect())
    }

    pub(super) fn active_pat_translation_count(&self, now_us: u64) -> usize {
        let Self::NatRouter(appliance) = self else {
            return 0;
        };
        appliance.active_translation_count(now_us)
    }

    pub(super) fn runtime_snapshot(&self, now_us: u64) -> Option<RuntimeDeviceSnapshot> {
        let mut snapshot = match self {
            Self::Endpoint(appliance) => {
                device_snapshot(appliance.as_ref(), false, true, false, false)
            }
            Self::Switch(appliance) => {
                device_snapshot(appliance.as_ref(), true, false, false, false)
            }
            Self::Layer3Switch(appliance) => {
                device_snapshot(appliance.as_ref(), true, true, false, false)
            }
            Self::Router(appliance) => {
                device_snapshot(appliance.as_ref(), false, true, false, false)
            }
            Self::NatRouter(appliance) => {
                device_snapshot(appliance.as_ref(), false, true, true, false)
            }
            Self::Firewall(appliance) => {
                device_snapshot(appliance.as_ref(), false, true, false, true)
            }
            Self::Dns(_)
            | Self::WebGateway(_)
            | Self::Link(_)
            | Self::Hmi(_)
            | Self::VirtualPlc(_)
            | Self::RemoteIo(_)
            | Self::FieldSensor(_)
            | Self::FieldActuator(_)
            | Self::Safety(_) => return None,
        };
        match self {
            Self::Endpoint(appliance) => {
                snapshot.neighbors = neighbor_entries(appliance.neighbors(now_us), now_us);
            }
            Self::Switch(appliance) => {
                snapshot.mac_table = mac_entries(appliance.active_mac_table(now_us));
            }
            Self::Layer3Switch(appliance) => {
                snapshot.mac_table = mac_entries(appliance.active_mac_table(now_us));
                snapshot.neighbors = neighbor_entries(appliance.neighbors(now_us), now_us);
            }
            Self::Router(appliance) => {
                snapshot.neighbors = neighbor_entries(appliance.neighbors(now_us), now_us);
            }
            Self::NatRouter(appliance) => {
                snapshot.neighbors = neighbor_entries(appliance.neighbors(now_us), now_us);
                snapshot.pat_translations =
                    pat_entries(appliance.active_translations(now_us), now_us);
            }
            Self::Firewall(appliance) => {
                snapshot.neighbors = neighbor_entries(appliance.neighbors(now_us), now_us);
                snapshot.firewall_sessions =
                    firewall_entries(appliance.active_sessions(now_us), now_us);
            }
            Self::Dns(_)
            | Self::WebGateway(_)
            | Self::Link(_)
            | Self::Hmi(_)
            | Self::VirtualPlc(_)
            | Self::RemoteIo(_)
            | Self::FieldSensor(_)
            | Self::FieldActuator(_)
            | Self::Safety(_) => unreachable!("non-runtime appliance returned above"),
        }
        Some(snapshot)
    }

    pub(super) fn disable_port(&mut self, port: &PortId) {
        match self {
            Self::Switch(appliance) => {
                let _ = appliance.set_port_forwarding(port, false);
            }
            Self::Layer3Switch(appliance) => {
                let _ = appliance.set_port_forwarding(port, false);
            }
            Self::Firewall(appliance) => {
                let _ = appliance.set_ha_sync_attached(port, false);
            }
            Self::Link(appliance) => {
                let _ = appliance.set_port_forwarding(port, false);
            }
            Self::Endpoint(_)
            | Self::Dns(_)
            | Self::Router(_)
            | Self::NatRouter(_)
            | Self::WebGateway(_)
            | Self::Hmi(_)
            | Self::VirtualPlc(_)
            | Self::RemoteIo(_)
            | Self::FieldSensor(_)
            | Self::FieldActuator(_)
            | Self::Safety(_) => {}
        }
    }

    pub(super) fn set_first_hop_active(
        &mut self,
        port: &PortId,
        address: Ipv4Addr,
        active: bool,
    ) -> bool {
        match self {
            Self::Layer3Switch(appliance) => appliance.set_first_hop_active(port, address, active),
            Self::Router(appliance) => appliance.set_first_hop_active(port, address, active),
            Self::Firewall(appliance) => appliance.set_first_hop_active(port, address, active),
            Self::Endpoint(_)
            | Self::Dns(_)
            | Self::Switch(_)
            | Self::NatRouter(_)
            | Self::WebGateway(_)
            | Self::Link(_)
            | Self::Hmi(_)
            | Self::VirtualPlc(_)
            | Self::RemoteIo(_)
            | Self::FieldSensor(_)
            | Self::FieldActuator(_)
            | Self::Safety(_) => false,
        }
    }

    pub(super) fn set_firewall_ha_active(&mut self, active: bool) -> bool {
        let Self::Firewall(appliance) = self else {
            return false;
        };
        appliance.set_ha_active(active);
        true
    }

    pub(super) fn firewall_ha_status(&self) -> Option<FirewallHaStatus> {
        let Self::Firewall(appliance) = self else {
            return None;
        };
        appliance.ha_status()
    }

    pub(super) fn set_spanning_tree_forwarding(
        &mut self,
        port: &PortId,
        vlan: VlanId,
        forwarding: bool,
    ) -> bool {
        match self {
            Self::Switch(appliance) => {
                appliance.set_spanning_tree_forwarding(port, vlan, forwarding)
            }
            Self::Layer3Switch(appliance) => {
                appliance.set_spanning_tree_forwarding(port, vlan, forwarding)
            }
            Self::Endpoint(_)
            | Self::Dns(_)
            | Self::Router(_)
            | Self::NatRouter(_)
            | Self::Firewall(_)
            | Self::WebGateway(_)
            | Self::Link(_)
            | Self::Hmi(_)
            | Self::VirtualPlc(_)
            | Self::RemoteIo(_)
            | Self::FieldSensor(_)
            | Self::FieldActuator(_)
            | Self::Safety(_) => false,
        }
    }

    pub(super) fn set_link_aggregation_forwarding(
        &mut self,
        port: &PortId,
        forwarding: bool,
    ) -> bool {
        match self {
            Self::Switch(appliance) => appliance.set_link_aggregation_forwarding(port, forwarding),
            Self::Layer3Switch(appliance) => {
                appliance.set_link_aggregation_forwarding(port, forwarding)
            }
            Self::Endpoint(_)
            | Self::Dns(_)
            | Self::Router(_)
            | Self::NatRouter(_)
            | Self::Firewall(_)
            | Self::WebGateway(_)
            | Self::Link(_)
            | Self::Hmi(_)
            | Self::VirtualPlc(_)
            | Self::RemoteIo(_)
            | Self::FieldSensor(_)
            | Self::FieldActuator(_)
            | Self::Safety(_) => false,
        }
    }

    pub(super) fn set_multi_chassis_peer_forwarding(
        &mut self,
        logical_id: &ComponentId,
        forwarding: bool,
    ) -> bool {
        match self {
            Self::Switch(appliance) => {
                appliance.set_multi_chassis_peer_forwarding(logical_id, forwarding)
            }
            Self::Layer3Switch(appliance) => {
                appliance.set_multi_chassis_peer_forwarding(logical_id, forwarding)
            }
            Self::Endpoint(_)
            | Self::Dns(_)
            | Self::Router(_)
            | Self::NatRouter(_)
            | Self::Firewall(_)
            | Self::WebGateway(_)
            | Self::Link(_)
            | Self::Hmi(_)
            | Self::VirtualPlc(_)
            | Self::RemoteIo(_)
            | Self::FieldSensor(_)
            | Self::FieldActuator(_)
            | Self::Safety(_) => false,
        }
    }
}

fn device_snapshot<T: SimulatedComponent>(
    appliance: &T,
    supports_mac_table: bool,
    supports_neighbors: bool,
    supports_pat: bool,
    supports_firewall_sessions: bool,
) -> RuntimeDeviceSnapshot {
    RuntimeDeviceSnapshot {
        id: appliance.id().to_string(),
        kind: appliance.kind().to_string(),
        supports_mac_table,
        supports_neighbors,
        supports_pat,
        supports_firewall_sessions,
        mac_table: Vec::new(),
        neighbors: Vec::new(),
        pat_translations: Vec::new(),
        firewall_sessions: Vec::new(),
    }
}

fn mac_entries<'a>(
    entries: impl Iterator<Item = (&'a MacTableEntry, u64)>,
) -> Vec<RuntimeMacEntry> {
    entries
        .map(|(entry, remaining_us)| RuntimeMacEntry {
            vlan: entry.vlan.get(),
            mac_address: entry.address.to_string(),
            interface: entry.port.to_string(),
            remaining_ttl_ms: remaining_us.div_ceil(1_000),
        })
        .collect()
}

fn neighbor_entries<'a>(
    entries: impl Iterator<Item = &'a NeighborEntry>,
    now_us: u64,
) -> Vec<RuntimeNeighborEntry> {
    entries
        .map(|entry| RuntimeNeighborEntry {
            address: entry.address.to_string(),
            mac_address: entry.mac.to_string(),
            interface: entry.port.to_string(),
            state: "reachable",
            remaining_ttl_ms: entry.expires_at_us.saturating_sub(now_us).div_ceil(1_000),
        })
        .collect()
}

fn pat_entries(entries: impl Iterator<Item = PatTranslation>, now_us: u64) -> Vec<RuntimePatEntry> {
    entries
        .map(|entry| RuntimePatEntry {
            protocol: protocol_name(entry.protocol),
            internal_address: entry.internal_address.to_string(),
            internal_token: entry.internal_token,
            external_address: entry.external_address.to_string(),
            external_token: entry.external_token,
            remote_address: entry.remote_address.to_string(),
            remote_port: entry.remote_port,
            remaining_ttl_ms: entry.expires_at_us.saturating_sub(now_us).div_ceil(1_000),
        })
        .collect()
}

fn firewall_entries(
    entries: impl Iterator<Item = FirewallSessionSnapshot>,
    now_us: u64,
) -> Vec<RuntimeFirewallSessionEntry> {
    entries
        .map(|entry| RuntimeFirewallSessionEntry {
            protocol: protocol_name(entry.flow.protocol),
            source_address: entry.flow.source.to_string(),
            source_port: entry.flow.source_port,
            destination_address: entry.flow.destination.to_string(),
            destination_port: entry.flow.destination_port,
            remaining_ttl_ms: entry.expires_at_us.saturating_sub(now_us).div_ceil(1_000),
        })
        .collect()
}

fn protocol_name(protocol: TransportProtocol) -> String {
    match protocol {
        TransportProtocol::Icmp => "icmp".into(),
        TransportProtocol::Tcp => "tcp".into(),
        TransportProtocol::Udp => "udp".into(),
        TransportProtocol::Other(number) => format!("ip-{number}"),
    }
}
use std::net::Ipv4Addr;

use hearthline_engine::{
    FirewallSessionSnapshot, MacTableEntry, NeighborEntry, PatTranslation, SimulatedComponent,
};
use hearthline_model::TransportProtocol;
use serde::Serialize;

pub const RUNTIME_COMPONENT_SNAPSHOT_SCHEMA_VERSION: &str = "1.0.0";

use super::ConfiguredAppliance;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDeviceSnapshot {
    pub schema_version: String,
    pub id: String,
    pub kind: String,
    pub authoritative_state: String,
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

impl ConfiguredAppliance {
    pub(in crate::runtime) fn runtime_snapshot(&self, now_us: u64) -> RuntimeDeviceSnapshot {
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
            Self::Dns(appliance) => device_snapshot(appliance.as_ref(), false, true, false, false),
            Self::WebGateway(appliance) => {
                device_snapshot(appliance.as_ref(), false, true, false, false)
            }
            Self::Link(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
            Self::Wireless(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
            Self::Monitor(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
            Self::Unaddressed(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
            Self::Hmi(appliance) => device_snapshot(appliance.as_ref(), false, false, false, false),
            Self::VirtualPlc(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
            Self::RemoteIo(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
            Self::FieldSensor(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
            Self::FieldActuator(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
            Self::Safety(appliance) => {
                device_snapshot(appliance.as_ref(), false, false, false, false)
            }
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
            | Self::Wireless(_)
            | Self::Monitor(_)
            | Self::Unaddressed(_)
            | Self::Hmi(_)
            | Self::VirtualPlc(_)
            | Self::RemoteIo(_)
            | Self::FieldSensor(_)
            | Self::FieldActuator(_)
            | Self::Safety(_) => {}
        }
        snapshot
    }
}

fn device_snapshot<T: SimulatedComponent + core::fmt::Debug>(
    appliance: &T,
    supports_mac_table: bool,
    supports_neighbors: bool,
    supports_pat: bool,
    supports_firewall_sessions: bool,
) -> RuntimeDeviceSnapshot {
    RuntimeDeviceSnapshot {
        schema_version: RUNTIME_COMPONENT_SNAPSHOT_SCHEMA_VERSION.into(),
        id: appliance.id().to_string(),
        kind: appliance.kind().to_string(),
        authoritative_state: format!("{appliance:?}"),
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

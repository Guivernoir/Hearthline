use std::collections::BTreeSet;
use std::net::Ipv4Addr;

use hearthline_engine::{
    FirewallHaStatus, MediaLink, NeighborEntry, SimulatedComponent, SimulationError,
    SimulationEvent, Simulator, TraceEntry,
};
use hearthline_model::{ComponentId, EthernetFrame, Ipv4Packet, PortId, VlanId};

use crate::appliance::{ConfigError, ConfigRepository, InterfaceMode};
use crate::connection::ConnectionRepository;

use super::{ConfiguredAppliance, RuntimeDeviceSnapshot, build_appliance};

#[derive(Clone, Debug)]
pub struct ConfiguredNetwork {
    appliances: Vec<ConfiguredAppliance>,
    links: Vec<MediaLink>,
}

impl ConfiguredNetwork {
    pub fn from_selection<I, S>(
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
        appliance_ids: I,
    ) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let selected = appliance_ids
            .into_iter()
            .map(|id| id.as_ref().to_owned())
            .collect::<BTreeSet<_>>();
        if selected.is_empty() {
            return Err(ConfigError::new(
                "configured network selection cannot be empty",
            ));
        }
        let selected_connections = connections
            .connections()
            .filter(|connection| {
                selected.contains(&connection.config.endpoints.a.appliance)
                    && selected.contains(&connection.config.endpoints.b.appliance)
            })
            .collect::<Vec<_>>();
        let active_ports = selected_connections
            .iter()
            .flat_map(|connection| {
                [
                    (
                        connection.config.endpoints.a.appliance.as_str(),
                        connection.config.endpoints.a.interface.as_str(),
                    ),
                    (
                        connection.config.endpoints.b.appliance.as_str(),
                        connection.config.endpoints.b.interface.as_str(),
                    ),
                ]
            })
            .collect::<BTreeSet<_>>();
        let mut runtime_appliances = Vec::with_capacity(selected.len());
        for id in &selected {
            let loaded = appliances
                .get(id)
                .ok_or_else(|| ConfigError::new(format!("unknown selected appliance {id}")))?;
            let mut appliance = build_appliance(loaded, appliances)?;
            for interface in &loaded.config.interfaces {
                if interface.mode != InterfaceMode::Svi
                    && !active_ports.contains(&(id.as_str(), interface.id.as_str()))
                {
                    appliance.disable_port(
                        &PortId::new(&interface.id)
                            .map_err(|error| ConfigError::new(error.to_string()))?,
                    );
                }
            }
            runtime_appliances.push(appliance);
        }
        let links = selected_connections
            .into_iter()
            .map(|connection| connection.config.media_link(appliances))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            appliances: runtime_appliances,
            links,
        })
    }

    pub fn appliance_count(&self) -> usize {
        self.appliances.len()
    }

    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    pub fn endpoint_neighbors(
        &self,
        appliance_id: &str,
        now_us: u64,
    ) -> Result<Vec<NeighborEntry>, ConfigError> {
        let appliance = self
            .appliances
            .iter()
            .find(|appliance| appliance.id().as_str() == appliance_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "selected network does not contain appliance {appliance_id}"
                ))
            })?;
        appliance.endpoint_neighbors(now_us).ok_or_else(|| {
            ConfigError::new(format!(
                "appliance {appliance_id} does not expose an endpoint neighbor cache"
            ))
        })
    }

    pub fn active_pat_translation_count(&self, now_us: u64) -> usize {
        self.appliances
            .iter()
            .map(|appliance| appliance.active_pat_translation_count(now_us))
            .sum()
    }

    pub fn runtime_snapshot(&self, now_us: u64) -> Vec<RuntimeDeviceSnapshot> {
        self.appliances
            .iter()
            .filter_map(|appliance| appliance.runtime_snapshot(now_us))
            .collect()
    }

    pub fn set_connection_operational(
        &mut self,
        connection_id: &str,
        operational: bool,
    ) -> Result<(), ConfigError> {
        let link = self
            .links
            .iter_mut()
            .find(|link| link.id().as_str() == connection_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "selected network does not contain connection {connection_id}"
                ))
            })?;
        link.set_operational(operational);
        Ok(())
    }

    pub fn set_first_hop_active(
        &mut self,
        appliance_id: &str,
        interface_id: &str,
        virtual_ip: Ipv4Addr,
        active: bool,
    ) -> Result<(), ConfigError> {
        let appliance = self
            .appliances
            .iter_mut()
            .find(|appliance| appliance.id().as_str() == appliance_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "selected network does not contain appliance {appliance_id}"
                ))
            })?;
        let port =
            PortId::new(interface_id).map_err(|error| ConfigError::new(error.to_string()))?;
        if !appliance.set_first_hop_active(&port, virtual_ip, active) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} does not contain first-hop address {virtual_ip}"
            )));
        }
        Ok(())
    }

    pub fn set_firewall_ha_active(
        &mut self,
        appliance_id: &str,
        active: bool,
    ) -> Result<(), ConfigError> {
        let appliance = self
            .appliances
            .iter_mut()
            .find(|appliance| appliance.id().as_str() == appliance_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "selected network does not contain appliance {appliance_id}"
                ))
            })?;
        if !appliance.set_firewall_ha_active(active) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} is not a stateful firewall"
            )));
        }
        Ok(())
    }

    pub fn firewall_ha_status(&self, appliance_id: &str) -> Result<FirewallHaStatus, ConfigError> {
        let appliance = self
            .appliances
            .iter()
            .find(|appliance| appliance.id().as_str() == appliance_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "selected network does not contain appliance {appliance_id}"
                ))
            })?;
        appliance.firewall_ha_status().ok_or_else(|| {
            ConfigError::new(format!(
                "appliance {appliance_id} is not an HA stateful firewall"
            ))
        })
    }

    pub fn set_spanning_tree_forwarding(
        &mut self,
        appliance_id: &str,
        interface_id: &str,
        vlan: u16,
        forwarding: bool,
    ) -> Result<(), ConfigError> {
        let appliance = self
            .appliances
            .iter_mut()
            .find(|appliance| appliance.id().as_str() == appliance_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "selected network does not contain appliance {appliance_id}"
                ))
            })?;
        let port =
            PortId::new(interface_id).map_err(|error| ConfigError::new(error.to_string()))?;
        let vlan = VlanId::new(vlan)
            .ok_or_else(|| ConfigError::new(format!("invalid spanning-tree VLAN {vlan}")))?;
        if !appliance.set_spanning_tree_forwarding(&port, vlan, forwarding) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} does not bridge VLAN {vlan:?}"
            )));
        }
        Ok(())
    }

    pub fn set_link_aggregation_forwarding(
        &mut self,
        appliance_id: &str,
        interface_id: &str,
        forwarding: bool,
    ) -> Result<(), ConfigError> {
        let appliance = self
            .appliances
            .iter_mut()
            .find(|appliance| appliance.id().as_str() == appliance_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "selected network does not contain appliance {appliance_id}"
                ))
            })?;
        let port =
            PortId::new(interface_id).map_err(|error| ConfigError::new(error.to_string()))?;
        if !appliance.set_link_aggregation_forwarding(&port, forwarding) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} interface {interface_id} is not an aggregate member"
            )));
        }
        Ok(())
    }

    pub fn set_multi_chassis_peer_forwarding(
        &mut self,
        appliance_id: &str,
        logical_id: &str,
        forwarding: bool,
    ) -> Result<(), ConfigError> {
        let appliance = self
            .appliances
            .iter_mut()
            .find(|appliance| appliance.id().as_str() == appliance_id)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "selected network does not contain appliance {appliance_id}"
                ))
            })?;
        let logical_id =
            ComponentId::new(logical_id).map_err(|error| ConfigError::new(error.to_string()))?;
        if !appliance.set_multi_chassis_peer_forwarding(&logical_id, forwarding) {
            return Err(ConfigError::new(format!(
                "appliance {appliance_id} does not contain multi-chassis aggregate {logical_id}"
            )));
        }
        Ok(())
    }

    pub fn run_ipv4(
        &mut self,
        source: &ComponentId,
        packet: Ipv4Packet,
        event_limit: usize,
    ) -> Result<Vec<TraceEntry>, SimulationError> {
        self.run_ipv4_with_wire_length(
            source,
            packet,
            EthernetFrame::MIN_WIRE_LEN_BYTES,
            event_limit,
        )
    }

    pub fn run_ipv4_with_wire_length(
        &mut self,
        source: &ComponentId,
        packet: Ipv4Packet,
        wire_length_bytes: u16,
        event_limit: usize,
    ) -> Result<Vec<TraceEntry>, SimulationError> {
        self.run_ipv4_at(source, packet, wire_length_bytes, 0, event_limit)
    }

    pub fn run_ipv4_at(
        &mut self,
        source: &ComponentId,
        packet: Ipv4Packet,
        wire_length_bytes: u16,
        at_us: u64,
        event_limit: usize,
    ) -> Result<Vec<TraceEntry>, SimulationError> {
        self.run_event_at(
            source,
            SimulationEvent::Ipv4Egress(hearthline_engine::Ipv4Egress {
                packet,
                wire_len_bytes: wire_length_bytes,
                sent_at_us: at_us,
            }),
            at_us,
            event_limit,
        )
    }

    pub fn run_event_at(
        &mut self,
        target: &ComponentId,
        event: SimulationEvent,
        at_us: u64,
        event_limit: usize,
    ) -> Result<Vec<TraceEntry>, SimulationError> {
        let mut simulator = Simulator::with_start_time_us(at_us);
        for appliance in &mut self.appliances {
            simulator.add(appliance)?;
        }
        for link in &mut self.links {
            simulator.add_link(link)?;
        }
        simulator.inject(target, event)?;
        Ok(simulator.run(event_limit)?.to_vec())
    }
}

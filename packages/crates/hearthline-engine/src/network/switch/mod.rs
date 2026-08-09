mod aggregation;
mod layer3;
mod state;

pub use aggregation::SwitchAggregationGroup;
pub use layer3::Layer3Switch;

use heapless::Vec as FixedList;

use hearthline_model::{ComponentId, ComponentKind, MacAddress, PortId, VlanId};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, SimulatedComponent, SimulationEvent};

const FORWARDING_CAPACITY: usize = 64;
const DEFAULT_AGING_TIME_US: u64 = 300_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwitchPort {
    pub id: PortId,
    pub allowed_vlans: FixedList<VlanId, 32>,
    pub forwarding: bool,
    blocked_vlans: FixedList<VlanId, 32>,
}

impl SwitchPort {
    pub fn access(id: PortId, vlan: VlanId) -> Self {
        Self {
            id,
            allowed_vlans: collect_fixed([vlan]),
            forwarding: true,
            blocked_vlans: FixedList::new(),
        }
    }

    pub fn trunk(id: PortId, allowed_vlans: impl IntoIterator<Item = VlanId>) -> Self {
        Self {
            id,
            allowed_vlans: collect_fixed(allowed_vlans),
            forwarding: true,
            blocked_vlans: FixedList::new(),
        }
    }

    fn forwards_vlan(&self, vlan: VlanId) -> bool {
        self.forwarding && self.allowed_vlans.contains(&vlan) && !self.blocked_vlans.contains(&vlan)
    }
}

#[derive(Clone, Debug)]
pub struct LearningSwitch {
    id: ComponentId,
    kind: ComponentKind,
    ports: FixedList<SwitchPort, 16>,
    forwarding_table: FixedList<MacTableEntry, FORWARDING_CAPACITY>,
    aggregations: FixedList<SwitchAggregationGroup, 16>,
    multi_chassis_peer_link: Option<PortId>,
    aging_time_us: u64,
    operational: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacTableEntry {
    pub vlan: VlanId,
    pub address: MacAddress,
    pub port: PortId,
    pub last_seen_us: u64,
}

impl LearningSwitch {
    pub fn new(id: ComponentId, ports: impl IntoIterator<Item = SwitchPort>) -> Self {
        Self::with_kind(id, ComponentKind::Layer2Switch, ports)
    }

    fn with_kind(
        id: ComponentId,
        kind: ComponentKind,
        ports: impl IntoIterator<Item = SwitchPort>,
    ) -> Self {
        Self {
            id,
            kind,
            ports: collect_fixed(ports),
            forwarding_table: FixedList::new(),
            aggregations: FixedList::new(),
            multi_chassis_peer_link: None,
            aging_time_us: DEFAULT_AGING_TIME_US,
            operational: true,
        }
    }

    pub fn learned_port(&self, vlan: VlanId, address: MacAddress) -> Option<&PortId> {
        self.forwarding_table
            .iter()
            .find(|entry| entry.vlan == vlan && entry.address == address)
            .map(|entry| &entry.port)
    }

    pub fn mac_table(&self) -> &[MacTableEntry] {
        self.forwarding_table.as_slice()
    }

    pub fn set_aging_time_us(&mut self, aging_time_us: u64) {
        assert!(aging_time_us > 0, "CAM aging time must be non-zero");
        self.aging_time_us = aging_time_us;
    }

    pub fn set_port_forwarding(&mut self, port: &PortId, forwarding: bool) -> bool {
        let Some(port) = self
            .ports
            .iter_mut()
            .find(|candidate| candidate.id == *port)
        else {
            return false;
        };
        port.forwarding = forwarding;
        if !forwarding {
            self.forwarding_table.retain(|entry| entry.port != port.id);
        }
        true
    }

    pub fn add_link_aggregation_group(&mut self, group: SwitchAggregationGroup) -> bool {
        if self
            .aggregations
            .iter()
            .any(|candidate| candidate.id == group.id || candidate.logical_id == group.logical_id)
            || !aggregation::validates_group_members(&group, &self.ports)
            || group
                .members
                .iter()
                .any(|member| self.aggregation_for_port(&member.port).is_some())
        {
            return false;
        }
        self.aggregations.push(group).is_ok()
    }

    pub fn set_multi_chassis_peer_link(&mut self, port: PortId) -> bool {
        if !self.has_port(&port) || self.aggregations.iter().any(|group| group.contains(&port)) {
            return false;
        }
        self.multi_chassis_peer_link = Some(port);
        true
    }

    pub fn set_link_aggregation_forwarding(&mut self, port: &PortId, forwarding: bool) -> bool {
        let Some(group) = self
            .aggregations
            .iter_mut()
            .find(|group| group.contains(port))
        else {
            return false;
        };
        if !group.set_member_forwarding(port, forwarding) {
            return false;
        }
        true
    }

    pub fn set_multi_chassis_peer_forwarding(
        &mut self,
        logical_id: &ComponentId,
        forwarding: bool,
    ) -> bool {
        let Some(group) = self
            .aggregations
            .iter_mut()
            .find(|group| group.logical_id == *logical_id && group.multi_chassis)
        else {
            return false;
        };
        group.peer_forwarding = forwarding;
        true
    }

    pub fn set_spanning_tree_forwarding(
        &mut self,
        port: &PortId,
        vlan: VlanId,
        forwarding: bool,
    ) -> bool {
        let Some(port) = self
            .ports
            .iter_mut()
            .find(|candidate| candidate.id == *port && candidate.allowed_vlans.contains(&vlan))
        else {
            return false;
        };
        if forwarding {
            if let Some(index) = port
                .blocked_vlans
                .iter()
                .position(|candidate| *candidate == vlan)
            {
                port.blocked_vlans.swap_remove(index);
            }
        } else if !port.blocked_vlans.contains(&vlan) {
            port.blocked_vlans
                .push(vlan)
                .expect("blocked VLAN capacity matches allowed VLAN capacity");
        }
        self.forwarding_table
            .retain(|entry| entry.port != port.id || entry.vlan != vlan);
        true
    }

    fn expire_entries(&mut self, now_us: u64) {
        let mut index = 0;
        while index < self.forwarding_table.len() {
            if now_us.saturating_sub(self.forwarding_table[index].last_seen_us)
                >= self.aging_time_us
            {
                self.forwarding_table.swap_remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn learn(&mut self, vlan: VlanId, address: MacAddress, port: PortId, now_us: u64) {
        if let Some(entry) = self
            .forwarding_table
            .iter_mut()
            .find(|entry| entry.vlan == vlan && entry.address == address)
        {
            entry.port = port;
            entry.last_seen_us = now_us;
            return;
        }
        if self.forwarding_table.is_full() {
            let oldest = self
                .forwarding_table
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.last_seen_us)
                .map(|(index, _)| index)
                .expect("full CAM table has an entry");
            self.forwarding_table.swap_remove(oldest);
        }
        self.forwarding_table
            .push(MacTableEntry {
                vlan,
                address,
                port,
                last_seen_us: now_us,
            })
            .expect("CAM table has capacity after eviction");
    }

    fn aggregation_for_port(&self, port: &PortId) -> Option<&SwitchAggregationGroup> {
        self.aggregations.iter().find(|group| group.contains(port))
    }

    fn port_forwards_vlan(&self, port: &SwitchPort, vlan: VlanId) -> bool {
        aggregation::port_forwards_vlan(port, vlan, &self.aggregations)
    }

    fn selected_egress<'a>(
        &'a self,
        learned_port: &PortId,
        frame: &hearthline_model::EthernetFrame,
    ) -> Option<&'a PortId> {
        if let Some(group) = self.aggregation_for_port(learned_port) {
            group.selected_member(frame, &self.ports)
        } else {
            self.ports
                .iter()
                .find(|port| port.id == *learned_port && self.port_forwards_vlan(port, frame.vlan))
                .map(|port| &port.id)
        }
    }

    fn flood_egress_allowed(
        &self,
        ingress: &PortId,
        candidate: &SwitchPort,
        frame: &hearthline_model::EthernetFrame,
    ) -> bool {
        if candidate.id == *ingress || !self.port_forwards_vlan(candidate, frame.vlan) {
            return false;
        }
        let ingress_group = self.aggregation_for_port(ingress);
        let candidate_group = self.aggregation_for_port(&candidate.id);
        if ingress_group.is_some_and(|group| {
            candidate_group.is_some_and(|candidate| candidate.logical_id == group.logical_id)
        }) {
            return false;
        }
        if self.multi_chassis_peer_link.as_ref() == Some(ingress)
            && candidate_group.is_some_and(|group| group.multi_chassis && group.peer_forwarding)
        {
            return false;
        }
        candidate_group
            .is_none_or(|group| group.selected_member(frame, &self.ports) == Some(&candidate.id))
    }
}

#[derive(Clone, Debug)]
pub struct WirelessAccessPoint {
    bridge: LearningSwitch,
    wireless_port: PortId,
    associated_clients: FixedList<MacAddress, 16>,
}

impl WirelessAccessPoint {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = SwitchPort>,
        wireless_port: PortId,
        associated_clients: impl IntoIterator<Item = MacAddress>,
    ) -> Self {
        let bridge = LearningSwitch::with_kind(id, ComponentKind::WirelessAccessPoint, ports);
        assert!(
            bridge.has_port(&wireless_port),
            "wireless port must exist on the access point"
        );
        Self {
            bridge,
            wireless_port,
            associated_clients: collect_fixed(associated_clients),
        }
    }

    pub fn associate(&mut self, client: MacAddress) {
        if !self.associated_clients.contains(&client) {
            self.associated_clients
                .push(client)
                .expect("wireless association table exceeds capacity");
        }
    }

    pub fn disassociate(&mut self, client: MacAddress) {
        if let Some(index) = self
            .associated_clients
            .iter()
            .position(|candidate| *candidate == client)
        {
            self.associated_clients.swap_remove(index);
        }
    }
}

impl SimulatedComponent for WirelessAccessPoint {
    fn id(&self) -> &ComponentId {
        self.bridge.id()
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::WirelessAccessPoint
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.bridge.has_port(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        if let SimulationEvent::Network(ingress) = &event
            && ingress.port == self.wireless_port
            && !self.associated_clients.contains(&ingress.frame.source)
        {
            return single_effect(Effect::Drop(DropReason::PolicyDenied {
                rule: Some("wireless-association".into()),
            }));
        }
        self.bridge.handle(event)
    }
}

impl SimulatedComponent for LearningSwitch {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        self.kind
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.iter().any(|candidate| candidate.id == *port)
    }

    fn handle(&mut self, event: SimulationEvent) -> EffectList {
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.operational {
                    return single_effect(Effect::Drop(DropReason::ComponentDown));
                }
                let Some(port) = self
                    .ports
                    .iter()
                    .find(|candidate| candidate.id == ingress.port)
                else {
                    return single_effect(Effect::Drop(DropReason::InvalidIngress(ingress.port)));
                };
                if !port.forwarding {
                    return single_effect(Effect::Drop(DropReason::PortDown(ingress.port)));
                }
                if !port.allowed_vlans.contains(&ingress.frame.vlan) {
                    return single_effect(Effect::Drop(DropReason::VlanNotAllowed(
                        ingress.frame.vlan.get(),
                    )));
                }
                if !port.forwards_vlan(ingress.frame.vlan) {
                    return single_effect(Effect::Drop(DropReason::SpanningTreeDiscarding {
                        port: ingress.port,
                        vlan: ingress.frame.vlan.get(),
                    }));
                }
                if self
                    .aggregation_for_port(&ingress.port)
                    .is_some_and(|group| !group.member_forwards(&ingress.port))
                {
                    return single_effect(Effect::Drop(DropReason::LinkAggregationDiscarding(
                        ingress.port,
                    )));
                }
                if !ingress.frame.has_valid_wire_length() {
                    return single_effect(Effect::Drop(DropReason::InvalidEthernetFrame));
                }
                if !ingress.frame.source.is_unicast() {
                    return single_effect(Effect::Drop(DropReason::InvalidSourceMac(
                        ingress.frame.source,
                    )));
                }

                self.expire_entries(ingress.received_at_us);
                self.learn(
                    ingress.frame.vlan,
                    ingress.frame.source,
                    ingress.port.clone(),
                    ingress.received_at_us,
                );

                let learned_egress = (!ingress.frame.destination.is_broadcast()
                    && !ingress.frame.destination.is_multicast())
                .then(|| {
                    self.forwarding_table
                        .iter()
                        .find(|entry| {
                            entry.vlan == ingress.frame.vlan
                                && entry.address == ingress.frame.destination
                        })
                        .and_then(|entry| self.selected_egress(&entry.port, &ingress.frame))
                })
                .flatten();

                let egress_ports: FixedList<PortId, 16> = if let Some(egress) = learned_egress {
                    if *egress == ingress.port {
                        return single_effect(Effect::Observe {
                            detail: "filtered destination learned on ingress port".into(),
                        });
                    }
                    collect_fixed([egress.clone()])
                } else {
                    collect_fixed(
                        self.ports
                            .iter()
                            .filter(|candidate| {
                                self.flood_egress_allowed(&ingress.port, candidate, &ingress.frame)
                            })
                            .map(|candidate| candidate.id.clone()),
                    )
                };

                let mut effects = EffectList::new();
                for egress in egress_ports {
                    effects
                        .push(Effect::Transmit {
                            egress,
                            next_hop: None,
                            frame: ingress.frame.clone(),
                            delay_ms: 0,
                        })
                        .expect("switch fan-out exceeds effect capacity");
                }
                effects
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!("operational={operational}")),
                })
            }
            SimulationEvent::Ipv4Egress(_)
            | SimulationEvent::Process(_)
            | SimulationEvent::FirewallHa(_) => {
                single_effect(Effect::Drop(DropReason::UnsupportedProtocol))
            }
        }
    }
}

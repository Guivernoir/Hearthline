use heapless::Vec as FixedList;

use hearthline_model::{ComponentId, EthernetFrame, PortId, VlanId};

use super::SwitchPort;
use crate::runtime::collect_fixed;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwitchAggregationGroup {
    pub id: ComponentId,
    pub logical_id: ComponentId,
    pub(super) members: FixedList<SwitchAggregationMember, 16>,
    pub(super) multi_chassis: bool,
    pub(super) peer_forwarding: bool,
}

impl SwitchAggregationGroup {
    pub fn new(
        id: ComponentId,
        logical_id: ComponentId,
        members: impl IntoIterator<Item = PortId>,
        multi_chassis: bool,
    ) -> Self {
        Self {
            id,
            logical_id,
            members: collect_fixed(members.into_iter().map(|port| SwitchAggregationMember {
                port,
                distributing: true,
            })),
            multi_chassis,
            peer_forwarding: false,
        }
    }

    pub(super) fn contains(&self, port: &PortId) -> bool {
        self.members.iter().any(|member| member.port == *port)
    }

    pub(super) fn member_forwards(&self, port: &PortId) -> bool {
        self.members
            .iter()
            .find(|member| member.port == *port)
            .is_some_and(|member| member.distributing)
    }

    pub(super) fn set_member_forwarding(&mut self, port: &PortId, forwarding: bool) -> bool {
        let Some(member) = self.members.iter_mut().find(|member| member.port == *port) else {
            return false;
        };
        member.distributing = forwarding;
        true
    }

    pub(super) fn selected_member<'a>(
        &self,
        frame: &EthernetFrame,
        ports: &'a [SwitchPort],
    ) -> Option<&'a PortId> {
        let eligible = self
            .members
            .iter()
            .filter(|member| member.distributing)
            .filter_map(|member| {
                ports
                    .iter()
                    .find(|port| port.id == member.port && port.forwards_vlan(frame.vlan))
            })
            .count();
        if eligible == 0 {
            return None;
        }
        let selected = flow_hash(frame) % eligible;
        self.members
            .iter()
            .filter(|member| member.distributing)
            .filter_map(|member| {
                ports
                    .iter()
                    .find(|port| port.id == member.port && port.forwards_vlan(frame.vlan))
            })
            .nth(selected)
            .map(|port| &port.id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SwitchAggregationMember {
    pub(super) port: PortId,
    pub(super) distributing: bool,
}

fn flow_hash(frame: &EthernetFrame) -> usize {
    frame
        .source
        .bytes()
        .into_iter()
        .chain(frame.destination.bytes())
        .fold(usize::from(frame.vlan.get()), |hash, byte| {
            hash.wrapping_mul(16777619) ^ usize::from(byte)
        })
}

pub(super) fn validates_group_members(
    group: &SwitchAggregationGroup,
    ports: &[SwitchPort],
) -> bool {
    !group.members.is_empty()
        && group
            .members
            .iter()
            .all(|member| ports.iter().any(|port| port.id == member.port))
}

pub(super) fn port_forwards_vlan(
    port: &SwitchPort,
    vlan: VlanId,
    groups: &[SwitchAggregationGroup],
) -> bool {
    port.forwards_vlan(vlan)
        && groups
            .iter()
            .find(|group| group.contains(&port.id))
            .is_none_or(|group| group.member_forwards(&port.id))
}

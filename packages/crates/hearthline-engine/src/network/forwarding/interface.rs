use core::net::Ipv4Addr;

use heapless::Vec as FixedList;
use hearthline_model::{Ipv4InterfaceAddress, MacAddress, PortId, VlanId};

use crate::runtime::collect_fixed;

const FIRST_HOP_CAPACITY: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirstHopAddress {
    pub address: Ipv4Addr,
    pub mac: MacAddress,
    pub active: bool,
}

impl FirstHopAddress {
    pub fn new(address: Ipv4Addr, mac: MacAddress, active: bool) -> Self {
        assert!(
            address != Ipv4Addr::UNSPECIFIED
                && address != Ipv4Addr::BROADCAST
                && !address.is_multicast(),
            "first-hop address requires a usable unicast IPv4 address"
        );
        assert!(mac.is_unicast(), "first-hop address requires a unicast MAC");
        Self {
            address,
            mac,
            active,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInterface {
    pub id: PortId,
    pub mac: MacAddress,
    pub addresses: FixedList<Ipv4InterfaceAddress, 4>,
    pub vlan: VlanId,
    pub mtu: u16,
    pub forwarding: bool,
    first_hop_addresses: FixedList<FirstHopAddress, FIRST_HOP_CAPACITY>,
}

impl RoutedInterface {
    pub fn new(
        id: PortId,
        mac: MacAddress,
        addresses: impl IntoIterator<Item = Ipv4InterfaceAddress>,
        vlan: VlanId,
        mtu: u16,
    ) -> Self {
        assert!(mac.is_unicast(), "routed interface requires a unicast MAC");
        assert!(mtu >= 68, "IPv4 interface MTU must be at least 68 bytes");
        Self {
            id,
            mac,
            addresses: collect_fixed(addresses),
            vlan,
            mtu,
            forwarding: true,
            first_hop_addresses: FixedList::new(),
        }
    }

    pub fn add_first_hop_address(&mut self, address: FirstHopAddress) -> Result<(), &'static str> {
        if !self.is_on_link(address.address) {
            return Err("first-hop address is not on-link");
        }
        if self.has_address(address.address) {
            return Err("first-hop address duplicates an interface address");
        }
        if self
            .first_hop_addresses
            .iter()
            .any(|candidate| candidate.address == address.address || candidate.mac == address.mac)
        {
            return Err("first-hop address or MAC is already configured");
        }
        self.first_hop_addresses
            .push(address)
            .map_err(|_| "first-hop address capacity exceeded")
    }

    pub fn first_hop_addresses(&self) -> &[FirstHopAddress] {
        self.first_hop_addresses.as_slice()
    }

    pub fn set_first_hop_active(&mut self, address: Ipv4Addr, active: bool) -> bool {
        let Some(candidate) = self
            .first_hop_addresses
            .iter_mut()
            .find(|candidate| candidate.address == address)
        else {
            return false;
        };
        candidate.active = active;
        true
    }

    pub fn set_all_first_hop_active(&mut self, active: bool) -> bool {
        if self.first_hop_addresses.is_empty() {
            return false;
        }
        for address in &mut self.first_hop_addresses {
            address.active = active;
        }
        true
    }

    pub fn has_address(&self, address: core::net::Ipv4Addr) -> bool {
        self.addresses
            .iter()
            .any(|candidate| candidate.address() == address)
    }

    pub fn owns_active_address(&self, address: Ipv4Addr) -> bool {
        self.has_address(address)
            || self
                .first_hop_addresses
                .iter()
                .any(|candidate| candidate.active && candidate.address == address)
    }

    pub fn accepts_destination_mac(&self, mac: MacAddress) -> bool {
        mac == self.mac
            || self
                .first_hop_addresses
                .iter()
                .any(|candidate| candidate.active && candidate.mac == mac)
    }

    pub fn response_mac(&self, address: Ipv4Addr) -> Option<MacAddress> {
        if self.has_address(address) {
            return Some(self.mac);
        }
        self.first_hop_addresses
            .iter()
            .find(|candidate| candidate.active && candidate.address == address)
            .map(|candidate| candidate.mac)
    }

    pub fn egress_identity(&self) -> Option<(Ipv4Addr, MacAddress)> {
        self.first_hop_addresses
            .iter()
            .find(|candidate| candidate.active)
            .map(|candidate| (candidate.address, candidate.mac))
            .or_else(|| self.primary_address().map(|address| (address, self.mac)))
    }

    pub fn is_on_link(&self, address: core::net::Ipv4Addr) -> bool {
        self.addresses
            .iter()
            .any(|candidate| candidate.is_on_link(address))
    }

    pub fn primary_address(&self) -> Option<core::net::Ipv4Addr> {
        self.addresses.first().map(|address| address.address())
    }

    pub fn accepts_wire_len(&self, wire_len_bytes: u16) -> bool {
        u32::from(wire_len_bytes)
            <= u32::from(self.mtu)
                .saturating_add(hearthline_model::EthernetFrame::VLAN_OVERHEAD_BYTES)
    }
}

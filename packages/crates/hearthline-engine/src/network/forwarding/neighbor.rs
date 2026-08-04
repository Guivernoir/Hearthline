use core::net::Ipv4Addr;

use heapless::Vec as FixedList;
use hearthline_model::{MacAddress, PortId};

const NEIGHBOR_CAPACITY: usize = 32;
const REACHABLE_TIME_US: u64 = 1_200_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeighborState {
    Reachable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NeighborEntry {
    pub address: Ipv4Addr,
    pub mac: MacAddress,
    pub port: PortId,
    pub state: NeighborState,
    pub updated_at_us: u64,
    pub expires_at_us: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct NeighborCache {
    entries: FixedList<NeighborEntry, NEIGHBOR_CAPACITY>,
}

impl NeighborCache {
    pub fn learn(&mut self, address: Ipv4Addr, mac: MacAddress, port: &PortId, now_us: u64) {
        self.expire(now_us);
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.address == address && entry.port == *port)
        {
            entry.mac = mac;
            entry.state = NeighborState::Reachable;
            entry.updated_at_us = now_us;
            entry.expires_at_us = now_us.saturating_add(REACHABLE_TIME_US);
            return;
        }
        if self.entries.is_full() {
            let oldest = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.updated_at_us)
                .map(|(index, _)| index)
                .expect("full neighbor cache has an entry");
            self.entries.swap_remove(oldest);
        }
        self.entries
            .push(NeighborEntry {
                address,
                mac,
                port: port.clone(),
                state: NeighborState::Reachable,
                updated_at_us: now_us,
                expires_at_us: now_us.saturating_add(REACHABLE_TIME_US),
            })
            .expect("neighbor cache has capacity after eviction");
    }

    pub fn lookup(&mut self, address: Ipv4Addr, port: &PortId, now_us: u64) -> Option<MacAddress> {
        self.expire(now_us);
        self.entries
            .iter()
            .find(|entry| entry.address == address && entry.port == *port)
            .map(|entry| entry.mac)
    }

    pub fn entry(&self, address: Ipv4Addr, port: &PortId, now_us: u64) -> Option<&NeighborEntry> {
        self.entries.iter().find(|entry| {
            entry.address == address && entry.port == *port && entry.expires_at_us > now_us
        })
    }

    fn expire(&mut self, now_us: u64) {
        let mut index = 0;
        while index < self.entries.len() {
            if self.entries[index].expires_at_us <= now_us {
                self.entries.swap_remove(index);
            } else {
                index += 1;
            }
        }
    }
}

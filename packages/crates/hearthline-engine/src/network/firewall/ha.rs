use heapless::Vec as FixedList;
use hearthline_model::{
    EthernetFrame, FirewallHaMessage, MacAddress, NetworkPayload, PortId, Text, VlanId,
};

use crate::runtime::{collect_fixed, runtime_text, single_effect};
use crate::{DropReason, Effect, EffectList, FirewallHaControl, NetworkIngress};

use super::{FirewallSession, StatefulFirewall};

const HA_WIRE_BYTES: u16 = 128;
const HA_INTERNAL_VLAN: u16 = 4094;
const HA_MULTICAST_MAC: MacAddress = MacAddress::new([0x01, 0x00, 0x5e, 0x00, 0x00, 0x12]);

#[derive(Clone, Debug)]
pub struct FirewallHaRuntimeConfig {
    pub domain: Text<64>,
    pub sync_port: PortId,
    pub sync_mac: MacAddress,
    pub monitored_ports: FixedList<PortId, 4>,
    pub active: bool,
    pub session_sync: bool,
    pub heartbeat_interval_us: u64,
    pub failure_hold_us: u64,
}

impl FirewallHaRuntimeConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain: Text<64>,
        sync_port: PortId,
        sync_mac: MacAddress,
        monitored_ports: impl IntoIterator<Item = PortId>,
        active: bool,
        session_sync: bool,
        heartbeat_interval_us: u64,
        failure_hold_us: u64,
    ) -> Self {
        assert!(
            sync_mac.is_unicast(),
            "firewall HA sync MAC must be unicast"
        );
        assert!(
            heartbeat_interval_us > 0,
            "firewall HA heartbeat interval must be non-zero"
        );
        assert!(
            failure_hold_us >= heartbeat_interval_us,
            "firewall HA failure hold must cover one heartbeat interval"
        );
        Self {
            domain,
            sync_port,
            sync_mac,
            monitored_ports: collect_fixed(monitored_ports),
            active,
            session_sync,
            heartbeat_interval_us,
            failure_hold_us,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirewallHaStatus {
    pub domain: Text<64>,
    pub active: bool,
    pub operational: bool,
    pub session_count: usize,
    pub replicated_updates: u64,
    pub last_heartbeat_us: Option<u64>,
    pub promoted_at_us: Option<u64>,
    pub promotion_inhibited_at_us: Option<u64>,
    pub heartbeat_interval_us: u64,
    pub failure_hold_us: u64,
}

impl StatefulFirewall {
    pub fn configure_ha(&mut self, config: FirewallHaRuntimeConfig) {
        self.ha_active = config.active;
        self.ha_promotion_inhibited_at_us = None;
        self.ha = Some(config);
        self.apply_ha_role_to_first_hops();
    }

    pub const fn ha_active(&self) -> bool {
        self.ha_active
    }

    pub fn set_ha_active(&mut self, active: bool) {
        self.ha_active = active;
        if active {
            self.ha_promotion_inhibited_at_us = None;
        }
        self.apply_ha_role_to_first_hops();
    }

    pub fn set_ha_sync_attached(&mut self, port: &PortId, attached: bool) -> bool {
        let Some(ha) = &self.ha else {
            return false;
        };
        if ha.sync_port != *port {
            return false;
        }
        self.ha_sync_attached = attached;
        true
    }

    pub fn replicate_sessions_from(&mut self, active_peer: &Self) -> usize {
        self.sessions = active_peer.sessions.clone();
        self.ha_replicated_updates = self
            .ha_replicated_updates
            .saturating_add(self.sessions.len() as u64);
        self.sessions.len()
    }

    pub fn ha_status(&self) -> Option<FirewallHaStatus> {
        let ha = self.ha.as_ref()?;
        Some(FirewallHaStatus {
            domain: ha.domain.clone(),
            active: self.ha_active,
            operational: self.operational,
            session_count: self.sessions.len(),
            replicated_updates: self.ha_replicated_updates,
            last_heartbeat_us: self.ha_last_heartbeat_us,
            promoted_at_us: self.ha_promoted_at_us,
            promotion_inhibited_at_us: self.ha_promotion_inhibited_at_us,
            heartbeat_interval_us: ha.heartbeat_interval_us,
            failure_hold_us: ha.failure_hold_us,
        })
    }

    pub(super) fn apply_ha_role_to_first_hops(&mut self) {
        let Some(ha) = &self.ha else {
            return;
        };
        for port in &ha.monitored_ports {
            let _ = self.plane.set_all_first_hop_active(port, self.ha_active);
        }
    }

    fn upsert_replicated_session(&mut self, session: FirewallSession) {
        if let Some(existing) = self
            .sessions
            .iter_mut()
            .find(|candidate| candidate.forward == session.forward)
        {
            *existing = session;
        } else {
            if self.sessions.is_full() {
                let oldest = self
                    .sessions
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, candidate)| candidate.expires_at_us)
                    .map(|(index, _)| index)
                    .expect("full session table has an entry");
                self.sessions.swap_remove(oldest);
            }
            self.sessions
                .push(session)
                .expect("replicated session table has capacity after eviction");
        }
        self.ha_replicated_updates = self.ha_replicated_updates.saturating_add(1);
    }

    fn ha_frame(&self, message: FirewallHaMessage) -> Option<EthernetFrame> {
        let ha = self.ha.as_ref()?;
        Some(EthernetFrame {
            source: ha.sync_mac,
            destination: HA_MULTICAST_MAC,
            vlan: VlanId::new(HA_INTERNAL_VLAN).expect("HA internal VLAN is valid"),
            payload: NetworkPayload::FirewallHa(message),
            wire_len_bytes: HA_WIRE_BYTES,
        })
    }

    pub(super) fn heartbeat_effect(&mut self, sent_at_us: u64) -> Option<Effect> {
        let ha = self.ha.as_ref()?;
        if !self.operational || !self.ha_active || !self.ha_sync_attached {
            return None;
        }
        let message = FirewallHaMessage::Heartbeat {
            domain: ha.domain.clone(),
            sequence: self.ha_next_sequence,
            sent_at_us,
        };
        self.ha_next_sequence = self.ha_next_sequence.wrapping_add(1);
        Some(Effect::Transmit {
            egress: ha.sync_port.clone(),
            next_hop: None,
            frame: self
                .ha_frame(message)
                .expect("configured HA member can build a heartbeat"),
            delay_ms: 0,
        })
    }

    pub(super) fn session_sync_effect(&mut self, session: FirewallSession) -> Option<Effect> {
        let ha = self.ha.as_ref()?;
        if !ha.session_sync || !self.operational || !self.ha_active || !self.ha_sync_attached {
            return None;
        }
        let message = FirewallHaMessage::SessionUpsert {
            domain: ha.domain.clone(),
            generation: self.ha_replicated_updates,
            flow: session.forward,
            expires_at_us: session.expires_at_us,
        };
        Some(Effect::Transmit {
            egress: ha.sync_port.clone(),
            next_hop: None,
            frame: self
                .ha_frame(message)
                .expect("configured HA member can build a session update"),
            delay_ms: 0,
        })
    }

    pub(super) fn handle_ha_network(&mut self, ingress: NetworkIngress) -> EffectList {
        let NetworkPayload::FirewallHa(message) = ingress.frame.payload else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        let Some(ha) = &self.ha else {
            return single_effect(Effect::Drop(DropReason::UnsupportedProtocol));
        };
        let domain = match &message {
            FirewallHaMessage::Heartbeat { domain, .. }
            | FirewallHaMessage::SessionUpsert { domain, .. } => domain,
        };
        if *domain != ha.domain {
            return single_effect(Effect::Drop(DropReason::FirewallHaDomainMismatch));
        }
        match message {
            FirewallHaMessage::Heartbeat { sequence, .. } => {
                if self
                    .ha_last_heartbeat_sequence
                    .is_none_or(|current| sequence > current)
                {
                    self.ha_last_heartbeat_sequence = Some(sequence);
                    self.ha_last_heartbeat_us = Some(ingress.received_at_us);
                }
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!(
                        "firewall HA heartbeat {sequence} received at {} us",
                        ingress.received_at_us
                    )),
                })
            }
            FirewallHaMessage::SessionUpsert {
                flow,
                expires_at_us,
                ..
            } => {
                if !self.ha_active && ha.session_sync {
                    self.upsert_replicated_session(FirewallSession {
                        forward: flow,
                        reverse: flow.reverse(),
                        expires_at_us,
                    });
                }
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!(
                        "firewall HA synchronized session {} -> {}",
                        flow.source, flow.destination
                    )),
                })
            }
        }
    }

    pub(super) fn handle_ha_control(&mut self, control: FirewallHaControl) -> EffectList {
        match control {
            FirewallHaControl::HeartbeatTick { at_us } => self
                .heartbeat_effect(at_us)
                .map_or_else(EffectList::new, single_effect),
            FirewallHaControl::ClearReplicatedSessions { at_us } => {
                let cleared = self.sessions.len();
                self.sessions.clear();
                single_effect(Effect::Observe {
                    detail: runtime_text(format_args!(
                        "firewall HA cleared {cleared} replicated session(s) at {at_us} us"
                    )),
                })
            }
            FirewallHaControl::EvaluatePeer {
                at_us,
                peer_failure_confirmed,
            } => {
                let Some(ha) = &self.ha else {
                    return EffectList::new();
                };
                let monitored_ports = ha.monitored_ports.clone();
                let timed_out = !self.ha_active
                    && self.operational
                    && self
                        .ha_last_heartbeat_us
                        .is_some_and(|last| at_us.saturating_sub(last) >= ha.failure_hold_us);
                if !timed_out {
                    return EffectList::new();
                }
                if !peer_failure_confirmed {
                    self.ha_promotion_inhibited_at_us = Some(at_us);
                    return single_effect(Effect::Observe {
                        detail: runtime_text(format_args!(
                            "firewall HA promotion inhibited at {at_us} us: peer failure is unconfirmed"
                        )),
                    });
                }
                self.ha_active = true;
                self.ha_promoted_at_us = Some(at_us);
                self.ha_promotion_inhibited_at_us = None;
                self.apply_ha_role_to_first_hops();
                let mut effects = single_effect(Effect::Observe {
                    detail: runtime_text(format_args!(
                        "firewall HA promoted after heartbeat timeout at {at_us} us"
                    )),
                });
                for effect in self.plane.first_hop_announcements(&monitored_ports) {
                    effects
                        .push(effect)
                        .expect("HA first-hop announcements fit effect capacity");
                }
                effects
            }
        }
    }
}

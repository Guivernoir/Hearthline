use hearthline_engine::{
    Actuator, DnsServer, EffectList, FieldSensor, FirewallHaStatus, Layer3Switch, LearningSwitch,
    LinkAppliance, NatRouter, OperatorInterface, RemoteIo, ReverseProxyWaf, Router,
    SafetyInterface, ServiceNode, SimulatedComponent, SimulationEvent, StatefulFirewall,
    VirtualPlc,
};
use hearthline_model::{ComponentId, ComponentKind, PortId, VlanId};

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
use std::net::Ipv4Addr;

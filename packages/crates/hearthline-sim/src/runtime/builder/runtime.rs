use hearthline_engine::{
    LinkAppliance, RoutedInterface, RoutingTable, SwitchAggregationGroup, SwitchPort,
    UnaddressedNode,
};
use hearthline_model::{Ipv4Cidr, Ipv4InterfaceAddress, MacAddress, PortId, Route};

use crate::runtime::ConfiguredAppliance;
use hearthline_config::parse_service_kind;
use hearthline_config::{
    ApplianceConfig, BehaviorConfig, ConfigError, InterfaceConfig, InterfaceMode, RouteConfig,
};

use super::{configure_first_hop, interface_vlan, parse_ipv4, port_id, vlan_id};

pub(in crate::runtime) fn routed_interfaces(
    config: &ApplianceConfig,
) -> Result<Vec<RoutedInterface>, ConfigError> {
    config
        .interfaces
        .iter()
        .filter(|interface| !interface.addresses.is_empty())
        .map(|interface| {
            let mac = interface
                .mac_address
                .as_deref()
                .ok_or_else(|| {
                    ConfigError::new(format!(
                        "appliance {} interface {} requires mac_address for simulation",
                        config.id, interface.id
                    ))
                })?
                .parse::<MacAddress>()
                .map_err(|error| ConfigError::new(error.to_string()))?;
            let addresses = interface
                .addresses
                .iter()
                .map(|address| {
                    address
                        .parse::<Ipv4InterfaceAddress>()
                        .map_err(|error| ConfigError::new(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut runtime = RoutedInterface::new(
                port_id(&interface.id)?,
                mac,
                addresses,
                interface_vlan(interface)?,
                u16::try_from(interface.settings.mtu)
                    .map_err(|_| ConfigError::new("interface MTU exceeds runtime limit"))?,
            );
            runtime.forwarding = interface.state.initially_usable();
            configure_first_hop(&mut runtime, &config.id, interface)?;
            Ok(runtime)
        })
        .collect()
}

pub(super) fn routing_table(routes: &[RouteConfig]) -> Result<RoutingTable, ConfigError> {
    Ok(RoutingTable::new(runtime_routes(routes)?))
}

pub(in crate::runtime) fn runtime_routes(
    routes: &[RouteConfig],
) -> Result<Vec<Route>, ConfigError> {
    routes
        .iter()
        .map(|route| {
            let destination = route
                .destination
                .parse::<Ipv4Cidr>()
                .map_err(|error| ConfigError::new(error.to_string()))?;
            Ok(Route {
                destination,
                egress: port_id(&route.interface)?,
                next_hop: route
                    .next_hop
                    .as_deref()
                    .map(|value| parse_ipv4(value, "route next hop"))
                    .transpose()?,
                metric: if destination.prefix() == 0 { 10 } else { 0 },
            })
        })
        .collect()
}

pub(super) fn switch_port(
    interface: &InterfaceConfig,
    fallback: &[u16],
) -> Result<SwitchPort, ConfigError> {
    let vlans = if interface.vlans.is_empty() {
        fallback
    } else {
        &interface.vlans
    };
    let mut port = if interface.mode == InterfaceMode::Trunk {
        SwitchPort::trunk(
            port_id(&interface.id)?,
            vlans
                .iter()
                .map(|vlan| vlan_id(*vlan))
                .collect::<Result<Vec<_>, _>>()?,
        )
    } else {
        SwitchPort::access(
            port_id(&interface.id)?,
            vlan_id(*vlans.first().ok_or_else(|| {
                ConfigError::new(format!("switch port {} requires a VLAN", interface.id))
            })?)?,
        )
    };
    port.forwarding = interface.state.initially_usable();
    Ok(port)
}

pub(super) fn runtime_aggregation_groups(
    config: &ApplianceConfig,
) -> Result<Vec<SwitchAggregationGroup>, ConfigError> {
    config
        .link_aggregation
        .iter()
        .flat_map(|aggregation| &aggregation.groups)
        .map(|group| {
            Ok(SwitchAggregationGroup::new(
                super::component_id(&group.id)?,
                super::component_id(&group.logical_id)?,
                group
                    .members
                    .iter()
                    .map(|member| port_id(member))
                    .collect::<Result<Vec<_>, _>>()?,
                config.multi_chassis.is_some(),
            ))
        })
        .collect()
}

pub(super) fn parse_services(
    values: &[String],
) -> Result<Vec<hearthline_model::ServiceKind>, ConfigError> {
    values
        .iter()
        .map(|value| parse_service_kind(value))
        .collect()
}

pub(super) fn configured_ports(config: &ApplianceConfig) -> Result<Vec<PortId>, ConfigError> {
    if config.interfaces.len() < 2 {
        return Err(ConfigError::new(format!(
            "link appliance {} requires at least two interfaces",
            config.id
        )));
    }
    config
        .interfaces
        .iter()
        .map(|interface| port_id(&interface.id))
        .collect()
}

pub(super) fn has_complete_addressing(config: &ApplianceConfig) -> bool {
    let addressed = config
        .interfaces
        .iter()
        .filter(|interface| !interface.addresses.is_empty())
        .collect::<Vec<_>>();
    !addressed.is_empty()
        && addressed
            .iter()
            .all(|interface| interface.mac_address.is_some())
}

pub(super) fn behavior_requires_addressing(behavior: &BehaviorConfig) -> bool {
    matches!(
        behavior,
        BehaviorConfig::Endpoint { .. }
            | BehaviorConfig::ServiceHost { .. }
            | BehaviorConfig::PolicyService { .. }
            | BehaviorConfig::Voice { .. }
            | BehaviorConfig::ComputeHost { .. }
            | BehaviorConfig::Router { .. }
            | BehaviorConfig::NatRouter { .. }
            | BehaviorConfig::StatefulFirewall { .. }
            | BehaviorConfig::ApplicationGateway { .. }
    )
}

pub(super) fn unaddressed_appliance(
    config: &ApplianceConfig,
    id: hearthline_model::ComponentId,
) -> Result<ConfiguredAppliance, ConfigError> {
    let ports = config
        .interfaces
        .iter()
        .map(|interface| port_id(&interface.id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ConfiguredAppliance::Unaddressed(Box::new(
        UnaddressedNode::new(id, config.kind, ports),
    )))
}

pub(super) fn configure_link_ports(
    appliance: &mut LinkAppliance,
    config: &ApplianceConfig,
) -> Result<(), ConfigError> {
    for interface in &config.interfaces {
        let port = port_id(&interface.id)?;
        if !appliance.set_port_forwarding(&port, interface.state.initially_usable()) {
            return Err(ConfigError::new(format!(
                "link appliance {} is missing interface {}",
                config.id, interface.id
            )));
        }
    }
    Ok(())
}

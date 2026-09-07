use hearthline_engine::{
    DnsServer, Layer3Switch, LearningSwitch, LinkAppliance, LinkMode, NatRouter, PassiveSensor,
    RoutedInterface, Router, ServiceNode, StatefulFirewall, StaticNat, WirelessAccessPoint,
};
use hearthline_model::{ComponentKind, HttpDocument, Text};

use hearthline_config::{BehaviorConfig, ConfigError, InterfaceMode, LoadedAppliance};

mod first_hop;
mod policy;
mod process;
mod runtime;
mod support;

use self::first_hop::configure_first_hop;
use self::policy::{configure_firewall_ha, firewall_rules};
use self::runtime::{
    behavior_requires_addressing, configure_link_ports, configured_ports, has_complete_addressing,
    parse_services, routing_table, runtime_aggregation_groups, switch_port, unaddressed_appliance,
};
pub(super) use self::runtime::{routed_interfaces, runtime_routes};
pub(super) use self::support::parse_ipv4;
use self::support::{component_id, interface_vlan, port_id, vlan_id};
use super::ConfiguredAppliance;

pub(super) fn build_appliance(
    loaded: &LoadedAppliance,
    appliances: &crate::ConfigRepository,
) -> Result<ConfiguredAppliance, ConfigError> {
    let config = &loaded.config;
    let id = component_id(&config.id)?;
    let virtual_switch_host = matches!(config.behavior, BehaviorConfig::ComputeHost { .. })
        && config
            .interfaces
            .iter()
            .all(|interface| interface.addresses.is_empty());
    if behavior_requires_addressing(&config.behavior)
        && !virtual_switch_host
        && !has_complete_addressing(config)
    {
        return unaddressed_appliance(config, id);
    }
    match &config.behavior {
        BehaviorConfig::ServiceHost {
            dns_records,
            respond_to_icmp,
            ..
        } if config.kind == ComponentKind::DnsServer => {
            if !has_complete_addressing(config) {
                return unaddressed_appliance(config, id);
            }
            let records = dns_records
                .iter()
                .map(|record| {
                    Ok((
                        Text::<128>::try_new(&record.name)
                            .map_err(|error| ConfigError::new(error.to_string()))?,
                        parse_ipv4(&record.address, "DNS record address")?,
                    ))
                })
                .collect::<Result<Vec<_>, ConfigError>>()?;
            let interfaces = routed_interfaces(config)?;
            let mut appliance = if let Some(gateway) = &config.default_gateway {
                DnsServer::with_default_gateway(
                    id,
                    interfaces,
                    parse_ipv4(gateway, "default gateway")?,
                    records,
                )
            } else {
                DnsServer::new(id, interfaces, records)
            };
            appliance.set_respond_to_icmp(*respond_to_icmp);
            Ok(ConfiguredAppliance::Dns(Box::new(appliance)))
        }
        BehaviorConfig::ComputeHost { .. }
            if config
                .interfaces
                .iter()
                .all(|interface| interface.addresses.is_empty()) =>
        {
            let mut appliance =
                LinkAppliance::embedded_virtual_switch(id, configured_ports(config)?);
            configure_link_ports(&mut appliance, config)?;
            Ok(ConfiguredAppliance::Link(Box::new(appliance)))
        }
        BehaviorConfig::Endpoint {
            accepted_services, ..
        }
        | BehaviorConfig::ServiceHost {
            accepted_services, ..
        }
        | BehaviorConfig::PolicyService {
            accepted_services, ..
        }
        | BehaviorConfig::Voice {
            accepted_services, ..
        }
        | BehaviorConfig::ComputeHost {
            accepted_services, ..
        } => {
            if !has_complete_addressing(config) {
                return unaddressed_appliance(config, id);
            }
            let interfaces = routed_interfaces(config)?;
            let services = parse_services(accepted_services)?;
            let mut appliance = if let Some(gateway) = &config.default_gateway {
                ServiceNode::with_default_gateway(
                    id,
                    config.kind,
                    interfaces,
                    parse_ipv4(gateway, "default gateway")?,
                    services,
                )
            } else {
                ServiceNode::new(id, config.kind, interfaces, services)
            };
            appliance.set_respond_to_icmp(config.behavior.responds_to_icmp());
            if let BehaviorConfig::ServiceHost {
                http_site: Some(site),
                ..
            } = &config.behavior
            {
                appliance.set_http_site(
                    Text::try_new(&site.host)
                        .map_err(|error| ConfigError::new(error.to_string()))?,
                    HttpDocument {
                        title: Text::try_new(&site.title)
                            .map_err(|error| ConfigError::new(error.to_string()))?,
                        heading: Text::try_new(&site.heading)
                            .map_err(|error| ConfigError::new(error.to_string()))?,
                        body: Text::try_new(&site.body)
                            .map_err(|error| ConfigError::new(error.to_string()))?,
                    },
                );
            }
            Ok(ConfiguredAppliance::Endpoint(Box::new(appliance)))
        }
        BehaviorConfig::EthernetSwitch { vlans, .. } => {
            let ports = config
                .interfaces
                .iter()
                .map(|interface| switch_port(interface, vlans))
                .collect::<Result<Vec<_>, _>>()?;
            let mut appliance = LearningSwitch::new(id, ports);
            for group in runtime_aggregation_groups(config)? {
                if !appliance.add_link_aggregation_group(group) {
                    return Err(ConfigError::new(format!(
                        "appliance {} has invalid runtime aggregation membership",
                        config.id
                    )));
                }
            }
            if let Some(domain) = &config.multi_chassis
                && !appliance.set_multi_chassis_peer_link(port_id(&domain.peer_link)?)
            {
                return Err(ConfigError::new(format!(
                    "appliance {} has invalid multi-chassis peer link",
                    config.id
                )));
            }
            Ok(ConfiguredAppliance::Switch(Box::new(appliance)))
        }
        BehaviorConfig::Router { routes, .. } => {
            if config.kind == ComponentKind::Layer3Switch {
                let switch_ports = config
                    .interfaces
                    .iter()
                    .filter(|interface| {
                        matches!(interface.mode, InterfaceMode::Access | InterfaceMode::Trunk)
                    })
                    .map(|interface| switch_port(interface, &interface.vlans))
                    .collect::<Result<Vec<_>, _>>()?;
                let svi_ports = config
                    .interfaces
                    .iter()
                    .filter(|interface| interface.mode == InterfaceMode::Svi)
                    .map(|interface| port_id(&interface.id))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut appliance = Layer3Switch::new(
                    id,
                    switch_ports,
                    routed_interfaces(config)?,
                    svi_ports,
                    routing_table(routes)?,
                );
                for group in runtime_aggregation_groups(config)? {
                    if !appliance.add_link_aggregation_group(group) {
                        return Err(ConfigError::new(format!(
                            "appliance {} has invalid runtime aggregation membership",
                            config.id
                        )));
                    }
                }
                if let Some(domain) = &config.multi_chassis
                    && !appliance.set_multi_chassis_peer_link(port_id(&domain.peer_link)?)
                {
                    return Err(ConfigError::new(format!(
                        "appliance {} has invalid multi-chassis peer link",
                        config.id
                    )));
                }
                Ok(ConfiguredAppliance::Layer3Switch(Box::new(appliance)))
            } else {
                Ok(ConfiguredAppliance::Router(Box::new(Router::new(
                    id,
                    config.kind,
                    routed_interfaces(config)?,
                    routing_table(routes)?,
                ))))
            }
        }
        BehaviorConfig::NatRouter {
            routes,
            inside_interfaces,
            outside_interfaces,
            translations,
        } => {
            let interfaces = routed_interfaces(config)?;
            let outside_address = outside_interfaces
                .iter()
                .find_map(|port| {
                    interfaces
                        .iter()
                        .find(|interface| interface.id.as_str() == port)
                        .and_then(RoutedInterface::primary_address)
                })
                .ok_or_else(|| {
                    ConfigError::new(format!(
                        "NAT appliance {} requires an addressed outside interface",
                        config.id
                    ))
                })?;
            let mut appliance = NatRouter::new(
                id,
                interfaces,
                inside_interfaces
                    .iter()
                    .map(|port| port_id(port))
                    .collect::<Result<Vec<_>, _>>()?,
                outside_address,
                routing_table(routes)?,
            );
            for translation in translations {
                appliance
                    .add_static_nat(StaticNat {
                        public_address: parse_ipv4(
                            &translation.public_address,
                            "public NAT address",
                        )?,
                        private_address: parse_ipv4(
                            &translation.private_address,
                            "private NAT address",
                        )?,
                    })
                    .map_err(|error| {
                        ConfigError::new(format!("appliance {}: {error}", config.id))
                    })?;
            }
            Ok(ConfiguredAppliance::NatRouter(Box::new(appliance)))
        }
        BehaviorConfig::StatefulFirewall {
            stateful,
            zones,
            routes,
            rules,
            ..
        } => {
            if !stateful {
                return Err(ConfigError::new(format!(
                    "configured firewall {} must enable stateful inspection",
                    config.id
                )));
            }
            let interfaces = routed_interfaces(config)?;
            let runtime_zones = zones
                .iter()
                .map(|zone| {
                    let port = port_id(&zone.interface)?;
                    if !interfaces.iter().any(|interface| interface.id == port) {
                        return Err(ConfigError::new(format!(
                            "firewall {} zone {} references non-routed interface {}",
                            config.id, zone.name, zone.interface
                        )));
                    }
                    Ok((
                        port,
                        Text::<64>::try_new(&zone.name)
                            .map_err(|error| ConfigError::new(error.to_string()))?,
                    ))
                })
                .collect::<Result<Vec<_>, ConfigError>>()?;
            if runtime_zones.len() != interfaces.len() {
                return Err(ConfigError::new(format!(
                    "firewall {} requires exactly one zone for every routed interface",
                    config.id
                )));
            }
            let mut appliance = StatefulFirewall::new(
                id,
                runtime_zones,
                interfaces,
                routing_table(routes)?,
                firewall_rules(rules)?,
            );
            configure_firewall_ha(&mut appliance, config)?;
            Ok(ConfiguredAppliance::Firewall(Box::new(appliance)))
        }
        BehaviorConfig::ApplicationGateway { .. } => super::gateway::build_web_gateway(id, config),
        BehaviorConfig::TransparentLink { operational } => {
            let mut appliance = LinkAppliance::new(
                id,
                config.kind,
                configured_ports(config)?,
                LinkMode::Transparent,
            );
            configure_link_ports(&mut appliance, config)?;
            if !operational {
                use hearthline_engine::SimulatedComponent as _;
                appliance.handle(hearthline_engine::SimulationEvent::SetOperational(false));
            }
            Ok(ConfiguredAppliance::Link(Box::new(appliance)))
        }
        BehaviorConfig::ImpairedLink {
            operational,
            delay_ms,
            loss_every,
        } => {
            let mut appliance = LinkAppliance::new(
                id,
                config.kind,
                configured_ports(config)?,
                LinkMode::Wan {
                    delay_ms: *delay_ms,
                    drop_every: *loss_every,
                },
            );
            configure_link_ports(&mut appliance, config)?;
            if !operational {
                use hearthline_engine::SimulatedComponent as _;
                appliance.handle(hearthline_engine::SimulationEvent::SetOperational(false));
            }
            Ok(ConfiguredAppliance::Link(Box::new(appliance)))
        }
        BehaviorConfig::WirelessBridge { client_vlan, .. } => {
            let ports = config
                .interfaces
                .iter()
                .map(|interface| switch_port(interface, &[*client_vlan]))
                .collect::<Result<Vec<_>, _>>()?;
            let wireless_port = config
                .interfaces
                .iter()
                .find(|interface| {
                    interface.hardware == hearthline_engine::PortHardwareKind::WirelessRadio
                })
                .ok_or_else(|| {
                    ConfigError::new(format!(
                        "wireless appliance {} requires a radio interface",
                        config.id
                    ))
                })?;
            Ok(ConfiguredAppliance::Wireless(Box::new(
                WirelessAccessPoint::new(id, ports, port_id(&wireless_port.id)?, []),
            )))
        }
        BehaviorConfig::PassiveMonitor { inline, .. } => {
            if *inline {
                return Err(ConfigError::new(format!(
                    "passive monitor {} cannot use an inline topology",
                    config.id
                )));
            }
            let ports = config
                .interfaces
                .iter()
                .map(|interface| port_id(&interface.id))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ConfiguredAppliance::Monitor(Box::new(PassiveSensor::new(
                id, ports,
            ))))
        }
        BehaviorConfig::VirtualController { .. }
        | BehaviorConfig::OperatorInterface { .. }
        | BehaviorConfig::RemoteIo { .. }
        | BehaviorConfig::FieldSensor { .. }
        | BehaviorConfig::FieldActuator { .. }
        | BehaviorConfig::Safety { .. } => process::build_process_appliance(config, appliances),
    }
}

use hearthline_engine::{FirewallAction, FirewallHaRuntimeConfig, FirewallRule, StatefulFirewall};
use hearthline_model::{Ipv4Cidr, MacAddress, Text, TransportProtocol};

use crate::appliance::{ApplianceConfig, ConfigError, PolicyAction, PolicyRuleConfig};

use super::port_id;

pub(super) fn configure_firewall_ha(
    appliance: &mut StatefulFirewall,
    config: &ApplianceConfig,
) -> Result<(), ConfigError> {
    let Some(ha) = &config.firewall_ha else {
        return Ok(());
    };
    let sync_interface = config
        .interfaces
        .iter()
        .find(|interface| interface.id == ha.sync_interface)
        .expect("firewall HA validation guarantees the sync interface");
    let sync_mac = sync_interface
        .mac_address
        .as_deref()
        .expect("firewall HA validation guarantees the sync MAC")
        .parse::<MacAddress>()
        .map_err(|error| ConfigError::new(error.to_string()))?;
    appliance.configure_ha(FirewallHaRuntimeConfig::new(
        Text::try_new(&ha.domain).map_err(|error| ConfigError::new(error.to_string()))?,
        port_id(&ha.sync_interface)?,
        sync_mac,
        ha.monitored_interfaces
            .iter()
            .map(|interface| port_id(interface))
            .collect::<Result<Vec<_>, _>>()?,
        ha.role.is_active(),
        ha.session_sync,
        ha.heartbeat_interval_ms.saturating_mul(1_000),
        ha.failure_hold_ms.saturating_mul(1_000),
    ));
    Ok(())
}

pub(super) fn firewall_rules(rules: &[PolicyRuleConfig]) -> Result<Vec<FirewallRule>, ConfigError> {
    rules
        .iter()
        .map(|rule| {
            let (protocol, destination_port) = policy_service(&rule.service)?;
            Ok(FirewallRule {
                id: Text::try_new(&rule.name)
                    .map_err(|error| ConfigError::new(error.to_string()))?,
                source_zone: optional_text(rule.source_zone.as_deref())?,
                destination_zone: optional_text(rule.destination_zone.as_deref())?,
                source: policy_prefix(&rule.source, "firewall source")?,
                destination: policy_prefix(&rule.destination, "firewall destination")?,
                protocol,
                destination_port,
                action: match rule.action {
                    PolicyAction::Permit => FirewallAction::Permit,
                    PolicyAction::Deny => FirewallAction::Deny,
                },
            })
        })
        .collect()
}

fn policy_service(service: &str) -> Result<(Option<TransportProtocol>, Option<u16>), ConfigError> {
    match service {
        "any" => Ok((None, None)),
        "dns-udp" => Ok((Some(TransportProtocol::Udp), Some(53))),
        "dns-tcp" => Ok((Some(TransportProtocol::Tcp), Some(53))),
        "http" => Ok((Some(TransportProtocol::Tcp), Some(80))),
        "https" | "analytics" | "historian-replication" => {
            Ok((Some(TransportProtocol::Tcp), Some(443)))
        }
        "ssh" => Ok((Some(TransportProtocol::Tcp), Some(22))),
        other => Err(ConfigError::new(format!(
            "unsupported configured firewall service {other}"
        ))),
    }
}

fn policy_prefix(value: &str, field: &str) -> Result<Option<Ipv4Cidr>, ConfigError> {
    if value == "any" {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|error| ConfigError::new(format!("invalid {field} {value}: {error}")))
    }
}

fn optional_text(value: Option<&str>) -> Result<Option<Text<64>>, ConfigError> {
    value
        .map(|value| Text::try_new(value).map_err(|error| ConfigError::new(error.to_string())))
        .transpose()
}

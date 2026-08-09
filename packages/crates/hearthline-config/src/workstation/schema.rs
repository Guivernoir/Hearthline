use std::collections::BTreeMap;

use hearthline_model::ComponentKind;
use serde::{Deserialize, Serialize};

use crate::appliance::BehaviorConfig;
use crate::scenario::{InteractiveScenarioSession, is_interactive_scenario};
use crate::{
    ConfigError, ConfigRepository, ConnectionRepository, RuntimeDeviceSnapshot,
    ScenarioApplicationConfig, ScenarioConfig, ScenarioHttpMethod, ScenarioHttpResponse,
    ScenarioPacketConfig, ScenarioReport, ScenarioRepository,
};

pub const WORKSTATION_SCHEMA_VERSION: &str = "0.10.0";
pub const WORKSTATION_DNS_TTL_MS: u64 = 60_000;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CachedDnsRecord {
    address: String,
    expires_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkstationDnsCacheEntry {
    pub name: String,
    pub address: String,
    pub remaining_ttl_ms: u64,
}

#[derive(Clone, Debug, Default)]
pub struct WorkstationSession {
    elapsed_ms: u64,
    dns_cache: BTreeMap<String, CachedDnsRecord>,
    network: Option<InteractiveScenarioSession>,
}

impl WorkstationSession {
    pub fn tick(&mut self, elapsed_ms: u64) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(elapsed_ms);
        self.remove_expired_dns();
        if let Some(network) = &mut self.network {
            network.tick(elapsed_ms);
        }
    }

    pub fn cached_dns_address(&mut self, name: &str) -> Option<String> {
        self.remove_expired_dns();
        self.dns_cache
            .get(&name.to_ascii_lowercase())
            .map(|record| record.address.clone())
    }

    pub fn remember_dns(&mut self, name: &str, address: &str) {
        self.dns_cache.insert(
            name.to_ascii_lowercase(),
            CachedDnsRecord {
                address: address.into(),
                expires_at_ms: self.elapsed_ms.saturating_add(WORKSTATION_DNS_TTL_MS),
            },
        );
    }

    pub fn dns_entries(&mut self) -> Vec<WorkstationDnsCacheEntry> {
        self.remove_expired_dns();
        self.dns_cache
            .iter()
            .map(|(name, record)| WorkstationDnsCacheEntry {
                name: name.clone(),
                address: record.address.clone(),
                remaining_ttl_ms: record.expires_at_ms.saturating_sub(self.elapsed_ms),
            })
            .collect()
    }

    pub fn flush_dns(&mut self) -> usize {
        let removed = self.dns_cache.len();
        self.dns_cache.clear();
        removed
    }

    pub fn network_state(&self) -> Result<WorkstationNetworkState, ConfigError> {
        let Some(network) = &self.network else {
            return Ok(WorkstationNetworkState::default());
        };
        let now_us = network.now_us();
        let arp_entries = network
            .endpoint_neighbors()?
            .into_iter()
            .map(|entry| WorkstationArpEntry {
                address: entry.address.to_string(),
                mac_address: entry.mac.to_string(),
                interface: entry.port.to_string(),
                remaining_ttl_ms: entry.expires_at_us.saturating_sub(now_us).div_ceil(1_000),
            })
            .collect();
        Ok(WorkstationNetworkState {
            active: true,
            simulated_time_ms: now_us / 1_000,
            arp_entries,
            pat_translations: network.active_pat_translation_count(),
            devices: network.runtime_devices(),
        })
    }

    pub(crate) fn run_scenario(
        &mut self,
        appliances: &ConfigRepository,
        connections: &ConnectionRepository,
        scenarios: &ScenarioRepository,
        source: &str,
        scenario: &ScenarioConfig,
        packet_override: Option<ScenarioPacketConfig>,
    ) -> Result<ScenarioReport, ConfigError> {
        if self.network.is_none() {
            self.network = Some(InteractiveScenarioSession::from_source(
                appliances,
                connections,
                scenarios,
                source,
            )?);
        }
        self.network
            .as_mut()
            .expect("interactive network was initialized")
            .run(appliances, connections, scenario, packet_override)
    }

    fn remove_expired_dns(&mut self) {
        let elapsed_ms = self.elapsed_ms;
        self.dns_cache
            .retain(|_, record| record.expires_at_ms > elapsed_ms);
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkstationNetworkState {
    pub active: bool,
    pub simulated_time_ms: u64,
    pub arp_entries: Vec<WorkstationArpEntry>,
    pub pat_translations: usize,
    pub devices: Vec<RuntimeDeviceSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkstationArpEntry {
    pub address: String,
    pub mac_address: String,
    pub interface: String,
    pub remaining_ttl_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkstationProfile {
    pub schema_version: &'static str,
    pub id: String,
    pub label: String,
    pub kind: String,
    pub site: String,
    pub environment: String,
    pub zone: String,
    pub role: String,
    pub hostname: String,
    pub browser_home: Option<String>,
    pub default_gateway: Option<String>,
    pub dns_servers: Vec<String>,
    pub interfaces: Vec<WorkstationInterface>,
    pub applications: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkstationInterface {
    pub id: String,
    pub hardware: String,
    pub mac_address: Option<String>,
    pub addresses: Vec<String>,
    pub administrative_state: String,
    pub operational_state: String,
    pub speed_mbps: u64,
    pub mtu: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WorkstationAction {
    Terminal { command: String },
    Browser { url: String },
    Inspect { appliance: String, command: String },
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkstationActionKind {
    Terminal,
    Browser,
    Inspect,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkstationActionStatus {
    Completed,
    Succeeded,
    Denied,
    Failed,
    Unsupported,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkstationActionReport {
    pub schema_version: &'static str,
    pub workstation_id: String,
    pub action: WorkstationActionKind,
    pub status: WorkstationActionStatus,
    pub title: String,
    pub output: Vec<String>,
    pub clear_output: bool,
    pub browser: Option<BrowserNavigation>,
    pub simulations: Vec<ScenarioReport>,
    pub network_state: WorkstationNetworkState,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserNavigation {
    pub url: String,
    pub method: String,
    pub request_body_bytes: usize,
    pub host: String,
    pub path: String,
    pub resolved_address: Option<String>,
    pub resolution_source: &'static str,
    pub gateway: Option<String>,
    pub forwarded_to: Option<String>,
    pub response: Option<ScenarioHttpResponse>,
    pub outcome: &'static str,
}

pub fn workstation_profile(
    appliances: &ConfigRepository,
    scenarios: &ScenarioRepository,
    id: &str,
) -> Result<WorkstationProfile, ConfigError> {
    let appliance = appliances
        .get(id)
        .ok_or_else(|| ConfigError::new(format!("unknown appliance {id}")))?;
    if !is_workstation(appliance.config.kind) {
        return Err(ConfigError::new(format!(
            "appliance {id} is not a workstation"
        )));
    }
    let (hostname, dns_servers) = endpoint_network_settings(&appliance.config.behavior)?;
    Ok(WorkstationProfile {
        schema_version: WORKSTATION_SCHEMA_VERSION,
        id: appliance.config.id.clone(),
        label: appliance.config.label.clone(),
        kind: appliance.config.kind.to_string(),
        site: appliance.config.site.clone(),
        environment: appliance.config.environment.clone(),
        zone: appliance.config.zone.clone(),
        role: appliance.config.role.clone(),
        hostname: hostname
            .clone()
            .unwrap_or_else(|| appliance.config.id.clone()),
        browser_home: browser_home(scenarios, id),
        default_gateway: appliance.config.default_gateway.clone(),
        dns_servers: dns_servers.to_vec(),
        interfaces: appliance
            .config
            .interfaces
            .iter()
            .map(|interface| WorkstationInterface {
                id: interface.id.clone(),
                hardware: interface.hardware.to_string(),
                mac_address: interface.mac_address.clone(),
                addresses: interface.addresses.clone(),
                administrative_state: interface.state.administrative.to_string(),
                operational_state: interface.state.initial_operational.to_string(),
                speed_mbps: interface.settings.speed_mbps,
                mtu: interface.settings.mtu,
            })
            .collect(),
        applications: vec!["terminal", "browser", "configuration"],
    })
}

fn browser_home(scenarios: &ScenarioRepository, source: &str) -> Option<String> {
    scenarios.scenarios().find_map(|scenario| {
        if !is_interactive_scenario(&scenario.config, source) || scenario.config.security.is_some()
        {
            return None;
        }
        let ScenarioApplicationConfig::HttpRequest {
            method: ScenarioHttpMethod::Get,
            host,
            path,
            ..
        } = &scenario.config.packet.application
        else {
            return None;
        };
        Some(format!("https://{host}{path}"))
    })
}

fn endpoint_network_settings(
    behavior: &BehaviorConfig,
) -> Result<(&Option<String>, &[String]), ConfigError> {
    let BehaviorConfig::Endpoint {
        hostname,
        dns_servers,
        ..
    } = behavior
    else {
        return Err(ConfigError::new(
            "interactive workstation requires endpoint behavior",
        ));
    };
    Ok((hostname, dns_servers))
}

fn is_workstation(kind: ComponentKind) -> bool {
    matches!(
        kind,
        ComponentKind::Workstation
            | ComponentKind::PrivilegedWorkstation
            | ComponentKind::EngineeringWorkstation
    )
}

use hearthline_model::ComponentKind;
use serde::{Deserialize, Serialize};

use crate::appliance::BehaviorConfig;
use crate::{
    ConfigError, ConfigRepository, ScenarioApplicationConfig, ScenarioHttpMethod,
    ScenarioHttpResponse, ScenarioReport, ScenarioRepository,
};

pub const WORKSTATION_SCHEMA_VERSION: &str = "0.6.0";

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
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkstationActionKind {
    Terminal,
    Browser,
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
        if scenario.config.source != source
            || scenario.config.security.is_some()
            || !scenario.config.connection_overrides.is_empty()
            || !scenario.config.first_hop_overrides.is_empty()
            || scenario.config.recovery.is_some()
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

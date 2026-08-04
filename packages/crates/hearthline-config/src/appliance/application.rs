use std::collections::BTreeSet;
use std::fmt;
use std::net::Ipv4Addr;

use hearthline_engine::HttpInspectionTarget;
use hearthline_model::{ComponentId, HttpMethod, Text};
use serde::Deserialize;

use super::{ApplicationUpstreamConfig, ConfigError, ListenerConfig, RouteConfig, require_text};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnsRecordConfig {
    pub name: String,
    pub address: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum HttpMethodConfig {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
}

impl HttpMethodConfig {
    pub(crate) const fn runtime(self) -> HttpMethod {
        match self {
            Self::Get => HttpMethod::Get,
            Self::Head => HttpMethod::Head,
            Self::Post => HttpMethod::Post,
            Self::Put => HttpMethod::Put,
            Self::Patch => HttpMethod::Patch,
            Self::Delete => HttpMethod::Delete,
            Self::Options => HttpMethod::Options,
        }
    }
}

impl fmt::Display for HttpMethodConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Get => "GET",
            Self::Head => "HEAD",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum HttpInspectionTargetConfig {
    Path,
    Body,
}

impl HttpInspectionTargetConfig {
    pub(crate) const fn runtime(self) -> HttpInspectionTarget {
        match self {
            Self::Path => HttpInspectionTarget::Path,
            Self::Body => HttpInspectionTarget::Body,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpInspectionRuleConfig {
    pub id: String,
    pub target: HttpInspectionTargetConfig,
    pub contains: String,
    #[serde(default = "default_true")]
    pub case_sensitive: bool,
    pub reason: String,
}

pub(super) fn join_http_methods(methods: &[HttpMethodConfig]) -> String {
    methods
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn application_gateway_facts(
    listeners: &[ListenerConfig],
    allowed_hosts: &[String],
    allowed_methods: &[HttpMethodConfig],
    inspection_rules: &[HttpInspectionRuleConfig],
    upstreams: &[ApplicationUpstreamConfig],
    routes: &[RouteConfig],
    max_request_bytes: Option<u64>,
) -> Vec<String> {
    vec![
        format!("Listeners: {}", listeners.len()),
        format!("Allowed hosts: {}", allowed_hosts.join(", ")),
        format!("Allowed methods: {}", join_http_methods(allowed_methods)),
        format!("Inspection rules: {}", inspection_rules.len()),
        format!(
            "Upstreams: {}",
            upstreams
                .iter()
                .map(|upstream| format!("{} ({})", upstream.id, upstream.address))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!("Routes: {}", routes.len()),
        format!(
            "Request limit: {}",
            max_request_bytes.map_or_else(|| "not set".into(), |value| value.to_string())
        ),
    ]
}

pub(super) fn validate_application_gateway(
    appliance_id: &str,
    listeners: &[ListenerConfig],
    allowed_hosts: &[String],
    allowed_methods: &[HttpMethodConfig],
    upstreams: &[ApplicationUpstreamConfig],
    inspection_rules: &[HttpInspectionRuleConfig],
) -> Result<(), ConfigError> {
    if listeners.is_empty()
        || allowed_hosts.is_empty()
        || allowed_methods.is_empty()
        || upstreams.is_empty()
        || inspection_rules.is_empty()
    {
        return Err(ConfigError::new(format!(
            "application gateway {appliance_id} requires listeners, allowed hosts, allowed methods, upstreams, and inspection rules"
        )));
    }
    if inspection_rules.len() > 16 {
        return Err(ConfigError::new(format!(
            "application gateway {appliance_id} exceeds 16 inspection rules"
        )));
    }
    let mut ids = BTreeSet::new();
    let mut signatures = BTreeSet::new();
    for rule in inspection_rules {
        ComponentId::new(&rule.id).map_err(|error| ConfigError::new(error.to_string()))?;
        require_text("HTTP inspection pattern", &rule.contains)?;
        require_text("HTTP inspection reason", &rule.reason)?;
        Text::<96>::try_new(&rule.contains).map_err(|_| {
            ConfigError::new(format!(
                "application gateway {appliance_id} inspection rule {} pattern exceeds 96 bytes",
                rule.id
            ))
        })?;
        Text::<96>::try_new(&rule.reason).map_err(|_| {
            ConfigError::new(format!(
                "application gateway {appliance_id} inspection rule {} reason exceeds 96 bytes",
                rule.id
            ))
        })?;
        if !ids.insert(&rule.id) {
            return Err(ConfigError::new(format!(
                "application gateway {appliance_id} repeats inspection rule {}",
                rule.id
            )));
        }
        let signature = (
            rule.target,
            rule.contains.to_ascii_lowercase(),
            rule.case_sensitive,
        );
        if !signatures.insert(signature) {
            return Err(ConfigError::new(format!(
                "application gateway {appliance_id} repeats an inspection signature"
            )));
        }
    }
    Ok(())
}

pub(super) fn validate_dns_records(
    appliance_id: &str,
    records: &[DnsRecordConfig],
) -> Result<(), ConfigError> {
    let mut names = BTreeSet::new();
    for record in records {
        let name = record.name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err(ConfigError::new(format!(
                "DNS appliance {appliance_id} has an empty record name"
            )));
        }
        if !names.insert(name) {
            return Err(ConfigError::new(format!(
                "DNS appliance {appliance_id} repeats record {}",
                record.name
            )));
        }
        record.address.parse::<Ipv4Addr>().map_err(|_| {
            ConfigError::new(format!(
                "DNS appliance {appliance_id} record {} has invalid address {}",
                record.name, record.address
            ))
        })?;
    }
    Ok(())
}

const fn default_true() -> bool {
    true
}

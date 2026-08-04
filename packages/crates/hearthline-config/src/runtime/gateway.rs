use hearthline_engine::{HttpInspectionRule, ReverseProxyWaf};
use hearthline_model::{ComponentId, Text};

use crate::appliance::{ApplianceConfig, BehaviorConfig, ConfigError};

use super::ConfiguredAppliance;
use super::builder::{parse_ipv4, routed_interfaces, runtime_routes};

pub(super) fn build_web_gateway(
    id: ComponentId,
    config: &ApplianceConfig,
) -> Result<ConfiguredAppliance, ConfigError> {
    let BehaviorConfig::ApplicationGateway {
        listeners,
        allowed_hosts,
        allowed_methods,
        inspection_rules,
        upstreams,
        routes,
        max_request_bytes,
    } = &config.behavior
    else {
        return Err(ConfigError::new(format!(
            "appliance {} does not define application-gateway behavior",
            config.id
        )));
    };
    if !listeners
        .iter()
        .any(|listener| listener.protocol == "https" && listener.port == 443)
    {
        return Err(ConfigError::new(format!(
            "application gateway {} requires an HTTPS listener on TCP 443",
            config.id
        )));
    }
    if upstreams.len() != 1 {
        return Err(ConfigError::new(format!(
            "application gateway {} currently requires exactly one upstream",
            config.id
        )));
    }
    let hosts = allowed_hosts
        .iter()
        .map(|host| Text::<128>::try_new(host).map_err(|error| ConfigError::new(error.to_string())))
        .collect::<Result<Vec<_>, _>>()?;
    let upstream =
        ComponentId::new(&upstreams[0].id).map_err(|error| ConfigError::new(error.to_string()))?;
    let upstream_address = parse_ipv4(&upstreams[0].address, "application upstream address")?;
    let interfaces = routed_interfaces(config)?;
    let default_gateway = config
        .default_gateway
        .as_deref()
        .map(|gateway| parse_ipv4(gateway, "default gateway"))
        .transpose()?;
    let mut appliance = ReverseProxyWaf::with_routes(
        id,
        interfaces,
        default_gateway,
        runtime_routes(routes)?,
        hosts,
        upstream,
        upstream_address,
    );
    appliance.set_allowed_methods(allowed_methods.iter().map(|method| method.runtime()));
    appliance.set_inspection_rules(
        inspection_rules
            .iter()
            .map(|rule| {
                Ok(HttpInspectionRule::new(
                    rule.target.runtime(),
                    Text::try_new(&rule.contains)
                        .map_err(|error| ConfigError::new(error.to_string()))?,
                    rule.case_sensitive,
                    Text::try_new(&rule.reason)
                        .map_err(|error| ConfigError::new(error.to_string()))?,
                ))
            })
            .collect::<Result<Vec<_>, ConfigError>>()?,
    );
    if let Some(limit) = *max_request_bytes {
        appliance.set_maximum_body_bytes(
            usize::try_from(limit)
                .map_err(|_| ConfigError::new("gateway request limit exceeds runtime capacity"))?,
        );
    }
    Ok(ConfiguredAppliance::WebGateway(Box::new(appliance)))
}

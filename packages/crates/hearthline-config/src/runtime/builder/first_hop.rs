use std::net::Ipv4Addr;

use hearthline_engine::{FirstHopAddress, RoutedInterface};
use hearthline_model::MacAddress;

use crate::{ConfigError, InterfaceConfig};

pub(super) fn configure_first_hop(
    runtime: &mut RoutedInterface,
    appliance_id: &str,
    interface: &InterfaceConfig,
) -> Result<(), ConfigError> {
    let Some(first_hop) = &interface.first_hop else {
        return Ok(());
    };
    let virtual_ip = first_hop
        .virtual_ip
        .parse::<Ipv4Addr>()
        .map_err(|_| ConfigError::new("invalid first-hop virtual address"))?;
    let virtual_mac = first_hop
        .virtual_mac
        .parse::<MacAddress>()
        .map_err(|error| ConfigError::new(error.to_string()))?;
    runtime
        .add_first_hop_address(FirstHopAddress::new(
            virtual_ip,
            virtual_mac,
            first_hop.initial_role.is_active(),
        ))
        .map_err(|error| {
            ConfigError::new(format!(
                "appliance {appliance_id} interface {}: {error}",
                interface.id
            ))
        })
}

use std::collections::BTreeMap;

use super::LoadedAppliance;
use crate::ConfigError;

pub(super) fn validate_spanning_tree_bridges(
    appliances: &BTreeMap<String, LoadedAppliance>,
) -> Result<(), ConfigError> {
    let mut bridge_macs = BTreeMap::new();
    for appliance in appliances.values() {
        let Some(spanning_tree) = &appliance.config.spanning_tree else {
            continue;
        };
        if let Some(existing) = bridge_macs.insert(
            spanning_tree.bridge_mac.as_str(),
            appliance.config.id.as_str(),
        ) {
            return Err(ConfigError::new(format!(
                "spanning-tree bridge MAC {} is shared by {existing} and {}",
                spanning_tree.bridge_mac, appliance.config.id
            )));
        }
    }
    Ok(())
}

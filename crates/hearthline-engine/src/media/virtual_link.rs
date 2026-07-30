use serde::Deserialize;

use super::SimulatedMedium;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualMedium {
    pub technology: String,
}

impl SimulatedMedium for VirtualMedium {
    fn validate(&self) -> Result<(), String> {
        if self.technology.trim().is_empty() {
            Err("virtual-link technology cannot be empty".into())
        } else {
            Ok(())
        }
    }

    fn detail(&self) -> String {
        self.technology.clone()
    }

    fn physical_facts(&self) -> Vec<String> {
        vec![
            self.technology.clone(),
            "Logical attachment without a dedicated physical cable".into(),
            "Host scheduling and virtual-switch contention are not yet modeled".into(),
        ]
    }

    fn propagation_delay_us(&self) -> u64 {
        0
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        None
    }
}

use serde::Deserialize;

use super::SimulatedMedium;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarrierMedium {
    pub service: String,
}

impl SimulatedMedium for CarrierMedium {
    fn validate(&self) -> Result<(), String> {
        if self.service.trim().is_empty() {
            Err("carrier service cannot be empty".into())
        } else {
            Ok(())
        }
    }

    fn detail(&self) -> String {
        self.service.clone()
    }

    fn physical_facts(&self) -> Vec<String> {
        vec![
            self.service.clone(),
            "Provider underlay is abstracted".into(),
            "Configured latency represents the contracted access path".into(),
        ]
    }

    fn propagation_delay_us(&self) -> u64 {
        0
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        None
    }
}

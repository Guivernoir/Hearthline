use hearthline_model::Text;
use serde::Deserialize;

use super::{MediaError, MediaFacts, MediaText, SimulatedMedium, facts};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarrierMedium {
    pub service: Text<96>,
}

impl SimulatedMedium for CarrierMedium {
    fn validate(&self) -> Result<(), MediaError> {
        if self.service.trim().is_empty() {
            Err("carrier service cannot be empty".into())
        } else {
            Ok(())
        }
    }

    fn detail(&self) -> MediaText {
        Text::from(self.service.as_str())
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            Text::from(self.service.as_str()),
            "Provider underlay is abstracted".into(),
            "Configured latency represents the contracted access path".into(),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        0
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        None
    }
}

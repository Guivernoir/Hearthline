use hearthline_model::Text;
use serde::Deserialize;

use super::{MediaError, MediaFacts, MediaText, SimulatedMedium, facts};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualMedium {
    pub technology: Text<64>,
}

impl SimulatedMedium for VirtualMedium {
    fn validate(&self) -> Result<(), MediaError> {
        if self.technology.trim().is_empty() {
            Err("virtual-link technology cannot be empty".into())
        } else {
            Ok(())
        }
    }

    fn detail(&self) -> MediaText {
        Text::from(self.technology.as_str())
    }

    fn physical_facts(&self) -> MediaFacts {
        facts([
            Text::from(self.technology.as_str()),
            "Logical attachment without a dedicated physical cable".into(),
            "Host scheduling and virtual-switch contention are not yet modeled".into(),
        ])
    }

    fn propagation_delay_us(&self) -> u64 {
        0
    }

    fn max_capacity_mbps(&self) -> Option<u64> {
        None
    }
}

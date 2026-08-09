use super::{LearningSwitch, MacTableEntry};

impl LearningSwitch {
    pub fn active_mac_table(&self, now_us: u64) -> impl Iterator<Item = (&MacTableEntry, u64)> {
        let aging_time_us = self.aging_time_us;
        self.forwarding_table.iter().filter_map(move |entry| {
            let age_us = now_us.saturating_sub(entry.last_seen_us);
            (age_us < aging_time_us).then_some((entry, aging_time_us.saturating_sub(age_us)))
        })
    }
}

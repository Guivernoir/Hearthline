use std::collections::BTreeMap;

use super::super::{
    HmiAlarm, HmiAuditEntry, HmiSignal, HmiSupervisoryAsset, HmiSupervisoryEvent,
    HmiSupervisoryIdentity, HmiSupervisoryNode, HmiSupervisoryRepository, HmiSupervisorySample,
    HmiSupervisoryState, HmiSupervisoryTag, HmiSupervisoryTemplate,
};
use crate::SupervisoryProfileConfig;

#[derive(Clone, Debug)]
pub(in crate::hmi) struct SupervisoryRuntime {
    profile: SupervisoryProfileConfig,
    elapsed_ms: u64,
    sample_elapsed_ms: u64,
    samples: BTreeMap<String, Vec<HmiSupervisorySample>>,
}

impl SupervisoryRuntime {
    pub(in crate::hmi) fn new(profile: SupervisoryProfileConfig) -> Self {
        let samples = profile
            .history
            .tags
            .iter()
            .map(|tag| (tag.clone(), Vec::new()))
            .collect();
        Self {
            profile,
            elapsed_ms: 0,
            sample_elapsed_ms: 0,
            samples,
        }
    }

    pub(in crate::hmi) fn tick(&mut self, elapsed_ms: u64, signals: &[HmiSignal]) {
        self.elapsed_ms = self.elapsed_ms.saturating_add(elapsed_ms);
        self.sample_elapsed_ms = self.sample_elapsed_ms.saturating_add(elapsed_ms);
        if elapsed_ms == 0 || self.sample_elapsed_ms < self.profile.history.sample_interval_ms {
            return;
        }
        self.sample_elapsed_ms %= self.profile.history.sample_interval_ms;
        for (tag, samples) in &mut self.samples {
            let Some(signal) = signals.iter().find(|signal| signal.tag == *tag) else {
                continue;
            };
            samples.push(HmiSupervisorySample {
                timestamp_ms: self.elapsed_ms,
                value: signal.value,
                quality_good: signal.quality_good,
            });
            if samples.len() > self.profile.history.capacity {
                samples.remove(0);
            }
        }
    }

    pub(in crate::hmi) fn snapshot(
        &self,
        signals: &[HmiSignal],
        alarms: &[HmiAlarm],
        audit: &[HmiAuditEntry],
    ) -> HmiSupervisoryState {
        let identity = &self.profile.identity;
        let permissions = self
            .profile
            .roles
            .iter()
            .find(|role| role.id == identity.role)
            .map(|role| role.permissions.clone())
            .unwrap_or_default();
        HmiSupervisoryState {
            namespace: self.profile.namespace.clone(),
            model_id: self.profile.model_id.clone(),
            repository: HmiSupervisoryRepository {
                id: self.profile.repository.id.clone(),
                revision: self.profile.repository.revision.clone(),
                deployed_revision: self.profile.repository.deployed_revision.clone(),
                synchronized: self.profile.repository.revision
                    == self.profile.repository.deployed_revision,
            },
            templates: self.templates(),
            assets: self.assets(),
            deployment_nodes: self.nodes(),
            identity: HmiSupervisoryIdentity {
                user: identity.user.clone(),
                role: identity.role.clone(),
                authentication: identity.authentication.clone(),
                permissions,
            },
            tags: self.tags(signals),
            events: events(alarms, audit),
        }
    }

    fn templates(&self) -> Vec<HmiSupervisoryTemplate> {
        self.profile
            .templates
            .iter()
            .map(|template| HmiSupervisoryTemplate {
                id: template.id.clone(),
                label: template.label.clone(),
                parent: template.parent.clone(),
                attributes: template.attributes.clone(),
                alarm_capable: template.alarm_capable,
                history_capable: template.history_capable,
            })
            .collect()
    }

    fn assets(&self) -> Vec<HmiSupervisoryAsset> {
        self.profile
            .assets
            .iter()
            .map(|asset| HmiSupervisoryAsset {
                id: asset.id.clone(),
                label: asset.label.clone(),
                template: asset.template.clone(),
                parent: asset.parent.clone(),
                components: asset.components.clone(),
                historized_tags: asset.historized_tags.clone(),
            })
            .collect()
    }

    fn nodes(&self) -> Vec<HmiSupervisoryNode> {
        self.profile
            .deployment_nodes
            .iter()
            .map(|node| HmiSupervisoryNode {
                id: node.id.clone(),
                label: node.label.clone(),
                host: node.host.clone(),
                role: node.role.to_string(),
                state: node.state.to_string(),
                redundancy_group: node.redundancy_group.clone(),
            })
            .collect()
    }

    fn tags(&self, signals: &[HmiSignal]) -> Vec<HmiSupervisoryTag> {
        self.profile
            .history
            .tags
            .iter()
            .filter_map(|tag| {
                let signal = signals.iter().find(|signal| signal.tag == *tag)?;
                Some(HmiSupervisoryTag {
                    tag: tag.clone(),
                    value: signal.value,
                    unit: signal.unit.clone(),
                    quality: if signal.quality_good { "good" } else { "bad" },
                    timestamp_ms: signal.timestamp_ms,
                    samples: self.samples.get(tag).cloned().unwrap_or_default(),
                })
            })
            .collect()
    }
}

fn events(alarms: &[HmiAlarm], audit: &[HmiAuditEntry]) -> Vec<HmiSupervisoryEvent> {
    let mut events = alarms
        .iter()
        .map(|alarm| HmiSupervisoryEvent {
            sequence: alarm.sequence,
            category: "alarm",
            source: alarm.source.clone(),
            message: alarm.message.clone(),
            state: if alarm.active { "active" } else { "returned" }.into(),
        })
        .chain(audit.iter().map(|entry| HmiSupervisoryEvent {
            sequence: entry.sequence,
            category: "operator-audit",
            source: entry.target.clone(),
            message: entry.action.clone(),
            state: entry.result.clone(),
        }))
        .collect::<Vec<_>>();
    events.sort_by_key(|event| event.sequence);
    events
}

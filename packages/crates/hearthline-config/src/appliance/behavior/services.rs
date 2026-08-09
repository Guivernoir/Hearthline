use super::BehaviorConfig;

impl BehaviorConfig {
    pub(in crate::appliance) fn services(&self) -> Vec<String> {
        match self {
            Self::Endpoint {
                accepted_services, ..
            }
            | Self::ServiceHost {
                accepted_services, ..
            }
            | Self::PolicyService {
                accepted_services, ..
            }
            | Self::Voice {
                accepted_services, ..
            }
            | Self::ComputeHost {
                accepted_services, ..
            } => accepted_services.clone(),
            Self::ApplicationGateway { listeners, .. } => listeners
                .iter()
                .map(|listener| format!("{}:{}", listener.protocol, listener.port))
                .collect(),
            _ => Vec::new(),
        }
    }
}

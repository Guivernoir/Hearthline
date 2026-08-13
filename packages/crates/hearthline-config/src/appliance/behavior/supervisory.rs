use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupervisoryNodeRoleConfig {
    EngineeringRepository,
    ApplicationRuntime,
    Historian,
    OperatorClient,
}

impl std::fmt::Display for SupervisoryNodeRoleConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::EngineeringRepository => "engineering-repository",
            Self::ApplicationRuntime => "application-runtime",
            Self::Historian => "historian",
            Self::OperatorClient => "operator-client",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupervisoryNodeStateConfig {
    Active,
    Standby,
    Online,
}

impl std::fmt::Display for SupervisoryNodeStateConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Active => "active",
            Self::Standby => "standby",
            Self::Online => "online",
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisoryRepositoryConfig {
    pub id: String,
    pub engineering_node: String,
    pub revision: String,
    pub deployed_revision: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisoryTemplateConfig {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub attributes: Vec<String>,
    #[serde(default)]
    pub alarm_capable: bool,
    #[serde(default)]
    pub history_capable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisoryAssetConfig {
    pub id: String,
    pub label: String,
    pub template: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub components: Vec<String>,
    #[serde(default)]
    pub historized_tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisoryDeploymentNodeConfig {
    pub id: String,
    pub label: String,
    pub host: String,
    pub role: SupervisoryNodeRoleConfig,
    pub state: SupervisoryNodeStateConfig,
    #[serde(default)]
    pub redundancy_group: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisoryRoleConfig {
    pub id: String,
    pub label: String,
    pub permissions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisoryIdentityConfig {
    pub user: String,
    pub role: String,
    pub authentication: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisoryHistoryConfig {
    pub sample_interval_ms: u64,
    pub capacity: usize,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupervisoryProfileConfig {
    pub namespace: String,
    pub model_id: String,
    pub repository: SupervisoryRepositoryConfig,
    pub templates: Vec<SupervisoryTemplateConfig>,
    pub assets: Vec<SupervisoryAssetConfig>,
    pub deployment_nodes: Vec<SupervisoryDeploymentNodeConfig>,
    pub roles: Vec<SupervisoryRoleConfig>,
    pub identity: SupervisoryIdentityConfig,
    pub history: SupervisoryHistoryConfig,
}

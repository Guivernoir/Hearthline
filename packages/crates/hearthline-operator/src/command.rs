use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum OperatorCommand {
    Start {
        target: String,
    },
    Stop {
        target: String,
    },
    Reset {
        target: String,
    },
    SetMode {
        target: String,
        mode: String,
    },
    SetParameter {
        target: String,
        parameter: String,
        raw_value: i64,
    },
    ManualOutput {
        target: String,
        output: String,
        active: bool,
    },
}

impl OperatorCommand {
    pub fn target(&self) -> &str {
        match self {
            Self::Start { target }
            | Self::Stop { target }
            | Self::Reset { target }
            | Self::SetMode { target, .. }
            | Self::SetParameter { target, .. }
            | Self::ManualOutput { target, .. } => target,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorCommandEnvelope {
    pub sequence: u64,
    pub submitted_at_us: u64,
    pub operator: String,
    pub model_revision: String,
    pub command: OperatorCommand,
    #[serde(default)]
    pub context: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperatorCommandError {
    StaleRevision { expected: String, actual: String },
    QueueFull { limit: usize },
    SequenceExhausted,
    PermissionDenied(String),
}

impl Display for OperatorCommandError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleRevision { expected, actual } => write!(
                formatter,
                "operator model revision {expected} is stale; current revision is {actual}"
            ),
            Self::QueueFull { limit } => write!(
                formatter,
                "operator command queue reached its {limit}-command limit"
            ),
            Self::SequenceExhausted => formatter.write_str("operator command sequence exhausted"),
            Self::PermissionDenied(permission) => {
                write!(formatter, "operator lacks permission {permission}")
            }
        }
    }
}

impl std::error::Error for OperatorCommandError {}

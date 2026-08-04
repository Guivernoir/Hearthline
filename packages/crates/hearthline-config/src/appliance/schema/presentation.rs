use std::fmt::{self, Display, Formatter};

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    #[default]
    Design,
    Configured,
    Simulated,
}

impl Display for Lifecycle {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Design => "design",
            Self::Configured => "configured",
            Self::Simulated => "simulated",
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub enum RenderMode {
    #[default]
    Any,
    Physical,
    Logical,
}

impl Display for RenderMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Any => "any",
            Self::Physical => "physical",
            Self::Logical => "logical",
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderBinding {
    pub view: String,
    pub node: String,
    #[serde(default)]
    pub mode: RenderMode,
}

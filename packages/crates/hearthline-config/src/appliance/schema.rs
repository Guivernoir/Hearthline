use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};

use hearthline_model::{BehaviorFamily, ComponentId, ComponentKind};
use serde::Deserialize;

use hearthline_engine::{
    PortHardwareKind, PortSettings, PortState, PortStateConfig, appliance_supports_port,
};

use super::{BehaviorConfig, ConfigError, deserialize_component_kind, require_text};

pub const APPLIANCE_SCHEMA_VERSION: &str = "0.3.0";
pub const FRONTEND_CATALOG_SCHEMA_VERSION: &str = "0.3.0";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplianceConfig {
    pub schema_version: String,
    pub id: String,
    pub label: String,
    #[serde(deserialize_with = "deserialize_component_kind")]
    pub kind: ComponentKind,
    pub site: String,
    pub environment: String,
    pub zone: String,
    pub role: String,
    pub summary: String,
    #[serde(default)]
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub render: Vec<RenderBinding>,
    #[serde(default)]
    pub interfaces: Vec<InterfaceConfig>,
    pub behavior: BehaviorConfig,
}

impl ApplianceConfig {
    pub fn from_yaml(source: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml_ng::from_str(source)
            .map_err(|error| ConfigError::new(format!("invalid YAML: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != APPLIANCE_SCHEMA_VERSION {
            return Err(ConfigError::new(format!(
                "appliance {} uses schema {}, expected {}",
                self.id, self.schema_version, APPLIANCE_SCHEMA_VERSION
            )));
        }
        ComponentId::new(&self.id).map_err(|error| ConfigError::new(error.to_string()))?;
        require_text("label", &self.label)?;
        require_text("site", &self.site)?;
        require_text("environment", &self.environment)?;
        require_text("zone", &self.zone)?;
        require_text("role", &self.role)?;
        require_text("summary", &self.summary)?;

        let expected_family = self.kind.behavior_family();
        let configured_family = self.behavior.family();
        if expected_family != configured_family {
            return Err(ConfigError::new(format!(
                "appliance {} kind {} requires behavior family {}, not {}",
                self.id, self.kind, expected_family, configured_family
            )));
        }

        let mut interface_ids = BTreeSet::new();
        for interface in &self.interfaces {
            ComponentId::new(&interface.id).map_err(|error| {
                ConfigError::new(format!(
                    "appliance {} has invalid interface id: {error}",
                    self.id
                ))
            })?;
            if !interface_ids.insert(&interface.id) {
                return Err(ConfigError::new(format!(
                    "appliance {} repeats interface {}",
                    self.id, interface.id
                )));
            }
            if !appliance_supports_port(self.kind, interface.hardware) {
                return Err(ConfigError::new(format!(
                    "appliance {} kind {} does not support {} port {}",
                    self.id, self.kind, interface.hardware, interface.id
                )));
            }
            if interface.state.administrative == PortState::Down
                && interface.state.initial_operational == PortState::Up
            {
                return Err(ConfigError::new(format!(
                    "appliance {} port {} cannot be operationally up while administratively down",
                    self.id, interface.id
                )));
            }
            interface.settings.validate().map_err(|error| {
                ConfigError::new(format!(
                    "appliance {} port {}: {error}",
                    self.id, interface.id
                ))
            })?;
        }

        let mut bindings = BTreeSet::new();
        for binding in &self.render {
            require_text("render.view", &binding.view)?;
            require_text("render.node", &binding.node)?;
            if !bindings.insert((&binding.view, &binding.node, binding.mode)) {
                return Err(ConfigError::new(format!(
                    "appliance {} repeats render binding {}:{}:{:?}",
                    self.id, binding.view, binding.node, binding.mode
                )));
            }
        }

        self.behavior.validate(&self.id)
    }

    pub fn behavior_family(&self) -> BehaviorFamily {
        self.behavior.family()
    }
}

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
        let value = match self {
            Self::Design => "design",
            Self::Configured => "configured",
            Self::Simulated => "simulated",
        };
        formatter.write_str(value)
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
        let value = match self {
            Self::Any => "any",
            Self::Physical => "physical",
            Self::Logical => "logical",
        };
        formatter.write_str(value)
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

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceConfig {
    pub id: String,
    pub hardware: PortHardwareKind,
    pub state: PortStateConfig,
    pub settings: PortSettings,
    pub mode: InterfaceMode,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub vlans: Vec<u16>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InterfaceMode {
    Access,
    Trunk,
    Routed,
    Transparent,
    Management,
    Monitor,
    FieldIo,
}

impl Display for InterfaceMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Access => "access",
            Self::Trunk => "trunk",
            Self::Routed => "routed",
            Self::Transparent => "transparent",
            Self::Management => "management",
            Self::Monitor => "monitor",
            Self::FieldIo => "field-io",
        };
        formatter.write_str(value)
    }
}

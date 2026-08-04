mod application;
mod behavior;
mod frontend;
mod repository;
mod schema;
mod support;

pub use application::{
    DnsRecordConfig, HttpInspectionRuleConfig, HttpInspectionTargetConfig, HttpMethodConfig,
};
pub use behavior::BehaviorConfig;
pub use frontend::{
    FrontendAppliance, FrontendApplianceCatalog, FrontendFirewallHa, FrontendFirstHop,
    FrontendInterface, FrontendLinkAggregation, FrontendLinkAggregationGroup, FrontendMultiChassis,
    FrontendSpanningTree,
};
pub use repository::{ConfigRepository, LoadedAppliance};
pub use schema::{
    APPLIANCE_SCHEMA_VERSION, ApplianceConfig, ApplicationUpstreamConfig,
    FRONTEND_CATALOG_SCHEMA_VERSION, FirewallHaConfig, FirewallHaRole, FirewallZoneConfig,
    FirstHopConfig, FirstHopProtocol, FirstHopRole, HttpSiteConfig, InterfaceConfig, InterfaceMode,
    Lifecycle, LinkAggregationConfig, LinkAggregationGroupConfig, LinkAggregationMode,
    LinkAggregationProtocol, ListenerConfig, MultiChassisConfig, MultiChassisRole,
    NatTranslationConfig, PolicyAction, PolicyRuleConfig, RenderBinding, RenderMode, RouteConfig,
    SpanningTreeConfig, SpanningTreeProtocol,
};
pub use support::ConfigError;
pub(crate) use support::source_revision;

use application::{application_gateway_facts, validate_application_gateway, validate_dns_records};
use support::{
    collect_yaml_paths, default_true, deserialize_component_kind, join_numbers, require_text,
};

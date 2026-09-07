/// Evidence used to justify a fixed-capacity runtime resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityEvidence {
    CanonicalConfiguration,
    ExecutableScenario,
    ProtocolContract,
    AlgorithmicBound,
}

/// Required behavior when a bounded resource cannot accept more work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaturationBehavior {
    RejectConfiguration,
    RejectOperation,
    DropWithDiagnostic,
    StopSimulation,
}

/// One fixed-capacity runtime resource and its supported operating load.
///
/// `nominal_load` is a reviewed design limit, not a convenient fraction of
/// capacity. The evidence category identifies how that limit is verified.
#[derive(Debug, Eq, PartialEq)]
pub struct CapacityBudget {
    pub resource: &'static str,
    pub capacity: usize,
    pub nominal_load: usize,
    pub evidence: CapacityEvidence,
    pub saturation: SaturationBehavior,
    pub rationale: &'static str,
}

impl CapacityBudget {
    const fn new(
        resource: &'static str,
        capacity: usize,
        nominal_load: usize,
        evidence: CapacityEvidence,
        saturation: SaturationBehavior,
        rationale: &'static str,
    ) -> Self {
        Self {
            resource,
            capacity,
            nominal_load,
            evidence,
            saturation,
            rationale,
        }
    }

    const fn configured(resource: &'static str, capacity: usize, nominal_load: usize) -> Self {
        Self::new(
            resource,
            capacity,
            nominal_load,
            CapacityEvidence::CanonicalConfiguration,
            SaturationBehavior::RejectConfiguration,
            "measured from the validated canonical project configuration",
        )
    }

    const fn scenario(resource: &'static str, capacity: usize, nominal_load: usize) -> Self {
        Self::new(
            resource,
            capacity,
            nominal_load,
            CapacityEvidence::ExecutableScenario,
            SaturationBehavior::StopSimulation,
            "measured by deterministic positive, failure, and overload scenarios",
        )
    }

    const fn protocol(resource: &'static str, capacity: usize, nominal_load: usize) -> Self {
        Self::new(
            resource,
            capacity,
            nominal_load,
            CapacityEvidence::ProtocolContract,
            SaturationBehavior::DropWithDiagnostic,
            "bounded by the modeled protocol or component contract",
        )
    }

    const fn algorithmic(resource: &'static str, capacity: usize, nominal_load: usize) -> Self {
        Self::new(
            resource,
            capacity,
            nominal_load,
            CapacityEvidence::AlgorithmicBound,
            SaturationBehavior::RejectOperation,
            "bounded by one deterministic algorithmic operation",
        )
    }

    pub const fn reserve(&self) -> usize {
        self.capacity - self.nominal_load
    }

    pub const fn has_required_headroom(&self) -> bool {
        self.nominal_load > 0
            && self.nominal_load <= self.capacity
            && self.nominal_load.saturating_mul(4) <= self.capacity.saturating_mul(3)
            && !self.rationale.is_empty()
    }

    pub const fn accepts_design_load(&self, observed: usize) -> bool {
        observed <= self.nominal_load
    }
}

pub(crate) const MEDIA_FACT_CAPACITY: usize = 6;
pub(crate) const PROCESS_PORT_CAPACITY: usize = 36;
pub(crate) const PROCESS_TAG_CAPACITY: usize = 36;
pub(crate) const PLC_RULE_CAPACITY: usize = 16;
pub(crate) const HMI_COMMAND_CAPACITY: usize = 24;
pub(crate) const SAFETY_PERMISSIVE_CAPACITY: usize = 16;
pub const SEQUENCE_STEP_CAPACITY: usize = 24;
pub const SEQUENCE_OUTPUT_CAPACITY: usize = 16;
pub const ROBOT_CELL_QUEUE_CAPACITY: usize = 8;
pub const ROBOT_PROGRAM_CAPACITY: usize = 128;

pub(crate) const FIREWALL_HA_PORT_CAPACITY: usize = 4;
pub(crate) const FIREWALL_ZONE_CAPACITY: usize = 16;
pub(crate) const FIREWALL_RULE_CAPACITY: usize = 16;
pub(crate) const FIREWALL_SESSION_CAPACITY: usize = 128;
pub(crate) const FIREWALL_INTERFACE_CAPACITY: usize = 16;
pub(crate) const INTERFACE_ADDRESS_CAPACITY: usize = 4;
pub(crate) const FIRST_HOP_CAPACITY: usize = 4;
pub(crate) const FORWARDING_INTERFACE_CAPACITY: usize = 16;
pub(crate) const FORWARDING_PENDING_CAPACITY: usize = 16;
pub(crate) const PROXY_ADDRESS_CAPACITY: usize = 16;
pub(crate) const NEIGHBOR_CAPACITY: usize = 32;
pub(crate) const ROUTER_ROUTE_CAPACITY: usize = 16;

pub(crate) const HOST_SERVICE_CAPACITY: usize = 16;
pub(crate) const DNS_RECORD_CAPACITY: usize = 8;
pub(crate) const HOST_INTERFACE_CAPACITY: usize = 16;
pub(crate) const UNADDRESSED_PORT_CAPACITY: usize = 24;
pub(crate) const HOST_PENDING_CAPACITY: usize = 16;
pub(crate) const HOST_ROUTE_CAPACITY: usize = 16;
pub(crate) const MONITOR_PORT_CAPACITY: usize = 16;
pub(crate) const GATEWAY_HOST_CAPACITY: usize = 8;
pub(crate) const GATEWAY_METHOD_CAPACITY: usize = 8;
pub(crate) const GATEWAY_RULE_CAPACITY: usize = 16;
pub(crate) const GATEWAY_PENDING_CAPACITY: usize = 8;
pub(crate) const LINK_APPLIANCE_PORT_CAPACITY: usize = 36;
pub(crate) const NAT_INSIDE_PORT_CAPACITY: usize = 16;
pub(crate) const NAT_STATIC_CAPACITY: usize = 16;
pub(crate) const NAT_PAT_CAPACITY: usize = 64;
pub(crate) const NAT_INTERFACE_CAPACITY: usize = 16;

pub(crate) const SWITCH_VLAN_CAPACITY: usize = 32;
pub(crate) const SWITCH_PORT_CAPACITY: usize = 24;
pub(crate) const SWITCH_FORWARDING_CAPACITY: usize = 64;
pub(crate) const SWITCH_AGGREGATION_CAPACITY: usize = 16;
pub(crate) const SWITCH_AGGREGATION_MEMBER_CAPACITY: usize = 16;
pub(crate) const WIRELESS_CLIENT_CAPACITY: usize = 16;
pub(crate) const SWITCH_EGRESS_CAPACITY: usize = 16;
pub(crate) const LAYER3_SWITCH_PORT_CAPACITY: usize = 24;

pub(crate) const SIMULATOR_COMPONENT_CAPACITY: usize = 192;
pub(crate) const SIMULATOR_LINK_CAPACITY: usize = 256;
pub(crate) const SIMULATOR_IMMEDIATE_CAPACITY: usize = 64;
pub(crate) const SIMULATOR_DELAYED_CAPACITY: usize = 64;
pub(crate) const SIMULATOR_TRACE_CAPACITY: usize = 224;
pub(crate) const SIMULATOR_SHARED_MEDIA_CAPACITY: usize = 32;
pub const EFFECT_CAPACITY: usize = 32;

/// Capacity ledger for every concrete bounded collection in the engine.
///
/// Generic caller-sized collections, currently `HistorianBuffer`, are covered
/// by type-level validity and saturation tests instead of a global budget.
pub const RUNTIME_CAPACITY_BUDGETS: &[CapacityBudget] = &[
    CapacityBudget::algorithmic("physical.media-facts", MEDIA_FACT_CAPACITY, 4),
    CapacityBudget::configured("industrial.process-ports", PROCESS_PORT_CAPACITY, 27),
    CapacityBudget::configured("industrial.process-tags", PROCESS_TAG_CAPACITY, 27),
    CapacityBudget::protocol("industrial.plc-rules", PLC_RULE_CAPACITY, 12),
    CapacityBudget::configured("industrial.hmi-command-tags", HMI_COMMAND_CAPACITY, 18),
    CapacityBudget::protocol(
        "industrial.safety-permissives",
        SAFETY_PERMISSIVE_CAPACITY,
        12,
    ),
    CapacityBudget::protocol("industrial.sequence-steps", SEQUENCE_STEP_CAPACITY, 18),
    CapacityBudget::protocol("industrial.sequence-outputs", SEQUENCE_OUTPUT_CAPACITY, 12),
    CapacityBudget::scenario("industrial.robot-cell-queue", ROBOT_CELL_QUEUE_CAPACITY, 6),
    CapacityBudget::scenario("industrial.robot-program", ROBOT_PROGRAM_CAPACITY, 96),
    CapacityBudget::protocol("network.firewall-ha-ports", FIREWALL_HA_PORT_CAPACITY, 3),
    CapacityBudget::protocol("network.firewall-zones", FIREWALL_ZONE_CAPACITY, 12),
    CapacityBudget::protocol("network.firewall-rules", FIREWALL_RULE_CAPACITY, 12),
    CapacityBudget::scenario("network.firewall-sessions", FIREWALL_SESSION_CAPACITY, 96),
    CapacityBudget::protocol(
        "network.firewall-interfaces",
        FIREWALL_INTERFACE_CAPACITY,
        12,
    ),
    CapacityBudget::protocol("network.interface-addresses", INTERFACE_ADDRESS_CAPACITY, 3),
    CapacityBudget::protocol("network.first-hop-addresses", FIRST_HOP_CAPACITY, 3),
    CapacityBudget::protocol(
        "network.forwarding-interfaces",
        FORWARDING_INTERFACE_CAPACITY,
        12,
    ),
    CapacityBudget::scenario(
        "network.forwarding-pending",
        FORWARDING_PENDING_CAPACITY,
        12,
    ),
    CapacityBudget::protocol("network.proxy-addresses", PROXY_ADDRESS_CAPACITY, 12),
    CapacityBudget::scenario("network.neighbors", NEIGHBOR_CAPACITY, 24),
    CapacityBudget::protocol("network.router-routes", ROUTER_ROUTE_CAPACITY, 12),
    CapacityBudget::protocol("network.host-services", HOST_SERVICE_CAPACITY, 12),
    CapacityBudget::protocol("network.dns-records", DNS_RECORD_CAPACITY, 6),
    CapacityBudget::protocol("network.host-interfaces", HOST_INTERFACE_CAPACITY, 12),
    CapacityBudget::configured("network.unaddressed-ports", UNADDRESSED_PORT_CAPACITY, 18),
    CapacityBudget::scenario("network.host-pending", HOST_PENDING_CAPACITY, 12),
    CapacityBudget::protocol("network.host-routes", HOST_ROUTE_CAPACITY, 12),
    CapacityBudget::protocol("network.monitor-ports", MONITOR_PORT_CAPACITY, 12),
    CapacityBudget::protocol("network.gateway-hosts", GATEWAY_HOST_CAPACITY, 6),
    CapacityBudget::protocol("network.gateway-methods", GATEWAY_METHOD_CAPACITY, 6),
    CapacityBudget::protocol("network.gateway-rules", GATEWAY_RULE_CAPACITY, 12),
    CapacityBudget::scenario("network.gateway-pending", GATEWAY_PENDING_CAPACITY, 6),
    CapacityBudget::configured(
        "network.link-appliance-ports",
        LINK_APPLIANCE_PORT_CAPACITY,
        27,
    ),
    CapacityBudget::protocol("network.nat-inside-ports", NAT_INSIDE_PORT_CAPACITY, 12),
    CapacityBudget::protocol("network.nat-static-mappings", NAT_STATIC_CAPACITY, 12),
    CapacityBudget::scenario("network.nat-pat-sessions", NAT_PAT_CAPACITY, 48),
    CapacityBudget::protocol("network.nat-interfaces", NAT_INTERFACE_CAPACITY, 12),
    CapacityBudget::protocol("network.switch-vlans", SWITCH_VLAN_CAPACITY, 24),
    CapacityBudget::configured("network.switch-ports", SWITCH_PORT_CAPACITY, 18),
    CapacityBudget::scenario("network.switch-forwarding", SWITCH_FORWARDING_CAPACITY, 48),
    CapacityBudget::protocol(
        "network.switch-aggregations",
        SWITCH_AGGREGATION_CAPACITY,
        12,
    ),
    CapacityBudget::protocol(
        "network.switch-aggregation-members",
        SWITCH_AGGREGATION_MEMBER_CAPACITY,
        12,
    ),
    CapacityBudget::scenario("network.wireless-clients", WIRELESS_CLIENT_CAPACITY, 12),
    CapacityBudget::scenario("network.switch-egress", SWITCH_EGRESS_CAPACITY, 12),
    CapacityBudget::configured(
        "network.layer3-switch-ports",
        LAYER3_SWITCH_PORT_CAPACITY,
        18,
    ),
    CapacityBudget::configured(
        "runtime.partition-components",
        SIMULATOR_COMPONENT_CAPACITY,
        144,
    ),
    CapacityBudget::configured("runtime.partition-links", SIMULATOR_LINK_CAPACITY, 192),
    CapacityBudget::scenario("runtime.immediate-events", SIMULATOR_IMMEDIATE_CAPACITY, 48),
    CapacityBudget::scenario("runtime.delayed-events", SIMULATOR_DELAYED_CAPACITY, 48),
    CapacityBudget::scenario("runtime.trace", SIMULATOR_TRACE_CAPACITY, 168),
    CapacityBudget::scenario(
        "runtime.shared-media-fanout",
        SIMULATOR_SHARED_MEDIA_CAPACITY,
        24,
    ),
    CapacityBudget::scenario("runtime.effects-per-event", EFFECT_CAPACITY, 24),
];

pub fn capacity_budget(resource: &str) -> Option<&'static CapacityBudget> {
    RUNTIME_CAPACITY_BUDGETS
        .iter()
        .find(|budget| budget.resource == resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_budget_constructors_bind_evidence_and_saturation_contracts() {
        let configured = CapacityBudget::configured("configured", 8, 6);
        assert_eq!(
            configured.evidence,
            CapacityEvidence::CanonicalConfiguration
        );
        assert_eq!(
            configured.saturation,
            SaturationBehavior::RejectConfiguration
        );

        let scenario = CapacityBudget::scenario("scenario", 8, 6);
        assert_eq!(scenario.evidence, CapacityEvidence::ExecutableScenario);
        assert_eq!(scenario.saturation, SaturationBehavior::StopSimulation);

        let protocol = CapacityBudget::protocol("protocol", 8, 6);
        assert_eq!(protocol.evidence, CapacityEvidence::ProtocolContract);
        assert_eq!(protocol.saturation, SaturationBehavior::DropWithDiagnostic);

        let algorithmic = CapacityBudget::algorithmic("algorithmic", 8, 6);
        assert_eq!(algorithmic.evidence, CapacityEvidence::AlgorithmicBound);
        assert_eq!(algorithmic.saturation, SaturationBehavior::RejectOperation);

        for budget in [configured, scenario, protocol, algorithmic] {
            assert_eq!(budget.reserve(), 2);
            assert!(budget.has_required_headroom());
            assert!(budget.accepts_design_load(6));
            assert!(!budget.accepts_design_load(7));
        }
    }

    #[test]
    fn headroom_validation_rejects_missing_overloaded_and_under_reserved_budgets() {
        for budget in [
            CapacityBudget::new(
                "zero",
                8,
                0,
                CapacityEvidence::AlgorithmicBound,
                SaturationBehavior::RejectOperation,
                "evidence",
            ),
            CapacityBudget::new(
                "overloaded",
                8,
                9,
                CapacityEvidence::AlgorithmicBound,
                SaturationBehavior::RejectOperation,
                "evidence",
            ),
            CapacityBudget::new(
                "under-reserved",
                8,
                7,
                CapacityEvidence::AlgorithmicBound,
                SaturationBehavior::RejectOperation,
                "evidence",
            ),
            CapacityBudget::new(
                "missing-rationale",
                8,
                6,
                CapacityEvidence::AlgorithmicBound,
                SaturationBehavior::RejectOperation,
                "",
            ),
        ] {
            assert!(!budget.has_required_headroom(), "{}", budget.resource);
        }
        assert!(capacity_budget("runtime.partition-components").is_some());
        assert!(capacity_budget("unknown-resource").is_none());
    }
}

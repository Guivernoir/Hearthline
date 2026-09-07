//! Deterministic component behavior for Hearthline.
//!
//! The engine models appliance-level decisions and produces explained effects.
//! It is intentionally not a vendor firmware emulator or bit-level packet
//! simulator.

#![no_std]

mod capacity;
mod catalog;
mod industrial;
mod network;
mod physical;
mod runtime;

pub use capacity::{
    CapacityBudget, CapacityEvidence, EFFECT_CAPACITY, ROBOT_CELL_QUEUE_CAPACITY,
    ROBOT_PROGRAM_CAPACITY, RUNTIME_CAPACITY_BUDGETS, SEQUENCE_OUTPUT_CAPACITY,
    SEQUENCE_STEP_CAPACITY, SaturationBehavior, capacity_budget,
};
pub use catalog::{
    ApplianceContract, ApplianceFamilyContract, BehaviorImplementation, CapabilityProfile,
    FAMILY_CONTRACTS, RENDERED_ROLE_CONTRACTS, RUNTIME_COMPONENT_HANDLE_BYTES,
    RenderedRoleContract, SnapshotContract, appliance_contracts, appliance_family_contract,
};
pub use industrial::{
    Actuator, BodyPreparationControlState, BodyPreparationControlledTick, BodyPreparationFault,
    BodyPreparationMeasurements, BodyPreparationOutputs, BodyPreparationPhase,
    BodyPreparationPhysicsFeedback, BodyPreparationPhysicsInputs,
    BodyPreparationPipelineMeasurements, BodyPreparationProcess, BodyPreparationSetpoints,
    BodyPreparationStartError, BodyPreparationTick, BodyPreparationTrip, CeramicSlipBatch,
    Comparison, DownstreamMaterialEffects, FieldSensor, FormingControlState, FormingFault,
    FormingMeasurements, FormingOutputs, FormingPhase, FormingPhysicsFeedback,
    FormingPhysicsInputs, FormingProcess, FormingSetpoints, FormingStartError, FormingTick,
    FormingTrip, GlazeBatch, GlazeMeasurements, GlazePhase, GlazeSetpoints,
    HandoffPipelineMeasurements, HistorianBuffer, IoDirection, LogicRule, OperatorInterface,
    PUMP_HEARTBEAT_INTERVAL_MS, PUMP_HEARTBEAT_TIMEOUT_MS, PreparationTrain, PumpMaintenanceState,
    RemoteIo, ReturnWaterMeasurements, ReturnWaterPhase, RobotCartesianAxis,
    RobotCartesianIncrement, RobotCellArbiter, RobotCellRequestStatus, RobotCellStage,
    RobotInstruction, RobotJoints, RobotMotionError, RobotMotionKind, RobotMotionRuntime,
    RobotPose, RobotProgram, RobotProgramLine, RobotProgramRuntime, RobotWorkspace,
    SIMULATED_MS_PER_PROCESS_MINUTE, SafetyInterface, SequenceAssignment, SequenceCondition,
    SequenceInputs, SequenceProgram, SequenceRuntime, SequenceScan, SequenceStep,
    SequenceTransition, SlipMeasurements, SlipPhase, SlipSetpoints, VirtualPlc,
    WATER_NETWORK_PUMP_COUNT, WATER_NETWORK_ROUTE_COUNT, WaterMeasurements,
    WaterNetworkMeasurements, WaterPhase, WaterPumpMeasurements, WaterQuality,
    WaterRouteMeasurements, WaterSetpoints,
};
pub use network::{
    DnsServer, FirewallAction, FirewallHaRuntimeConfig, FirewallHaStatus, FirewallRule,
    FirewallSessionSnapshot, FirstHopAddress, HttpInspectionRule, HttpInspectionTarget,
    Layer3Switch, LearningSwitch, LinkAppliance, LinkMode, MacTableEntry, NatRouter, NeighborEntry,
    NeighborState, PassiveSensor, PatTranslation, ReverseProxyWaf, RoutedInterface, Router,
    RoutingTable, ServiceNode, StatefulFirewall, StaticNat, StaticNatError, SwitchAggregationGroup,
    SwitchPort, UnaddressedNode, WirelessAccessPoint,
};
pub use physical::{
    CarrierMedium, ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, FiberMedium,
    FiberMode, FieldWiringMedium, LinkDirection, LinkEndpoint, MediaDropReason, MediaError,
    MediaFacts, MediaLink, MediaLinkConfig, MediaLinkError, MediaText, MediaTransit, MediumKind,
    PortDuplex, PortHardwareKind, PortSettings, PortState, PortStateConfig, RadioMedium,
    SimulatedMedium, SimulatedPort, TelephoneMedium, VirtualMedium, appliance_supports_port,
};
pub use runtime::{
    DropReason, Effect, EffectList, FirewallHaControl, Ipv4Egress, NetworkIngress, ProcessEffect,
    SimulatedComponent, SimulationError, SimulationEvent, Simulator, TraceEntry,
};

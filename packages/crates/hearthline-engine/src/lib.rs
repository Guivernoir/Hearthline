//! Deterministic component behavior for Hearthline.
//!
//! The engine models appliance-level decisions and produces explained effects.
//! It is intentionally not a vendor firmware emulator or bit-level packet
//! simulator.

#![no_std]

mod catalog;
mod industrial;
mod network;
mod physical;
mod runtime;

pub use catalog::{
    ApplianceContract, RENDERED_ROLE_CONTRACTS, RenderedRoleContract, appliance_contracts,
};
pub use industrial::{
    Actuator, BodyPreparationFault, BodyPreparationMeasurements, BodyPreparationOutputs,
    BodyPreparationPhase, BodyPreparationPipelineMeasurements, BodyPreparationProcess,
    BodyPreparationSetpoints, BodyPreparationStartError, BodyPreparationTick, BodyPreparationTrip,
    CeramicSlipBatch, Comparison, DownstreamMaterialEffects, FieldSensor, FormingFault,
    FormingMeasurements, FormingOutputs, FormingPhase, FormingProcess, FormingSetpoints,
    FormingStartError, FormingTick, FormingTrip, GlazeBatch, GlazeMeasurements, GlazePhase,
    GlazeSetpoints, HandoffPipelineMeasurements, HistorianBuffer, IoDirection, LogicRule,
    OperatorInterface, PUMP_HEARTBEAT_INTERVAL_MS, PUMP_HEARTBEAT_TIMEOUT_MS, PreparationTrain,
    PumpMaintenanceState, ROBOT_CELL_QUEUE_CAPACITY, ROBOT_PROGRAM_CAPACITY, RemoteIo,
    ReturnWaterMeasurements, ReturnWaterPhase, RobotCartesianAxis, RobotCellArbiter,
    RobotCellRequestStatus, RobotCellStage, RobotInstruction, RobotJoints, RobotMotionError,
    RobotMotionKind, RobotMotionRuntime, RobotPose, RobotProgram, RobotProgramLine,
    RobotProgramRuntime, RobotWorkspace, SEQUENCE_OUTPUT_CAPACITY, SEQUENCE_STEP_CAPACITY,
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
    SwitchPort, WirelessAccessPoint,
};
pub use physical::{
    CarrierMedium, ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, FiberMedium,
    FiberMode, FieldWiringMedium, LinkDirection, LinkEndpoint, MediaDropReason, MediaError,
    MediaFacts, MediaLink, MediaLinkConfig, MediaLinkError, MediaText, MediaTransit, MediumKind,
    PortDuplex, PortHardwareKind, PortSettings, PortState, PortStateConfig, RadioMedium,
    SimulatedMedium, SimulatedPort, TelephoneMedium, VirtualMedium, appliance_supports_port,
};
pub use runtime::{
    DropReason, EFFECT_CAPACITY, Effect, EffectList, FirewallHaControl, Ipv4Egress, NetworkIngress,
    ProcessEffect, SimulatedComponent, SimulationError, SimulationEvent, Simulator, TraceEntry,
};

import type { HmiRobotState } from "./robot";
import type { HmiSupervisoryState } from "./supervisory";

export interface HmiSnapshot {
  schemaVersion: string;
  id: string;
  label: string;
  environment: string;
  zone: string;
  role: string;
  interfaceKind: "hmi" | "scada-workstation";
  controller: string;
  remoteIo: string;
  remoteIoStations: string[];
  permissions: string[];
  sequence: number;
  controlProgram: HmiControlProgramState | null;
  controlStation: HmiControlStation | null;
  stationStatus: HmiStationStatus[];
  parameters: HmiParameter[];
  recipes: HmiRecipe[];
  activeRecipe: string | null;
  process: HmiProcessState | null;
  bodyPreparation: HmiBodyPreparationState | null;
  moulds: HmiMouldProcessState[];
  robot: HmiRobotState | null;
  guardedCell: HmiGuardedCellState | null;
  supervisory: HmiSupervisoryState | null;
  signals: HmiSignal[];
  actuators: HmiActuator[];
  safety: HmiSafety[];
  alarms: HmiAlarm[];
  audit: HmiAuditEntry[];
}

export interface HmiBodyPreparationState {
  recipeBasis: string;
  simulatedMsPerProcessMinute: number;
  slip: HmiSlipPreparationState;
  water: HmiWaterPreparationState;
  returnWater: HmiReturnWaterState;
  glaze: HmiGlazePreparationState;
  pipelines: HmiBodyPreparationPipelineState;
  waterNetworks: HmiWaterNetworkState;
}

export type BodyPreparationHmiScope =
  | "slip"
  | "water-process"
  | "water-pipeline"
  | "return-water-process"
  | "return-water-pipeline"
  | "glaze";

export type WaterHmiScope = Extract<
  BodyPreparationHmiScope,
  "water-process" | "water-pipeline" | "return-water-process" | "return-water-pipeline"
>;

export interface HmiWaterNetworkState {
  pumps: HmiWaterPumpState[];
  routes: HmiWaterRouteState[];
  heartbeatIntervalMs: number;
  heartbeatTimeoutMs: number;
}

export interface HmiWaterPumpState {
  id: string;
  label: string;
  groupId: string;
  service: string;
  preferredDuty: boolean;
  commanded: boolean;
  runningFeedback: boolean;
  heartbeatSequence: number;
  heartbeatAgeMs: number;
  heartbeatOk: boolean;
  maintenance: "normal" | "required" | "dispatched";
}

export interface HmiWaterRouteState {
  id: string;
  label: string;
  network: "industrial" | "return";
  source: string;
  destination: string;
  pumpGroup: string;
  demanded: boolean;
  available: boolean;
  inletFlowLMin: number;
  outletFlowLMin: number;
  inletPressureBar: number;
  outletPressureBar: number;
  leakDetected: boolean;
  quality: HmiWaterQuality;
}

export interface HmiBodyPreparationPipelineState {
  waterToSlip: HmiHandoffPipelineState;
  waterToGlaze: HmiHandoffPipelineState;
  slipToForming: HmiHandoffPipelineState;
  glazeToGlazing: HmiHandoffPipelineState;
}

export interface HmiHandoffPipelineState {
  inletFlowLMin: number;
  outletFlowLMin: number;
  inletPressureBar: number;
  outletPressureBar: number;
  lineLossPercent: number;
  entrainedAirPercent: number;
  deliveredQualityPercent: number;
  leakDetected: boolean;
}

export type HmiPreparationTrain = "slip" | "water" | "return-water" | "glaze";

export interface HmiPreparationTrainState {
  id: HmiPreparationTrain;
  label: string;
  running: boolean;
  held: boolean;
  phase: string;
  phaseProgressPercent: number;
  phaseElapsedProcessMinutes: number;
  phaseTargetProcessMinutes: number;
  cycleCount: number;
  phases: HmiProcessPhase[];
}

export interface HmiSlipPreparationState {
  train: HmiPreparationTrainState;
  batchMassKg: number;
  targetBatchMassKg: number;
  solidsPercent: number;
  densityKgL: number;
  highShearViscosityMpaS: number;
  lowShearViscosityMpaS: number;
  thixotropicIndex: number;
  structureParameter: number;
  temperatureC: number;
  mixerLevelPercent: number;
  conditioningTankLevelPercent: number;
  transferFlowLMin: number;
  specificEnergyKwhT: number;
  residue44umPercent: number;
  medianParticleUm: number;
  castingRateGCm2Min: number;
  qualityIndex: number;
  qualityReleased: boolean;
  ingredients: HmiBodyIngredientState[];
  qualityChecks: HmiBodyQualityCheck[];
  water: HmiWaterQuality;
  downstream: HmiDownstreamMaterialEffects;
}

export interface HmiWaterPreparationState {
  train: HmiPreparationTrainState;
  rawTankL: number;
  treatedTankL: number;
  feedFlowLMin: number;
  permeateFlowLMin: number;
  rejectFlowLMin: number;
  mediaFilterDpBar: number;
  roRecoveryPercent: number;
  raw: HmiWaterQuality;
  product: HmiWaterQuality;
}

export interface HmiReturnWaterState {
  train: HmiPreparationTrainState;
  activeStream: string;
  bodyEqualizationL: number;
  glazeEqualizationL: number;
  bodyReuseTankL: number;
  glazeReuseTankL: number;
  feedFlowLMin: number;
  clarifiedFlowLMin: number;
  sludgeCakeKg: number;
  influentTurbidityNtu: number;
  effluentTurbidityNtu: number;
  bodyReuseQuality: HmiWaterQuality;
  glazeReuseQuality: HmiWaterQuality;
}

export interface HmiGlazePreparationState {
  train: HmiPreparationTrainState;
  powderMassKg: number;
  targetPowderMassKg: number;
  batchMassKg: number;
  solidsPercent: number;
  densityKgL: number;
  fordCupSeconds: number;
  medianParticleUm: number;
  residue63umPercent: number;
  millEnergyKwhT: number;
  storageLevelPercent: number;
  transferFlowLMin: number;
  settlingRiskPercent: number;
  qualityIndex: number;
  qualityReleased: boolean;
  ingredients: HmiBodyIngredientState[];
  qualityChecks: HmiBodyQualityCheck[];
  water: HmiWaterQuality;
}

export interface HmiWaterQuality {
  temperatureC: number;
  ph: number;
  turbidityNtu: number;
  conductivityUsCm: number;
  hardnessMgLCaco3: number;
  suspendedSolidsMgL: number;
  glazeContaminationPercent: number;
  recoveredFractionPercent: number;
}

export interface HmiDownstreamMaterialEffects {
  fillingFlowFactor: number;
  castingRateGCm2Min: number;
  predictedGreenMoisturePercent: number;
  predictedDryingShrinkagePercent: number;
  dryingEnergyFactor: number;
  greenStrengthIndex: number;
  firedDefectRiskPercent: number;
}

export interface HmiBodyIngredientState {
  id: string;
  label: string;
  targetKg: number;
  actualKg: number;
}

export interface HmiBodyQualityCheck {
  id: string;
  label: string;
  value: number;
  unit: string;
  minimum: number;
  maximum: number;
  withinLimit: boolean;
}

export interface HmiGuardedCellState {
  guard: HmiCellGuardState;
  handoffStations: HmiHandoffStationState[];
}

export interface HmiCellGuardState {
  safetyComponent: string;
  positionSensor: string;
  position: "open" | "closed";
  closedPermissive: boolean;
  resetRequired: boolean;
}

export interface HmiHandoffStationState {
  mould: string;
  actuator: string;
  state: "stopped" | "in-cell" | "moving-to-operator" | "operator-side" | "moving-to-cell";
  progressPercent: number;
  inCellSensor: string;
  operatorSideSensor: string;
  inCellConfirmed: boolean;
  operatorSideConfirmed: boolean;
  piecePresent: boolean;
}

export interface HmiMouldProcessState {
  target: string;
  label: string;
  phase: string;
  operatingState:
    | "stopped"
    | "producing"
    | "stopping-after-phase"
    | "ending-after-cycle"
    | "paused"
    | "faulted";
  running: boolean;
  productionEnabled: boolean;
  paused: boolean;
  stopRequest: "after-phase" | "after-cycle" | null;
  phaseElapsedMs: number;
  scanCount: number;
  cycleCount: number;
  fault: HmiProcessFault | null;
  targetDurationMs: number;
  castingPressureBar: number;
  setpointsBound: boolean;
  controlCabinet: HmiMouldControlCabinet | null;
  utilityCabinet: HmiMouldUtilityCabinet | null;
  phases: HmiProcessPhase[];
}

export interface HmiMouldControlCabinet {
  remoteIo: string;
  enclosureRating: string;
  controlVoltageVdc: number;
  safetyRelay: string;
  modules: string[];
}

export interface HmiMouldUtilityCircuit {
  id: string;
  label: string;
  medium: string;
  source: string;
  nominalPressure: number | null;
  state: string;
}

export interface HmiMouldUtilityCabinet {
  actuator: string;
  enclosureRating: string;
  controlVoltageVdc: number;
  isolationState: string;
  activeState: string;
  circuits: HmiMouldUtilityCircuit[];
}

export type HmiControlMode = "manual" | "auto" | "setup";

export interface HmiControlStation {
  stationType: "machine-pc" | "mould-panel" | "robot-joystick";
  target: string;
  positions: HmiControlMode[];
  selectedMode: HmiControlMode;
  setupAuthenticated: boolean;
  sensorBypassActive: boolean;
  bypassedPermissives: string[];
  retainedProtections: string[];
}

export interface HmiStationStatus {
  stationId: string;
  label: string;
  stationType: "machine-pc" | "mould-panel" | "robot-joystick";
  target: string;
  selectedMode: HmiControlMode;
  setupAuthenticated: boolean;
  sensorBypassActive: boolean;
}

export interface HmiParameter {
  id: string;
  label: string;
  target: string;
  unit: string;
  minimum: number;
  maximum: number;
  step: number;
  value: number;
}

export interface HmiRecipe {
  id: string;
  label: string;
  description: string;
}

export interface HmiControlProgramState {
  language: "structured-text";
  program: string;
  task: string;
  sourcePath: string;
  bindingPath: string;
  revision: string;
  currentStep: number;
  scanIntervalMs: number;
  watchdogMs: number;
}

export interface HmiControlProgramDocument {
  schemaVersion: string;
  controller: string;
  language: "structured-text";
  program: string;
  task: string;
  sourcePath: string;
  bindingPath: string;
  revision: string;
  source: string;
  bindingYaml: string;
}

export interface HmiProcessState {
  model: string;
  phase: string;
  running: boolean;
  phaseElapsedMs: number;
  scanCount: number;
  cycleCount: number;
  fault: HmiProcessFault | null;
  phases: HmiProcessPhase[];
}

export interface HmiProcessPhase {
  key: string;
  label: string;
}

export type HmiProcessFault =
  | "slip-supply-loss"
  | "compressed-air-loss"
  | "mould-overpressure"
  | "vacuum-loss"
  | "robot-pickup-failure"
  | "ingredient-shortage"
  | "mixer-overload"
  | "screen-blocked"
  | "quality-out-of-spec"
  | "transfer-no-flow"
  | "raw-water-quality"
  | "water-filter-blocked"
  | "return-water-contamination"
  | "glaze-mill-overload"
  | "glaze-quality-out-of-spec"
  | "slip-pipeline-leak"
  | "water-to-slip-leak"
  | "water-to-glaze-leak"
  | "glaze-pipeline-leak";

export interface HmiSignal {
  componentId: string;
  label: string;
  tag: string;
  unit: string;
  minimum: number;
  maximum: number;
  value: number;
  qualityGood: boolean;
  timestampMs: number;
}

export interface HmiActuator {
  componentId: string;
  label: string;
  commandTag: string;
  feedbackTag: string | null;
  safeState: string;
  states: string[];
  currentState: string;
}

export interface HmiSafety {
  componentId: string;
  label: string;
  permissives: HmiPermissive[];
  tripLatched: boolean;
}

export interface HmiPermissive {
  tag: string;
  satisfied: boolean;
}

export interface HmiAlarm {
  id: string;
  code: string;
  source: string;
  message: string;
  severity: "warning" | "trip";
  active: boolean;
  acknowledged: boolean;
  sequence: number;
}

export interface HmiAuditEntry {
  sequence: number;
  action: string;
  target: string;
  result: string;
}

export interface HmiTraceEntry {
  sequence: number;
  component: string;
  stage: string;
  detail: string;
}

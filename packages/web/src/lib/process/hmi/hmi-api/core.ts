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
  | "robot-pickup-failure";

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

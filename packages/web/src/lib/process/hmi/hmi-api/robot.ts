export interface HmiRobotPose {
  x: number;
  y: number;
  z: number;
  w: number;
  p: number;
  r: number;
}

export type HmiRobotCoordinateSystem = "world" | "joint";
export type HmiRobotAxis =
  | "x"
  | "y"
  | "z"
  | "w"
  | "p"
  | "r"
  | "j1"
  | "j2"
  | "j3"
  | "j4"
  | "j5"
  | "j6";

export interface HmiRobotMotionState {
  active: boolean;
  kind: "rapid" | "linear" | "joint" | "jog";
  progressPercent: number;
  elapsedMs: number;
  durationMs: number;
  speedPercent: number;
  targetPose: HmiRobotPose;
  targetJoints: number[];
}

export interface HmiRobotProgramLine {
  number: number;
  source: string;
  operation: string | null;
  active: boolean;
}

export interface HmiRobotProgramState {
  name: string;
  sourcePath: string;
  revision: string;
  running: boolean;
  paused: boolean;
  activeLine: number | null;
  cycleCount: number;
  source: string;
  lines: HmiRobotProgramLine[];
}

export interface HmiRobotTaughtPosition {
  id: string;
  label: string;
  pose: HmiRobotPose;
}

export interface HmiRobotWorkspace {
  minimum: HmiRobotPose;
  maximum: HmiRobotPose;
  jointMinimum: number[];
  jointMaximum: number[];
}

export interface HmiRobotState {
  coordinateSystem: HmiRobotCoordinateSystem;
  motionEnabled: boolean;
  pose: HmiRobotPose;
  joints: number[];
  gripperClosed: boolean;
  automaticCommand: string;
  controllerState: "ready" | "executing" | "faulted";
  activeUserFrame: string;
  activeTool: string;
  activePayload: string;
  architecture: HmiRobotArchitecture;
  frames: HmiRobotFrame[];
  payloads: HmiRobotPayload[];
  tools: HmiRobotTool[];
  handoffs: HmiRobotHandoff[];
  cell: HmiRobotCellState;
  motion: HmiRobotMotionState;
  program: HmiRobotProgramState;
  taughtPositions: HmiRobotTaughtPosition[];
  workspace: HmiRobotWorkspace;
}

export interface HmiRobotArchitecture {
  controller: string;
  manipulator: string;
  pendant: string;
  safetyInterface: string;
  cellController: string;
  servoAxes: number;
  motionGroup: string;
  interpolationCycleMs: number;
}

export interface HmiRobotFrame {
  id: string;
  label: string;
  parent: string | null;
  pose: HmiRobotPose;
}

export interface HmiRobotPayload {
  id: string;
  label: string;
  massKg: number;
  centerOfMassMm: number[];
}

export interface HmiRobotTool {
  id: string;
  label: string;
  tcp: HmiRobotPose;
  payload: string;
}

export interface HmiRobotHandoff {
  mould: string;
  program: string;
  userFrame: string;
  approachPosition: string;
  pickupPosition: string;
  handoffPosition: string;
  retreatPosition: string;
  pickupToleranceMm: number;
  handoffToleranceMm: number;
  orientationToleranceDeg: number;
}

export interface HmiRobotCellState {
  activeMould: string | null;
  queuedMoulds: string[];
  stage: string;
  completedHandoffs: number;
  activeProgram: string | null;
  faultCode: string | null;
  faultMessage: string | null;
}

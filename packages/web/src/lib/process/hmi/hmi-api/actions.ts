import type {
  HmiControlMode,
  HmiPreparationTrain,
  HmiProcessFault,
  HmiSnapshot,
  HmiTraceEntry,
} from "./core";
import type {
  HmiRobotAxis,
  HmiRobotCoordinateSystem,
  HmiRobotPose,
} from "./robot";

export type HmiAction =
  | { kind: "command"; tag: string; value: string }
  | { kind: "start-process" }
  | { kind: "hold-process" }
  | { kind: "start-preparation-train"; train: HmiPreparationTrain }
  | { kind: "hold-preparation-train"; train: HmiPreparationTrain }
  | { kind: "set-water-pump-failure"; pumpId: string; failed: boolean }
  | { kind: "dispatch-water-pump-maintenance"; pumpId: string }
  | { kind: "start-mould" }
  | { kind: "stop-mould-after-phase" }
  | { kind: "end-mould-after-cycle" }
  | { kind: "reset-process" }
  | { kind: "set-process-fault"; fault: HmiProcessFault; active: boolean }
  | { kind: "reset-safety"; safetyId: string }
  | { kind: "set-guard-door"; open: boolean }
  | { kind: "acknowledge-alarm"; alarmId: string }
  | { kind: "set-control-mode"; mode: HmiControlMode; password?: string }
  | { kind: "set-parameter"; parameterId: string; value: number }
  | { kind: "select-recipe"; recipeId: string }
  | { kind: "set-robot-motion-enable"; enabled: boolean }
  | { kind: "move-robot"; target: HmiRobotPose; speedPercent: number }
  | { kind: "move-robot-to-position"; positionId: string; speedPercent: number }
  | {
      kind: "jog-robot";
      coordinateSystem: HmiRobotCoordinateSystem;
      axis: HmiRobotAxis;
      increment: number;
      speedPercent: number;
    }
  | { kind: "teach-robot-position"; positionId: string; label: string }
  | { kind: "run-robot-program" }
  | { kind: "pause-robot-program" }
  | { kind: "step-robot-program" }
  | { kind: "reset-robot-program" }
  | { kind: "load-robot-program"; name: string; source: string };

export interface HmiActionReport {
  schemaVersion: string;
  status: "applied" | "completed" | "denied";
  message: string;
  trace: HmiTraceEntry[];
  snapshot: HmiSnapshot;
}

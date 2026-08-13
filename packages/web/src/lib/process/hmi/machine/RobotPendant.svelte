<script lang="ts">
  import { Braces, CircleGauge, Cpu, Crosshair, Frame, Hand, Power, ShieldCheck, Workflow } from "@lucide/svelte";
  import type {
    HmiAction,
    HmiActuator,
    HmiControlStation,
    HmiGuardedCellState,
    HmiRobotState,
    HmiSafety,
  } from "../hmi-api";
  import RobotJogPanel from "./RobotJogPanel.svelte";
  import RobotLiveView from "./RobotLiveView.svelte";
  import RobotProgramPanel from "./RobotProgramPanel.svelte";

  export let station: HmiControlStation;
  export let robot: HmiRobotState;
  export let actuators: HmiActuator[] = [];
  export let safety: HmiSafety[] = [];
  export let guardedCell: HmiGuardedCellState | null = null;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  let tab: "status" | "jog" | "program" = "status";
  $: safetyHealthy = safety.every((state) => !state.tripLatched && state.permissives.every((item) => item.satisfied));
  $: modeAllowsEnable = ["manual", "setup"].includes(station.selectedMode);
  $: robotActuator = actuators.find((actuator) => actuator.commandTag === "area-02-robot-01-command") ?? null;

  function display(value: string) {
    return value.replaceAll("-", " ");
  }
</script>

<section class:faulted={Boolean(robot.cell.faultCode)} class="robot-pendant" aria-label="Robot teach pendant">
  <header>
    <div><CircleGauge size={18} /><span><small>Robot controller</small><strong>{robot.motion.active ? "motion active" : display(robot.automaticCommand)}</strong></span></div>
    <dl>
      <div><dt>Mode</dt><dd>{station.selectedMode}</dd></div>
      <div><dt>Program</dt><dd>{robot.cell.activeProgram ?? robot.program.name}</dd></div>
      <div><dt>Line</dt><dd>{robot.program.activeLine ?? "-"}</dd></div>
    </dl>
    <div class:safe={safetyHealthy} class="robot-safety-state"><ShieldCheck size={15} />{safetyHealthy ? "Safety ready" : "Safety inhibited"}</div>
  </header>

  <RobotLiveView {robot} {guardedCell} />

  <div class="robot-controller-strip">
    <span><Cpu size={15} /><small>Controller</small><strong>{robot.architecture.controller}</strong><em>{robot.controllerState}</em></span>
    <span><Workflow size={15} /><small>Motion group</small><strong>{robot.architecture.motionGroup}</strong><em>{robot.architecture.servoAxes} axes / {robot.architecture.interpolationCycleMs} ms</em></span>
    <span><Frame size={15} /><small>User frame</small><strong>{robot.activeUserFrame}</strong><em>{robot.activeTool}</em></span>
    <span><Hand size={15} /><small>Payload</small><strong>{robot.activePayload}</strong><em>{robot.payloads.find((item) => item.id === robot.activePayload)?.massKg ?? "-"} kg</em></span>
  </div>

  <nav class="robot-workspace-tabs" aria-label="Robot pendant views">
    <button class:active={tab === "status"} type="button" onclick={() => (tab = "status")}><CircleGauge size={15} />Status</button>
    <button class:active={tab === "jog"} type="button" onclick={() => (tab = "jog")}><Crosshair size={15} />Jog</button>
    <button class:active={tab === "program"} type="button" onclick={() => (tab = "program")}><Braces size={15} />Program</button>
    <button
      class:active={robot.motionEnabled}
      class="robot-enable-command"
      type="button"
      disabled={!modeAllowsEnable || !safetyHealthy || Boolean(busyTarget)}
      aria-pressed={robot.motionEnabled}
      onclick={() => onExecute({ kind: "set-robot-motion-enable", enabled: !robot.motionEnabled }, "robot-motion-enable")}
    ><Power size={15} />{robot.motionEnabled ? "Motion enabled" : "Enable motion"}</button>
  </nav>

  {#if tab === "status"}
    <div class="robot-status-workspace">
      <section>
        <header><span><Crosshair size={15} />Cartesian position</span><strong>{robot.coordinateSystem}</strong></header>
        <dl>{#each Object.entries(robot.pose) as [axis, value]}<div><dt>{axis.toUpperCase()}</dt><dd>{value.toFixed(2)}<small>{["x", "y", "z"].includes(axis) ? "mm" : "deg"}</small></dd></div>{/each}</dl>
      </section>
      <section>
        <header><span><CircleGauge size={15} />Joint position</span><strong>degrees</strong></header>
        <dl>{#each robot.joints as value, index}<div><dt>J{index + 1}</dt><dd>{value.toFixed(2)}<small>deg</small></dd></div>{/each}</dl>
      </section>
      <section class="robot-position-summary">
        <header><span><Hand size={15} />Position registers</span><strong>{robot.taughtPositions.length}</strong></header>
        {#each robot.taughtPositions as position}<article><span><strong>{position.label}</strong><small>{position.id}</small></span><em>X {position.pose.x.toFixed(0)} / Y {position.pose.y.toFixed(0)} / Z {position.pose.z.toFixed(0)}</em></article>{/each}
      </section>
      <section class="robot-cell-summary">
        <header><span><Workflow size={15} />Cell arbitration</span><strong>{display(robot.cell.stage)}</strong></header>
        {#if robot.cell.faultCode}
          <div class="robot-cell-fault" role="alert">
            <strong>{robot.cell.faultCode}</strong>
            <span>{robot.cell.faultMessage}</span>
          </div>
        {/if}
        <dl>
          <div><dt>Active mould</dt><dd>{robot.cell.activeMould ?? "none"}</dd></div>
          <div><dt>Active routine</dt><dd>{robot.cell.activeProgram ?? "none"}</dd></div>
          <div><dt>Queue depth</dt><dd>{robot.cell.queuedMoulds.length}</dd></div>
          <div><dt>Completed</dt><dd>{robot.cell.completedHandoffs}</dd></div>
        </dl>
        <div class="robot-handoff-registers">
          {#each robot.handoffs as handoff}
            <article class:active={robot.cell.activeMould === handoff.mould}>
              <strong>{handoff.mould} / {handoff.program}</strong>
              <span>{handoff.userFrame}</span>
              <small>{handoff.pickupPosition} to {handoff.handoffPosition}</small>
              <small>Pick +/-{handoff.pickupToleranceMm} mm / out +/-{handoff.handoffToleranceMm} mm</small>
            </article>
          {/each}
        </div>
      </section>
    </div>
  {:else if tab === "jog"}
    <RobotJogPanel {robot} {station} actuator={robotActuator} {busyTarget} {onExecute} />
  {:else}
    <RobotProgramPanel {robot} {station} {busyTarget} {onExecute} />
  {/if}
</section>

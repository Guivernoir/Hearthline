<script lang="ts">
  import { Crosshair, Gauge, Hand, LoaderCircle, Minus, Plus, Save, Target } from "@lucide/svelte";
  import type {
    HmiAction,
    HmiActuator,
    HmiControlStation,
    HmiRobotAxis,
    HmiRobotCoordinateSystem,
    HmiRobotPose,
    HmiRobotState,
  } from "../hmi-api";

  export let robot: HmiRobotState;
  export let actuator: HmiActuator | null = null;
  export let station: HmiControlStation;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  let coordinateSystem: HmiRobotCoordinateSystem = "world";
  let increment = 10;
  let speedPercent = 15;
  let target: HmiRobotPose = { ...robot.pose };
  let positionId = "service-point";
  let positionLabel = "Service point";

  $: axes = coordinateSystem === "world"
    ? (["x", "y", "z", "w", "p", "r"] as HmiRobotAxis[])
    : (["j1", "j2", "j3", "j4", "j5", "j6"] as HmiRobotAxis[]);
  $: authority = ["manual", "setup"].includes(station.selectedMode);
  $: canMove = authority && robot.motionEnabled && !robot.motion.active && !busyTarget;

  function moveToCurrentDraft() {
    onExecute({ kind: "move-robot", target, speedPercent }, "robot-cartesian-target");
  }

  function jog(axis: HmiRobotAxis, direction: number) {
    onExecute(
      { kind: "jog-robot", coordinateSystem, axis, increment: increment * direction, speedPercent },
      `robot-jog-${axis}`,
    );
  }

  function copyCurrent() {
    target = { ...robot.pose };
  }

  function axisValue(pose: HmiRobotPose, axis: HmiRobotAxis) {
    return pose[axis as keyof HmiRobotPose] ?? 0;
  }
</script>

<div class="robot-jog-workspace">
  <section class="robot-jog-controls">
    <header><span><Crosshair size={16} />Jog control</span><strong>{coordinateSystem}</strong></header>
    <div class="robot-jog-settings">
      <div class="robot-segmented" role="group" aria-label="Jog coordinate system">
        <button class:active={coordinateSystem === "world"} type="button" onclick={() => (coordinateSystem = "world")}>World</button>
        <button class:active={coordinateSystem === "joint"} type="button" onclick={() => (coordinateSystem = "joint")}>Joint</button>
      </div>
      <label><span>Increment</span><select bind:value={increment}><option value={1}>1 {coordinateSystem === "world" ? "mm" : "deg"}</option><option value={10}>10 {coordinateSystem === "world" ? "mm" : "deg"}</option><option value={50}>50 {coordinateSystem === "world" ? "mm" : "deg"}</option></select></label>
      <label class="robot-speed-control"><span><Gauge size={13} />Speed override</span><input bind:value={speedPercent} type="range" min="1" max="50" step="1" /><strong>{speedPercent}%</strong></label>
    </div>
    <div class="robot-axis-grid">
      {#each axes as axis}
        <article>
          <strong>{axis.toUpperCase()}</strong>
          <button type="button" aria-label={`${axis} negative jog`} disabled={!canMove} onclick={() => jog(axis, -1)}>{#if busyTarget === `robot-jog-${axis}`}<LoaderCircle class="spin" size={13} />{:else}<Minus size={15} />{/if}</button>
          <span>{coordinateSystem === "world" ? axisValue(robot.pose, axis).toFixed(1) : robot.joints[Number(axis.slice(1)) - 1].toFixed(1)}</span>
          <button type="button" aria-label={`${axis} positive jog`} disabled={!canMove} onclick={() => jog(axis, 1)}><Plus size={15} /></button>
        </article>
      {/each}
    </div>
    {#if actuator}
      <div class="robot-sequence-commands" aria-label="Robot sequence commands">
        {#each actuator.states as state}
          <button
            class:active={actuator.currentState === state}
            type="button"
            aria-pressed={actuator.currentState === state}
            disabled={!canMove}
            onclick={() => onExecute(
              { kind: "command", tag: actuator.commandTag, value: state === actuator.currentState ? actuator.safeState : state },
              actuator.commandTag,
            )}
          >{state.replaceAll("-", " ")}</button>
        {/each}
      </div>
    {/if}
  </section>

  <section class="robot-position-panel">
    <header><span><Target size={16} />Cartesian target</span><button type="button" title="Use current position" aria-label="Use current robot position" onclick={copyCurrent}><Crosshair size={14} /></button></header>
    <div class="robot-coordinate-inputs">
      <label><span>X</span><input bind:value={target.x} type="number" step="1" /></label>
      <label><span>Y</span><input bind:value={target.y} type="number" step="1" /></label>
      <label><span>Z</span><input bind:value={target.z} type="number" step="1" /></label>
      <label><span>W</span><input bind:value={target.w} type="number" step="1" /></label>
      <label><span>P</span><input bind:value={target.p} type="number" step="1" /></label>
      <label><span>R</span><input bind:value={target.r} type="number" step="1" /></label>
    </div>
    <button class="robot-primary-command" type="button" disabled={!canMove} onclick={moveToCurrentDraft}>{#if busyTarget === "robot-cartesian-target"}<LoaderCircle class="spin" size={14} />{:else}<Target size={14} />{/if}Move to target</button>
  </section>

  <section class="robot-taught-panel">
    <header><span><Hand size={16} />Taught positions</span><strong>{robot.taughtPositions.length}</strong></header>
    <div class="robot-taught-list">
      {#each robot.taughtPositions as position}
        <button type="button" disabled={!canMove} onclick={() => onExecute({ kind: "move-robot-to-position", positionId: position.id, speedPercent }, `robot-position-${position.id}`)}>
          <span><strong>{position.label}</strong><small>{position.id}</small></span><em>X {position.pose.x.toFixed(0)} / Y {position.pose.y.toFixed(0)} / Z {position.pose.z.toFixed(0)}</em><Target size={14} />
        </button>
      {/each}
    </div>
    <div class="robot-teach-row">
      <input bind:value={positionId} aria-label="Taught position ID" placeholder="position-id" />
      <input bind:value={positionLabel} aria-label="Taught position label" placeholder="Position label" />
      <button type="button" title="Teach current position" aria-label="Teach current robot position" disabled={station.selectedMode !== "setup" || Boolean(busyTarget)} onclick={() => onExecute({ kind: "teach-robot-position", positionId, label: positionLabel }, "robot-teach-position")}><Save size={15} /></button>
    </div>
  </section>
</div>

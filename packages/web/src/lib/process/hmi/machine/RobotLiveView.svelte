<script lang="ts">
  import { Crosshair, Gauge, Hand, Route, Timer } from "@lucide/svelte";
  import type { HmiGuardedCellState, HmiRobotState } from "../hmi-api";

  export let robot: HmiRobotState;
  export let guardedCell: HmiGuardedCellState | null = null;

  $: xRatio = normalize(robot.pose.x, robot.workspace.minimum.x, robot.workspace.maximum.x);
  $: yRatio = normalize(robot.pose.y, robot.workspace.minimum.y, robot.workspace.maximum.y);
  $: zRatio = normalize(robot.pose.z, robot.workspace.minimum.z, robot.workspace.maximum.z);
  $: targetX = 110 + normalize(robot.motion.targetPose.x, robot.workspace.minimum.x, robot.workspace.maximum.x) * 390;
  $: targetY = 310 - normalize(robot.motion.targetPose.z, robot.workspace.minimum.z, robot.workspace.maximum.z) * 220;
  $: baseX = 220 + xRatio * 140;
  $: shoulder = -42 + normalize(robot.joints[1], robot.workspace.jointMinimum[1], robot.workspace.jointMaximum[1]) * 68;
  $: elbow = -68 + normalize(robot.joints[2], robot.workspace.jointMinimum[2], robot.workspace.jointMaximum[2]) * 105;
  $: wrist = -30 + normalize(robot.joints[4], robot.workspace.jointMinimum[4], robot.workspace.jointMaximum[4]) * 60;
  $: tcpX = 110 + xRatio * 390;
  $: tcpY = 310 - zRatio * 220;
  $: activeHandoff = robot.handoffs.find((handoff) => handoff.mould === robot.cell.activeMould) ?? null;

  function normalize(value: number, minimum: number, maximum: number) {
    return Math.min(1, Math.max(0, (value - minimum) / (maximum - minimum)));
  }

  function display(value: string) {
    return value.replaceAll("-", " ");
  }
</script>

<section class:active={robot.motion.active} class:faulted={Boolean(robot.cell.faultCode)} class="robot-live-view" aria-label="Live robot position">
  <header>
    <span><Route size={15} />Live cell position</span>
    <strong>{robot.cell.faultCode ?? (robot.motion.active ? `${robot.motion.progressPercent.toFixed(0)}% complete` : display(robot.automaticCommand))}</strong>
  </header>
  <div class="robot-scene">
    <svg viewBox="0 0 620 370" role="img" aria-label={`Robot TCP X ${robot.pose.x.toFixed(0)}, Y ${robot.pose.y.toFixed(0)}, Z ${robot.pose.z.toFixed(0)} millimetres`}>
      <path class="robot-cell-floor" d="M42 320 H582"></path>
      <path class="robot-fence" d="M34 42 V319 M586 42 V319 M34 42 H586" stroke-dasharray="8 7"></path>
      <path class="robot-motion-path" d={`M${tcpX} ${tcpY} L${targetX} ${targetY}`}></path>
      <circle class="robot-target" cx={targetX} cy={targetY} r="11"></circle>
      <g class="robot-arm" transform={`translate(${baseX} 304) rotate(${(yRatio - 0.5) * 24})`}>
        <path class="robot-base" d="M-46 16 H46 L34 -34 H-34 Z"></path>
        <circle class="robot-joint" cx="0" cy="-28" r="25"></circle>
        <g transform={`rotate(${shoulder})`}>
          <path class="robot-link" d="M0 -45 L0 -146"></path>
          <circle class="robot-joint" cx="0" cy="-151" r="21"></circle>
          <g transform={`translate(0 -151) rotate(${elbow})`}>
            <path class="robot-link secondary" d="M0 0 L100 0"></path>
            <circle class="robot-joint" cx="103" cy="0" r="17"></circle>
            <g transform={`translate(103 0) rotate(${wrist})`}>
              <path class="robot-wrist" d="M0 0 H45"></path>
              <path class:gripped={robot.gripperClosed} class="robot-gripper" d="M45 -19 V19 M45 -14 L66 -24 M45 14 L66 24"></path>
              {#if robot.gripperClosed}<path class="robot-piece" d="M67 -27 Q91 -19 89 3 Q87 25 67 28 Z"></path>{/if}
            </g>
          </g>
        </g>
      </g>
      <path class="robot-pickup-station" d="M62 292 H154 V320 H62 Z M84 292 V231 H135 V292"></path>
      <path class="robot-handoff-station" d="M458 270 H551 V320 H458 Z M476 270 V241 H533 V270"></path>
      <g class:open={guardedCell?.guard.position === "open"} class="robot-cell-gate">
        <path d="M586 244 V319 M586 244 H550"></path><circle cx="580" cy="254" r="5"></circle>
      </g>
      <g class="robot-station-registers">
        {#each robot.handoffs as handoff, index}
          <g class:active={activeHandoff?.mould === handoff.mould} transform={`translate(52 ${62 + index * 31})`}>
            <rect width="102" height="22"></rect><text x="8" y="15">{handoff.mould.toUpperCase()} PICK</text>
          </g>
          {@const transfer = guardedCell?.handoffStations.find((station) => station.mould === handoff.mould)}
          <g class:active={activeHandoff?.mould === handoff.mould} class="robot-transfer-register" transform={`translate(466 ${62 + index * 31})`}>
            <path d="M0 11 H102"></path>
            <rect x={Math.min(82, (transfer?.progressPercent ?? 0) * 0.82)} y="3" width="20" height="16"></rect>
            <text x="8" y="-3">{transfer ? display(transfer.state).toUpperCase() : "HANDOFF"}</text>
          </g>
        {/each}
      </g>
      <circle class="robot-tcp" cx={tcpX} cy={tcpY} r="7"></circle>
      <g class="robot-view-labels"><text x="68" y="217">SELECTED MOULD PICKUP</text><text x="458" y="227">ASSIGNED OPERATOR HANDOFF</text><text x="42" y="32">INTERLOCKED ROBOT CELL / FOUR-STATION ARBITRATION</text></g>
    </svg>
    <div class="robot-motion-meter">
      <span style={`--progress: ${robot.motion.progressPercent}%`}></span>
    </div>
  </div>
  <div class="robot-live-readouts">
    <span><Crosshair size={14} /><small>TCP position</small><strong>X {robot.pose.x.toFixed(0)} / Y {robot.pose.y.toFixed(0)} / Z {robot.pose.z.toFixed(0)}</strong></span>
    <span><Gauge size={14} /><small>Motion</small><strong>{robot.motion.active ? display(robot.motion.kind) : "in position"} / {robot.motion.speedPercent.toFixed(0)}%</strong></span>
    <span><Timer size={14} /><small>Move time</small><strong>{(robot.motion.elapsedMs / 1000).toFixed(1)} / {(robot.motion.durationMs / 1000).toFixed(1)} s</strong></span>
    <span><Hand size={14} /><small>Gripper</small><strong>{robot.gripperClosed ? "piece held" : "open"}</strong></span>
  </div>
</section>

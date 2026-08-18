<script lang="ts">
  import { CircleGauge, FlaskConical, LoaderCircle, ShieldAlert } from "@lucide/svelte";
  import type {
    HmiAction,
    HmiBodyPreparationState,
    HmiProcessFault,
    HmiProcessState,
    HmiSnapshot,
    BodyPreparationHmiScope,
  } from "../../hmi-api";

  export let snapshot: HmiSnapshot;
  export let body: HmiBodyPreparationState;
  export let process: HmiProcessState | null;
  export let scope: BodyPreparationHmiScope;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  const faultsByScope: Partial<Record<BodyPreparationHmiScope, { id: HmiProcessFault; label: string }[]>> = {
    slip: [
      { id: "ingredient-shortage", label: "Ingredient shortage" },
      { id: "mixer-overload", label: "Mixer overload" },
      { id: "screen-blocked", label: "Screen blocked" },
      { id: "quality-out-of-spec", label: "QC out of range" },
      { id: "transfer-no-flow", label: "Transfer no-flow" },
      { id: "slip-pipeline-leak", label: "Slip pipeline leak / air ingress" },
    ],
    "water-process": [
      { id: "raw-water-quality", label: "Raw-water quality" },
      { id: "water-filter-blocked", label: "Water filter blocked" },
      { id: "return-water-contamination", label: "Return cross-contamination" },
      { id: "water-to-slip-leak", label: "Slip-water branch leak" },
      { id: "water-to-glaze-leak", label: "Glaze-water branch leak" },
    ],
    glaze: [
      { id: "glaze-mill-overload", label: "Mill overload" },
      { id: "glaze-quality-out-of-spec", label: "QC out of range" },
      { id: "glaze-pipeline-leak", label: "Glaze pipeline leak" },
    ],
  };
  $: faults = faultsByScope[scope] ?? [];

  function display(value: string) {
    return value.replaceAll("-", " ");
  }
</script>

<div class="body-diagnostics">
  <section class="body-disturbances">
    <header><span><ShieldAlert size={15} />Simulation disturbances</span><small>One active process disturbance</small></header>
    <div>{#each faults as fault}<button type="button" class:active={process?.fault === fault.id} disabled={Boolean(busyTarget)} aria-pressed={process?.fault === fault.id} onclick={() => onExecute({ kind: "set-process-fault", fault: fault.id, active: process?.fault !== fault.id }, `fault-${fault.id}`)}>{#if busyTarget === `fault-${fault.id}`}<LoaderCircle class="spin" size={13} />{/if}{fault.label}</button>{/each}</div>
  </section>

  <section class="body-disposition-grid">
    {#if scope === "slip"}
      <article class:released={body.slip.qualityReleased}><FlaskConical size={18} /><span><small>Slip disposition</small><strong>{body.slip.qualityReleased ? "Released to Forming" : "Release pending"}</strong></span></article>
      <article class:released={!body.pipelines.slipToForming.leakDetected}><CircleGauge size={18} /><span><small>Pipeline balance</small><strong>{body.pipelines.slipToForming.lineLossPercent.toFixed(1)}% loss</strong></span></article>
      <article class:released={body.pipelines.slipToForming.entrainedAirPercent < 0.5}><CircleGauge size={18} /><span><small>Entrained air</small><strong>{body.pipelines.slipToForming.entrainedAirPercent.toFixed(2)}%</strong></span></article>
    {:else if scope === "water-process"}
      <article class:released={body.water.product.turbidityNtu <= 2}><CircleGauge size={18} /><span><small>Treated-water turbidity</small><strong>{body.water.product.turbidityNtu.toFixed(2)} NTU</strong></span></article>
      <article class:released={body.returnWater.bodyReuseQuality.glazeContaminationPercent <= 0.05}><CircleGauge size={18} /><span><small>Body-return contamination</small><strong>{body.returnWater.bodyReuseQuality.glazeContaminationPercent.toFixed(3)}%</strong></span></article>
      <article class:released={!body.pipelines.waterToSlip.leakDetected}><CircleGauge size={18} /><span><small>Slip-water branch</small><strong>{body.pipelines.waterToSlip.deliveredQualityPercent.toFixed(1)}%</strong></span></article>
      <article class:released={!body.pipelines.waterToGlaze.leakDetected}><CircleGauge size={18} /><span><small>Glaze-water branch</small><strong>{body.pipelines.waterToGlaze.deliveredQualityPercent.toFixed(1)}%</strong></span></article>
    {:else}
      <article class:released={body.glaze.qualityReleased}><FlaskConical size={18} /><span><small>Glaze disposition</small><strong>{body.glaze.qualityReleased ? "Released to glazing" : "Release pending"}</strong></span></article>
      <article class:released={!body.pipelines.glazeToGlazing.leakDetected}><CircleGauge size={18} /><span><small>Pipeline balance</small><strong>{body.pipelines.glazeToGlazing.lineLossPercent.toFixed(1)}% loss</strong></span></article>
      <article class:released={!body.pipelines.waterToGlaze.leakDetected}><CircleGauge size={18} /><span><small>Process-water delivery</small><strong>{body.pipelines.waterToGlaze.deliveredQualityPercent.toFixed(1)}%</strong></span></article>
    {/if}
  </section>

  <section class="body-output-list">
    <header><span><CircleGauge size={16} />Field output states</span><small>{snapshot.actuators.length} configured outputs</small></header>
    <div>{#each snapshot.actuators as actuator}<article class:active={actuator.currentState !== actuator.safeState}><i></i><span><strong>{actuator.label}</strong><small>{actuator.commandTag}</small></span><em>{display(actuator.currentState)}</em></article>{/each}</div>
  </section>
</div>

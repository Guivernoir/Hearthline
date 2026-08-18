<script lang="ts">
  import {
    Activity,
    Beaker,
    ClipboardList,
    Droplets,
    FlaskConical,
    Gauge,
    RotateCcw,
    Route,
    SlidersHorizontal,
  } from "@lucide/svelte";
  import type { Component } from "svelte";
  import type { BodyPreparationHmiScope, HmiAction, HmiSnapshot, WaterHmiScope } from "../hmi-api";
  import GlazePanel from "./panels/GlazePanel.svelte";
  import HandoffPanel from "./panels/HandoffPanel.svelte";
  import QualityIoPanel from "./panels/QualityIoPanel.svelte";
  import RecipePanel from "./panels/RecipePanel.svelte";
  import SlipPanel from "./panels/SlipPanel.svelte";
  import TrainControl from "./panels/TrainControl.svelte";
  import WaterWorkspace from "./water/WaterWorkspace.svelte";

  export let snapshot: HmiSnapshot;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  type Page = "overview" | "process" | "handoffs" | "recipe" | "diagnostics";
  interface ScopePresentation { label: string; processLabel: string; icon: Component<any>; }
  const scopePresentation: Record<BodyPreparationHmiScope, ScopePresentation> = {
    slip: { label: "Slip Preparation", processLabel: "Slip process", icon: Beaker },
    "water-process": { label: "Industrial-Water Process", processLabel: "Water process", icon: Droplets },
    "water-pipeline": { label: "Industrial-Water Distribution", processLabel: "Water routes", icon: Route },
    "return-water-process": { label: "Return-Water Process", processLabel: "Return process", icon: Droplets },
    "return-water-pipeline": { label: "Return-Water Pipelines", processLabel: "Return routes", icon: Route },
    glaze: { label: "Glaze Preparation", processLabel: "Glaze process", icon: FlaskConical },
  };

  let page: Page = "overview";
  $: body = snapshot.bodyPreparation;
  $: process = snapshot.process;
  $: scope = scopeFor(snapshot.controller);
  $: presentation = scopePresentation[scope];
  $: ProcessIcon = presentation.icon;
  $: safetyTripped = snapshot.safety.some((safety) => safety.tripLatched);

  function scopeFor(controller: string): BodyPreparationHmiScope {
    if (controller === "area-01-wt-vplc-01") return "water-process";
    if (controller === "area-01-wd-vplc-01") return "water-pipeline";
    if (controller === "area-01-rw-vplc-01") return "return-water-process";
    if (controller === "area-01-rc-vplc-01") return "return-water-pipeline";
    if (controller === "area-01-gl-vplc-01") return "glaze";
    return "slip";
  }

  function isWaterScope(value: BodyPreparationHmiScope): value is WaterHmiScope {
    return value !== "slip" && value !== "glaze";
  }
</script>

{#if body}
  {#if isWaterScope(scope)}
    <WaterWorkspace {snapshot} {scope} {busyTarget} {onExecute} />
  {:else}
  <section class="body-workspace" aria-label={`${presentation.label} control`}>
    <nav class="body-nav" aria-label={`${presentation.label} pages`}>
      <button type="button" class:active={page === "overview"} onclick={() => (page = "overview")}><Activity size={15} />Overview</button>
      <button type="button" class:active={page === "process"} onclick={() => (page = "process")}><ProcessIcon size={15} />{presentation.processLabel}</button>
      <button type="button" class:active={page === "handoffs"} onclick={() => (page = "handoffs")}><Route size={15} />Handoffs</button>
      <button type="button" class:active={page === "recipe"} onclick={() => (page = "recipe")}><ClipboardList size={15} />Recipes</button>
      <button type="button" class:active={page === "diagnostics"} onclick={() => (page = "diagnostics")}><SlidersHorizontal size={15} />Diagnostics</button>
    </nav>

    <header class="body-area-header">
      <div><small>Local control cell</small><strong>{presentation.label}</strong></div>
      <dl>
        <div><dt>Controller</dt><dd>{snapshot.controller}</dd></div>
        <div><dt>Local RIO</dt><dd>{snapshot.remoteIoStations.length}</dd></div>
        <div><dt>Safety</dt><dd class:bad={safetyTripped}>{safetyTripped ? "Reset required" : "Ready"}</dd></div>
      </dl>
      <button type="button" class="body-reset" disabled={process?.phase !== "faulted" || Boolean(busyTarget)} onclick={() => onExecute({ kind: "reset-process" }, "reset-process")}><RotateCcw size={14} />Reset process</button>
    </header>

    {#if page === "overview"}
      <div class="body-train-overview local">
        {#if scope === "slip"}
          <TrainControl train={body.slip.train} compact {safetyTripped} {busyTarget} {onExecute} />
        {:else}
          <TrainControl train={body.glaze.train} compact {safetyTripped} {busyTarget} {onExecute} />
        {/if}
      </div>
      <div class="body-overview-metrics">
        {#if scope === "slip"}
          <article><Beaker size={18} /><span><small>Slip quality index</small><strong>{body.slip.qualityIndex.toFixed(0)}%</strong></span></article>
          <article><Gauge size={18} /><span><small>Casting rate</small><strong>{body.slip.downstream.castingRateGCm2Min.toFixed(3)} g/cm2/min</strong></span></article>
          <article><Route size={18} /><span><small>Transfer line loss</small><strong>{body.pipelines.slipToForming.lineLossPercent.toFixed(1)}%</strong></span></article>
          <article><Activity size={18} /><span><small>Fired defect risk</small><strong>{body.slip.downstream.firedDefectRiskPercent.toFixed(1)}%</strong></span></article>
        {:else}
          <article><FlaskConical size={18} /><span><small>Glaze quality index</small><strong>{body.glaze.qualityIndex.toFixed(0)}%</strong></span></article>
          <article><Gauge size={18} /><span><small>Settling risk</small><strong>{body.glaze.settlingRiskPercent.toFixed(1)}%</strong></span></article>
          <article><Route size={18} /><span><small>Water branch loss</small><strong>{body.pipelines.waterToGlaze.lineLossPercent.toFixed(1)}%</strong></span></article>
          <article><Route size={18} /><span><small>Glaze line loss</small><strong>{body.pipelines.glazeToGlazing.lineLossPercent.toFixed(1)}%</strong></span></article>
        {/if}
      </div>
      <HandoffPanel pipelines={body.pipelines} {scope} compact />
    {:else if page === "process"}
      {#if scope === "slip"}<SlipPanel slip={body.slip} {safetyTripped} {busyTarget} {onExecute} />
      {:else}<GlazePanel glaze={body.glaze} {safetyTripped} {busyTarget} {onExecute} />{/if}
    {:else if page === "handoffs"}
      <HandoffPanel pipelines={body.pipelines} {scope} />
      {#if scope === "slip"}
        <section class="body-material-contract"><header><span>Slip quality effect received by Forming</span><small>Applied on batch release</small></header><dl><div><dt>Filling flow</dt><dd>{(body.slip.downstream.fillingFlowFactor * 100).toFixed(0)}%</dd></div><div><dt>Green moisture</dt><dd>{body.slip.downstream.predictedGreenMoisturePercent.toFixed(1)}%</dd></div><div><dt>Drying shrinkage</dt><dd>{body.slip.downstream.predictedDryingShrinkagePercent.toFixed(2)}%</dd></div><div><dt>Drying energy</dt><dd>{(body.slip.downstream.dryingEnergyFactor * 100).toFixed(0)}%</dd></div><div><dt>Green strength</dt><dd>{body.slip.downstream.greenStrengthIndex.toFixed(0)}%</dd></div><div><dt>Fired defect risk</dt><dd>{body.slip.downstream.firedDefectRiskPercent.toFixed(1)}%</dd></div></dl></section>
      {/if}
    {:else if page === "recipe"}
      <RecipePanel {snapshot} {body} {scope} {busyTarget} {onExecute} />
    {:else}
      <QualityIoPanel {snapshot} {body} {process} {scope} {busyTarget} {onExecute} />
    {/if}
  </section>
  {/if}
{/if}

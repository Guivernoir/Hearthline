<script lang="ts">
  import { Activity, Route, TriangleAlert, Wind } from "@lucide/svelte";
  import type {
    HmiBodyPreparationPipelineState,
    HmiHandoffPipelineState,
    BodyPreparationHmiScope,
  } from "../../hmi-api";

  export let pipelines: HmiBodyPreparationPipelineState;
  export let scope: BodyPreparationHmiScope;
  export let compact = false;

  interface PipelineRow {
    id: string;
    label: string;
    route: string;
    state: HmiHandoffPipelineState;
    tracksAir: boolean;
  }

  $: rows = relevantRows(pipelines, scope);
  $: anyLeak = rows.some((row) => row.state.leakDetected);

  function relevantRows(
    value: HmiBodyPreparationPipelineState,
    localScope: BodyPreparationHmiScope,
  ): PipelineRow[] {
    const all: Partial<Record<BodyPreparationHmiScope, PipelineRow[]>> = {
      slip: [
        { id: "water-slip", label: "Process water delivery", route: "Water cell to slip charge", state: value.waterToSlip, tracksAir: false },
        { id: "slip-forming", label: "Released slip handoff", route: "Slip cell to Forming", state: value.slipToForming, tracksAir: true },
      ],
      "water-pipeline": [
        { id: "water-slip", label: "Slip-water branch", route: "Treated tank to Slip Preparation", state: value.waterToSlip, tracksAir: false },
        { id: "water-glaze", label: "Glaze-water branch", route: "Treated tank to Glaze Preparation", state: value.waterToGlaze, tracksAir: false },
      ],
      glaze: [
        { id: "water-glaze", label: "Process water delivery", route: "Water cell to glaze charge", state: value.waterToGlaze, tracksAir: false },
        { id: "glaze-process", label: "Released glaze handoff", route: "Glaze cell to glazing process", state: value.glazeToGlazing, tracksAir: false },
      ],
    };
    return all[localScope] ?? [];
  }
</script>

<section class:compact class="body-handoff-panel">
  <header>
    <span><Route size={16} />Monitored process handoffs</span>
    <strong class:bad={anyLeak}>{anyLeak ? "Abnormal balance" : "Routes healthy"}</strong>
  </header>
  <div class="body-handoff-grid">
    {#each rows as row (row.id)}
      <article class:leak={row.state.leakDetected}>
        <header><span>{#if row.state.leakDetected}<TriangleAlert size={15} />{:else}<Activity size={15} />{/if}<strong>{row.label}</strong></span><small>{row.route}</small></header>
        <dl>
          <div><dt>Flow in / out</dt><dd>{row.state.inletFlowLMin.toFixed(1)} / {row.state.outletFlowLMin.toFixed(1)} L/min</dd></div>
          <div><dt>Pressure in / out</dt><dd>{row.state.inletPressureBar.toFixed(2)} / {row.state.outletPressureBar.toFixed(2)} bar</dd></div>
          <div><dt>Line loss</dt><dd>{row.state.lineLossPercent.toFixed(1)}%</dd></div>
          <div><dt>Delivered quality</dt><dd>{row.state.deliveredQualityPercent.toFixed(1)}%</dd></div>
          {#if row.tracksAir}<div class="air"><dt><Wind size={12} />Entrained air</dt><dd>{row.state.entrainedAirPercent.toFixed(2)}%</dd></div>{/if}
        </dl>
      </article>
    {/each}
  </div>
  {#if scope === "slip"}
    <p class:bad={pipelines.slipToForming.leakDetected}>The released-slip pipeline contract is applied to Forming filling flow, casting rate, green strength, and fired-defect risk.</p>
  {:else if scope === "water-pipeline"}
    <p class:bad={anyLeak}>Separate branch balances expose whether Slip and Glaze receive the volume and pressure released by the Water cell.</p>
  {:else}
    <p class:bad={pipelines.glazeToGlazing.leakDetected}>The Glaze HMI tracks both incoming process water and the released suspension delivered to the glazing process.</p>
  {/if}
</section>

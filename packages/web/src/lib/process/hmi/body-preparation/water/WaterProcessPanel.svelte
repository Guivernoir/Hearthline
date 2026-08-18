<script lang="ts">
  import { Activity, Beaker, CircleGauge, Droplets, FlaskConical } from "@lucide/svelte";
  import type {
    HmiAction,
    HmiBodyPreparationState,
    HmiProcessFault,
    WaterHmiScope,
  } from "../../hmi-api";
  import TrainControl from "../panels/TrainControl.svelte";

  export let body: HmiBodyPreparationState;
  export let scope: WaterHmiScope;
  export let safetyTripped = false;
  export let busyTarget = "";
  export let diagnostics = false;
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  const industrialRows = [
    { label: "Temperature", key: "temperatureC", unit: "degC" },
    { label: "pH", key: "ph", unit: "pH" },
    { label: "Turbidity", key: "turbidityNtu", unit: "NTU" },
    { label: "Specific conductance", key: "conductivityUsCm", unit: "uS/cm" },
    { label: "Hardness", key: "hardnessMgLCaco3", unit: "mg/L CaCO3" },
    { label: "Suspended solids", key: "suspendedSolidsMgL", unit: "mg/L" },
  ] as const;
  const processFaults: Record<"industrial" | "return", { id: HmiProcessFault; label: string }[]> = {
    industrial: [
      { id: "raw-water-quality", label: "Raw analyzer deviation" },
      { id: "water-filter-blocked", label: "Filter differential high" },
    ],
    return: [{ id: "return-water-contamination", label: "Return segregation deviation" }],
  };

  $: returnMode = scope === "return-water-process";
  $: train = returnMode ? body.returnWater.train : body.water.train;
  $: faults = processFaults[returnMode ? "return" : "industrial"];
</script>

<div class="water-process-layout">
  <TrainControl {train} {safetyTripped} {busyTarget} {onExecute} />

  {#if returnMode}
    <section class="water-process-block">
      <header><span><Activity size={16} />Return-water train</span><small>{body.returnWater.activeStream.replaceAll("-", " ")}</small></header>
      <dl class="water-process-metrics">
        <div><dt>Body equalization</dt><dd>{body.returnWater.bodyEqualizationL.toFixed(0)} L</dd></div>
        <div><dt>Glaze equalization</dt><dd>{body.returnWater.glazeEqualizationL.toFixed(0)} L</dd></div>
        <div><dt>Feed flow</dt><dd>{body.returnWater.feedFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Clarified flow</dt><dd>{body.returnWater.clarifiedFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Influent turbidity</dt><dd>{body.returnWater.influentTurbidityNtu.toFixed(1)} NTU</dd></div>
        <div><dt>Effluent turbidity</dt><dd>{body.returnWater.effluentTurbidityNtu.toFixed(2)} NTU</dd></div>
      </dl>
    </section>
    <section class="water-process-block water-reading-table">
      <header><span><FlaskConical size={16} />Recovered-water analyzer readings</span><small>Direct values by segregated stream</small></header>
      <table>
        <thead><tr><th>Reading</th><th>Body reuse</th><th>Glaze reuse</th></tr></thead>
        <tbody>
          {#each industrialRows as row}
            <tr><th>{row.label}</th><td>{body.returnWater.bodyReuseQuality[row.key].toFixed(2)} {row.unit}</td><td>{body.returnWater.glazeReuseQuality[row.key].toFixed(2)} {row.unit}</td></tr>
          {/each}
        </tbody>
      </table>
    </section>
  {:else}
    <section class="water-process-block">
      <header><span><Droplets size={16} />Industrial-water treatment</span><small>RO recovery {body.water.roRecoveryPercent.toFixed(0)}%</small></header>
      <dl class="water-process-metrics">
        <div><dt>Raw-water tank</dt><dd>{body.water.rawTankL.toFixed(0)} L</dd></div>
        <div><dt>Treated-water tank</dt><dd>{body.water.treatedTankL.toFixed(0)} L</dd></div>
        <div><dt>Feed flow</dt><dd>{body.water.feedFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Permeate flow</dt><dd>{body.water.permeateFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Reject flow</dt><dd>{body.water.rejectFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Filter DP</dt><dd>{body.water.mediaFilterDpBar.toFixed(2)} bar</dd></div>
      </dl>
    </section>
    <section class="water-process-block water-reading-table">
      <header><span><Beaker size={16} />Water analyzer readings</span><small>Raw and treated sample stations</small></header>
      <table>
        <thead><tr><th>Reading</th><th>Raw</th><th>Treated</th></tr></thead>
        <tbody>
          {#each industrialRows as row}
            <tr><th>{row.label}</th><td>{body.water.raw[row.key].toFixed(2)} {row.unit}</td><td>{body.water.product[row.key].toFixed(2)} {row.unit}</td></tr>
          {/each}
        </tbody>
      </table>
    </section>
  {/if}

  {#if diagnostics}
    <section class="water-process-block water-diagnostics">
      <header><span><CircleGauge size={16} />Simulation diagnostics</span><small>Development-only disturbances</small></header>
      <div class="water-diagnostic-actions">
        {#each faults as fault}
          <div><span>{fault.label}</span><button type="button" disabled={Boolean(busyTarget)} onclick={() => onExecute({ kind: "set-process-fault", fault: fault.id, active: true }, fault.id)}>Inject</button><button type="button" disabled={Boolean(busyTarget)} onclick={() => onExecute({ kind: "set-process-fault", fault: fault.id, active: false }, fault.id)}>Clear</button></div>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .water-process-layout { display: grid; min-width: 0; gap: 9px; }
  .water-process-block { min-width: 0; border: 1px solid #d1dbd6; border-radius: 3px; background: #f8faf9; }
  .water-process-block > header {
    display: flex;
    min-height: 38px;
    padding: 0 10px;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    border-bottom: 1px solid #d7dfdb;
  }
  .water-process-block > header span { display: flex; align-items: center; gap: 6px; color: #2f6570; font-size: 10px; font-weight: 850; }
  .water-process-block > header small { color: #75817a; font-size: 8px; }
  .water-process-metrics { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); margin: 0; gap: 1px; background: #dce3df; }
  .water-process-metrics div { min-height: 58px; padding: 9px; background: #f8faf9; }
  .water-process-metrics dt { color: #717e77; font-size: 8px; }
  .water-process-metrics dd { margin: 7px 0 0; color: #2e6358; font-size: 12px; font-weight: 850; }
  .water-reading-table { overflow-x: auto; }
  .water-reading-table table { width: 100%; min-width: 570px; border-collapse: collapse; font-size: 9px; }
  .water-reading-table th,
  .water-reading-table td { height: 35px; padding: 5px 9px; border-bottom: 1px solid #e1e6e3; text-align: right; }
  .water-reading-table th:first-child { text-align: left; }
  .water-reading-table thead th { color: #6e7c75; background: #edf2ef; font-size: 7px; text-transform: uppercase; }
  .water-reading-table tbody th { color: #495850; font-weight: 750; }
  .water-reading-table tbody td { color: #2b5f67; font-weight: 800; }
  .water-diagnostic-actions { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 6px; padding: 8px; }
  .water-diagnostic-actions > div { display: grid; min-height: 40px; grid-template-columns: minmax(130px, 1fr) 54px 54px; align-items: center; gap: 5px; border-bottom: 1px solid #e1e6e3; }
  .water-diagnostic-actions span { color: #4a5750; font-size: 9px; font-weight: 750; }
  .water-diagnostic-actions button { min-height: 27px; border: 1px solid #b6c2bb; border-radius: 3px; color: #42524a; background: #fff; font-size: 8px; font-weight: 800; cursor: pointer; }
  .water-diagnostic-actions button:first-of-type { border-color: #a76b46; color: #8a492c; }
  .water-diagnostic-actions button:disabled { opacity: .45; cursor: not-allowed; }
  @media (max-width: 900px) {
    .water-process-metrics { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }
  @media (max-width: 600px) {
    .water-process-block > header { padding: 7px 8px; align-items: flex-start; flex-direction: column; }
    .water-process-metrics,
    .water-diagnostic-actions { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .water-diagnostic-actions > div { grid-template-columns: minmax(0, 1fr); padding: 6px 0; }
  }
</style>

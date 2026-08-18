<script lang="ts">
  import { Droplets, Filter, Recycle, Waves } from "@lucide/svelte";
  import type {
    HmiAction,
    HmiReturnWaterState,
    HmiWaterPreparationState,
    HmiWaterQuality,
  } from "../../hmi-api";
  import TrainControl from "./TrainControl.svelte";

  export let water: HmiWaterPreparationState;
  export let returns: HmiReturnWaterState;
  export let safetyTripped = false;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  const waterRows: { label: string; key: keyof HmiWaterQuality; unit: string }[] = [
    { label: "pH", key: "ph", unit: "" },
    { label: "Turbidity", key: "turbidityNtu", unit: "NTU" },
    { label: "Conductivity", key: "conductivityUsCm", unit: "uS/cm" },
    { label: "Hardness", key: "hardnessMgLCaco3", unit: "mg/L CaCO3" },
    { label: "Suspended solids", key: "suspendedSolidsMgL", unit: "mg/L" },
  ];
</script>

<div class="body-detail-page">
  <TrainControl train={water.train} {safetyTripped} {busyTarget} {onExecute} />
  <div class="body-detail-columns">
    <section class="body-value-panel">
      <header><span><Filter size={15} />Water-treatment train</span><small>RO recovery {water.roRecoveryPercent.toFixed(0)}%</small></header>
      <dl>
        <div><dt>Raw-water tank</dt><dd>{water.rawTankL.toFixed(0)} L</dd></div>
        <div><dt>Treated-water tank</dt><dd>{water.treatedTankL.toFixed(0)} L</dd></div>
        <div><dt>Feed flow</dt><dd>{water.feedFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Permeate flow</dt><dd>{water.permeateFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Reject flow</dt><dd>{water.rejectFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Media-filter DP</dt><dd>{water.mediaFilterDpBar.toFixed(2)} bar</dd></div>
      </dl>
    </section>
    <section class="body-water-quality">
      <header><span><Droplets size={15} />Water quality</span><small>Raw / treated inventory</small></header>
      <table><thead><tr><th>Measure</th><th>Raw</th><th>Treated</th></tr></thead><tbody>{#each waterRows as row}<tr><th>{row.label}</th><td>{water.raw[row.key].toFixed(2)} {row.unit}</td><td>{water.product[row.key].toFixed(2)} {row.unit}</td></tr>{/each}</tbody></table>
    </section>
  </div>

  <TrainControl train={returns.train} {safetyTripped} {busyTarget} {onExecute} />
  <div class="body-detail-columns">
    <section class="body-value-panel">
      <header><span><Recycle size={15} />Segregated return-water recovery</span><small>{returns.activeStream.replaceAll("-", " ")}</small></header>
      <dl>
        <div><dt>Body equalization</dt><dd>{returns.bodyEqualizationL.toFixed(0)} L</dd></div>
        <div><dt>Glaze equalization</dt><dd>{returns.glazeEqualizationL.toFixed(0)} L</dd></div>
        <div><dt>Body reuse tank</dt><dd>{returns.bodyReuseTankL.toFixed(0)} L</dd></div>
        <div><dt>Glaze reuse tank</dt><dd>{returns.glazeReuseTankL.toFixed(0)} L</dd></div>
        <div><dt>Clarified flow</dt><dd>{returns.clarifiedFlowLMin.toFixed(1)} L/min</dd></div>
        <div><dt>Filter-press cake</dt><dd>{returns.sludgeCakeKg.toFixed(2)} kg</dd></div>
      </dl>
    </section>
    <section class="body-return-routing">
      <header><span><Waves size={15} />Reuse routing</span><small>Streams remain isolated</small></header>
      <div><article><strong>Body return</strong><span>{returns.bodyReuseQuality.turbidityNtu.toFixed(2)} NTU</span><small>{returns.bodyReuseQuality.conductivityUsCm.toFixed(0)} uS/cm · {returns.bodyReuseQuality.glazeContaminationPercent.toFixed(2)}% glaze carryover</small></article><article><strong>Glaze return</strong><span>{returns.glazeReuseQuality.turbidityNtu.toFixed(2)} NTU</span><small>{returns.glazeReuseQuality.conductivityUsCm.toFixed(0)} uS/cm · dedicated glaze route</small></article></div>
    </section>
  </div>
</div>

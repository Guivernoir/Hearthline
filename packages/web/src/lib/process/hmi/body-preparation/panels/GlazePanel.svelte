<script lang="ts">
  import { Beaker, FlaskConical, Gauge, Scale } from "@lucide/svelte";
  import type { HmiAction, HmiGlazePreparationState } from "../../hmi-api";
  import TrainControl from "./TrainControl.svelte";

  export let glaze: HmiGlazePreparationState;
  export let safetyTripped = false;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  function percent(actual: number, target: number) {
    return target <= 0 ? 0 : Math.min(100, Math.max(0, actual / target * 100));
  }
</script>

<div class="body-detail-page">
  <TrainControl train={glaze.train} {safetyTripped} {busyTarget} {onExecute} />
  <div class="body-detail-columns">
    <section class="body-charge-list glaze">
      <header><span><Scale size={15} />Glaze recipe charge</span><small>{glaze.powderMassKg.toFixed(1)} / {glaze.targetPowderMassKg.toFixed(1)} kg dry</small></header>
      {#each glaze.ingredients as ingredient}
        <div><span><strong>{ingredient.label}</strong><small>{ingredient.actualKg.toFixed(2)} / {ingredient.targetKg.toFixed(2)} kg</small></span><i><b style={`width: ${percent(ingredient.actualKg, ingredient.targetKg)}%`}></b></i></div>
      {/each}
    </section>
    <section class="body-value-panel">
      <header><span><Gauge size={15} />Glaze suspension</span><small>Quality {glaze.qualityIndex.toFixed(0)}%</small></header>
      <dl>
        <div><dt>Batch mass</dt><dd>{glaze.batchMassKg.toFixed(1)} kg</dd></div>
        <div><dt>Solids</dt><dd>{glaze.solidsPercent.toFixed(1)}%</dd></div>
        <div><dt>Density</dt><dd>{glaze.densityKgL.toFixed(3)} kg/L</dd></div>
        <div><dt>Ford-cup flow</dt><dd>{glaze.fordCupSeconds.toFixed(1)} s</dd></div>
        <div><dt>63 um residue</dt><dd>{glaze.residue63umPercent.toFixed(2)}%</dd></div>
        <div><dt>Median particle</dt><dd>{glaze.medianParticleUm.toFixed(1)} um</dd></div>
        <div><dt>Mill energy</dt><dd>{glaze.millEnergyKwhT.toFixed(1)} kWh/t</dd></div>
        <div><dt>Settling risk</dt><dd>{glaze.settlingRiskPercent.toFixed(1)}%</dd></div>
        <div><dt>Storage level</dt><dd>{glaze.storageLevelPercent.toFixed(1)}%</dd></div>
        <div><dt>Transfer flow</dt><dd>{glaze.transferFlowLMin.toFixed(1)} L/min</dd></div>
      </dl>
    </section>
  </div>
  <section class="body-quality-checks">
    <header><span><FlaskConical size={15} />Glaze release</span><strong class:released={glaze.qualityReleased}>{glaze.qualityReleased ? "Released" : "Pending"}</strong></header>
    <div>{#each glaze.qualityChecks as check}<article class:within={check.withinLimit}><i></i><span><small>{check.label}</small><strong>{check.value.toFixed(2)} {check.unit}</strong><em>{check.minimum}-{check.maximum} {check.unit}</em></span></article>{/each}<article class:within={glaze.water.conductivityUsCm <= 500}><i></i><span><small>Charge-water conductivity</small><strong>{glaze.water.conductivityUsCm.toFixed(0)} uS/cm</strong><em>Maximum 500 uS/cm</em></span></article></div>
  </section>
</div>

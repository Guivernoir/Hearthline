<script lang="ts">
  import { Beaker, Gauge, Route, Scale } from "@lucide/svelte";
  import type { HmiAction, HmiSlipPreparationState } from "../../hmi-api";
  import TrainControl from "./TrainControl.svelte";

  export let slip: HmiSlipPreparationState;
  export let safetyTripped = false;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  function percent(actual: number, target: number) {
    return target <= 0 ? 0 : Math.min(100, Math.max(0, actual / target * 100));
  }
</script>

<div class="body-detail-page">
  <TrainControl train={slip.train} {safetyTripped} {busyTarget} {onExecute} />
  <div class="body-detail-columns">
    <section class="body-charge-list">
      <header><span><Scale size={15} />Slip recipe charge</span><small>{slip.batchMassKg.toFixed(1)} / {slip.targetBatchMassKg.toFixed(1)} kg</small></header>
      {#each slip.ingredients as ingredient}
        <div><span><strong>{ingredient.label}</strong><small>{ingredient.actualKg.toFixed(2)} / {ingredient.targetKg.toFixed(2)} kg</small></span><i><b style={`width: ${percent(ingredient.actualKg, ingredient.targetKg)}%`}></b></i></div>
      {/each}
    </section>
    <section class="body-value-panel">
      <header><span><Gauge size={15} />Live rheology and process</span><small>Quality {slip.qualityIndex.toFixed(0)}%</small></header>
      <dl>
        <div><dt>Density</dt><dd>{slip.densityKgL.toFixed(3)} kg/L</dd></div>
        <div><dt>Solids</dt><dd>{slip.solidsPercent.toFixed(1)}%</dd></div>
        <div><dt>High-shear viscosity</dt><dd>{slip.highShearViscosityMpaS.toFixed(0)} mPa.s</dd></div>
        <div><dt>Low-shear viscosity</dt><dd>{slip.lowShearViscosityMpaS.toFixed(0)} mPa.s</dd></div>
        <div><dt>Thixotropic index</dt><dd>{slip.thixotropicIndex.toFixed(2)}</dd></div>
        <div><dt>Temperature</dt><dd>{slip.temperatureC.toFixed(1)} C</dd></div>
        <div><dt>44 um residue</dt><dd>{slip.residue44umPercent.toFixed(2)}%</dd></div>
        <div><dt>Median particle</dt><dd>{slip.medianParticleUm.toFixed(1)} um</dd></div>
        <div><dt>Mixing energy</dt><dd>{slip.specificEnergyKwhT.toFixed(2)} kWh/t</dd></div>
        <div><dt>Casting rate</dt><dd>{slip.castingRateGCm2Min.toFixed(3)} g/cm2/min</dd></div>
      </dl>
    </section>
  </div>
  <div class="body-detail-columns">
    <section class="body-quality-checks">
      <header><span><Beaker size={15} />Slip release</span><strong class:released={slip.qualityReleased}>{slip.qualityReleased ? "Released" : "Pending"}</strong></header>
      <div>{#each slip.qualityChecks as check}<article class:within={check.withinLimit}><i></i><span><small>{check.label}</small><strong>{check.value.toFixed(2)} {check.unit}</strong><em>{check.minimum}-{check.maximum} {check.unit}</em></span></article>{/each}</div>
    </section>
    <section class="body-value-panel">
      <header><span><Route size={15} />Downstream prediction</span><small>Applied on batch release</small></header>
      <dl>
        <div><dt>Filling-flow factor</dt><dd>{(slip.downstream.fillingFlowFactor * 100).toFixed(0)}%</dd></div>
        <div><dt>Green moisture</dt><dd>{slip.downstream.predictedGreenMoisturePercent.toFixed(1)}%</dd></div>
        <div><dt>Drying shrinkage</dt><dd>{slip.downstream.predictedDryingShrinkagePercent.toFixed(2)}%</dd></div>
        <div><dt>Drying energy factor</dt><dd>{(slip.downstream.dryingEnergyFactor * 100).toFixed(0)}%</dd></div>
        <div><dt>Green strength index</dt><dd>{slip.downstream.greenStrengthIndex.toFixed(0)}%</dd></div>
        <div><dt>Fired defect risk</dt><dd>{slip.downstream.firedDefectRiskPercent.toFixed(1)}%</dd></div>
      </dl>
    </section>
  </div>
</div>

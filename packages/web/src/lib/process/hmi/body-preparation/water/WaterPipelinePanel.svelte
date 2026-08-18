<script lang="ts">
  import { ArrowRight, CircleGauge, Droplets, Gauge, Waves } from "@lucide/svelte";
  import type { HmiWaterRouteState } from "../../hmi-api";

  export let routes: HmiWaterRouteState[] = [];

  function balance(route: HmiWaterRouteState): number {
    if (route.inletFlowLMin <= 0) return 0;
    return Math.max(0, (route.inletFlowLMin - route.outletFlowLMin) / route.inletFlowLMin * 100);
  }
</script>

<section class="water-route-board" aria-label="Water pipeline routes">
  <header><span><Waves size={17} />Monitored routes</span><small>Source, destination, hydraulics, and analyzer readings</small></header>
  <div class="water-route-list">
    {#each routes as route}
      <article class:warning={route.leakDetected} class:unavailable={!route.available}>
        <div class="water-route-title"><span><Droplets size={16} /><strong>{route.label}</strong></span><small>{route.available ? (route.demanded ? "Flowing" : "Pressurized / idle") : "Unavailable"}</small></div>
        <div class="water-route-path"><span>{route.source}</span><ArrowRight size={15} /><span>{route.destination}</span></div>
        <dl>
          <div><dt><Gauge size={13} />Pressure</dt><dd>{route.inletPressureBar.toFixed(2)} / {route.outletPressureBar.toFixed(2)} bar</dd></div>
          <div><dt><Waves size={13} />Flow</dt><dd>{route.inletFlowLMin.toFixed(1)} / {route.outletFlowLMin.toFixed(1)} L/min</dd></div>
          <div><dt><CircleGauge size={13} />Balance loss</dt><dd>{balance(route).toFixed(1)}%</dd></div>
          <div><dt>pH</dt><dd>{route.quality.ph.toFixed(2)}</dd></div>
          <div><dt>Conductivity</dt><dd>{route.quality.conductivityUsCm.toFixed(0)} uS/cm</dd></div>
          <div><dt>Turbidity</dt><dd>{route.quality.turbidityNtu.toFixed(2)} NTU</dd></div>
        </dl>
      </article>
    {/each}
  </div>
</section>

<style>
  .water-route-board { min-width: 0; }
  .water-route-board > header {
    display: flex;
    min-height: 38px;
    align-items: center;
    justify-content: space-between;
    gap: 9px;
    border-bottom: 1px solid #d2dcd7;
  }
  .water-route-board > header span { display: flex; align-items: center; gap: 6px; color: #315f69; font-size: 10px; font-weight: 850; }
  .water-route-board > header small { color: #75817a; font-size: 8px; }
  .water-route-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; padding-top: 8px; }
  .water-route-list article { overflow: hidden; border: 1px solid #ccd8d3; border-left: 5px solid #327b89; border-radius: 3px; background: #f8faf9; }
  .water-route-list article.warning { border-color: #ad6558; border-left-color: #a33f33; background: #fbf3f1; }
  .water-route-list article.unavailable { border-left-color: #8b7040; }
  .water-route-title { display: flex; min-height: 43px; padding: 7px 9px; align-items: center; justify-content: space-between; gap: 8px; border-bottom: 1px solid #dce3df; }
  .water-route-title span { display: flex; min-width: 0; align-items: center; gap: 6px; color: #2e6570; }
  .water-route-title strong { overflow: hidden; font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .water-route-title small { color: #587468; font-size: 8px; font-weight: 800; }
  .water-route-path { display: grid; min-height: 38px; padding: 6px 9px; grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr); align-items: center; gap: 7px; color: #55645d; background: #eef3f1; font-size: 8px; font-weight: 750; }
  .water-route-path span:last-child { text-align: right; }
  .water-route-list dl { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); margin: 0; gap: 1px; background: #dce3df; }
  .water-route-list dl div { min-height: 51px; padding: 7px; background: #f8faf9; }
  .water-route-list dt { display: flex; align-items: center; gap: 4px; color: #738078; font-size: 7px; }
  .water-route-list dd { margin: 6px 0 0; color: #315e56; font-size: 9px; font-weight: 850; }
  @media (max-width: 850px) { .water-route-list { grid-template-columns: minmax(0, 1fr); } }
  @media (max-width: 520px) {
    .water-route-board > header { padding: 7px 0; align-items: flex-start; flex-direction: column; }
    .water-route-list dl { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
</style>

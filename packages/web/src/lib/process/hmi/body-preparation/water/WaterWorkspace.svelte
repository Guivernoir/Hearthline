<script lang="ts">
  import { Activity, Bell, Droplets, Gauge, Radio, RotateCcw, Route, SlidersHorizontal } from "@lucide/svelte";
  import type { Component } from "svelte";
  import type { HmiAction, HmiSnapshot, WaterHmiScope } from "../../hmi-api";
  import PumpFleetPanel from "./PumpFleetPanel.svelte";
  import WaterPipelinePanel from "./WaterPipelinePanel.svelte";
  import WaterProcessPanel from "./WaterProcessPanel.svelte";

  export let snapshot: HmiSnapshot;
  export let scope: WaterHmiScope;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  type Page = "overview" | "process" | "routes" | "pumps" | "diagnostics";
  interface Presentation { label: string; process: boolean; network: "industrial" | "return"; icon: Component<any>; }
  const presentations: Record<WaterHmiScope, Presentation> = {
    "water-process": { label: "Industrial-Water Process", process: true, network: "industrial", icon: Droplets },
    "water-pipeline": { label: "Industrial-Water Distribution", process: false, network: "industrial", icon: Route },
    "return-water-process": { label: "Return-Water Process", process: true, network: "return", icon: Droplets },
    "return-water-pipeline": { label: "Return-Water Pipelines", process: false, network: "return", icon: Route },
  };

  let page: Page = "overview";
  $: body = snapshot.bodyPreparation;
  $: presentation = presentations[scope];
  $: ScopeIcon = presentation.icon;
  $: routes = body?.waterNetworks.routes.filter((route) => route.network === presentation.network) ?? [];
  $: pumpPrefix = presentation.network === "industrial" ? "area-01-wd-pmp-" : "area-01-rc-pmp-";
  $: pumps = body?.waterNetworks.pumps.filter((pump) => pump.id.startsWith(pumpPrefix)) ?? [];
  $: heartbeatFaults = pumps.filter((pump) => !pump.heartbeatOk).length;
  $: safety = snapshot.safety[0];
  $: safetyTripped = Boolean(safety?.tripLatched);
  $: activeAlarms = snapshot.alarms.filter((alarm) => alarm.active);
</script>

{#if body}
  <section class="body-workspace water-workspace" aria-label={`${presentation.label} control`}>
    <nav class="body-nav" aria-label={`${presentation.label} pages`}>
      <button type="button" class:active={page === "overview"} onclick={() => (page = "overview")}><Activity size={15} />Overview</button>
      {#if presentation.process}
        <button type="button" class:active={page === "process"} onclick={() => (page = "process")}><ScopeIcon size={15} />Process</button>
      {:else}
        <button type="button" class:active={page === "routes"} onclick={() => (page = "routes")}><Route size={15} />Routes</button>
        <button type="button" class:active={page === "pumps"} onclick={() => (page = "pumps")}><Radio size={15} />Pumps</button>
      {/if}
      <button type="button" class:active={page === "diagnostics"} onclick={() => (page = "diagnostics")}><SlidersHorizontal size={15} />Diagnostics</button>
    </nav>

    <header class="body-area-header water-area-header">
      <div><small>Water Preparation building</small><strong>{presentation.label}</strong></div>
      <dl>
        <div><dt>Controller</dt><dd>{snapshot.controller}</dd></div>
        <div><dt>Local RIO</dt><dd>{snapshot.remoteIoStations.join(", ")}</dd></div>
        <div><dt>Safety</dt><dd class:bad={safetyTripped}>{safetyTripped ? "Reset required" : "Ready"}</dd></div>
      </dl>
      {#if safety}
        <button type="button" class="body-reset" disabled={!safety.tripLatched || Boolean(busyTarget)} onclick={() => onExecute({ kind: "reset-safety", safetyId: safety.componentId }, safety.componentId)}><RotateCcw size={14} />Reset safety</button>
      {/if}
    </header>

    {#if page === "overview"}
      <div class="water-overview-strip">
        <article><Gauge size={18} /><span><small>{presentation.process ? "Process phase" : "Available routes"}</small><strong>{presentation.process ? (scope === "water-process" ? body.water.train.phase : body.returnWater.train.phase).replaceAll("-", " ") : `${routes.filter((route) => route.available).length} / ${routes.length}`}</strong></span></article>
        <article><Radio size={18} /><span><small>Pump heartbeat losses</small><strong>{heartbeatFaults}</strong></span></article>
        <article><Bell size={18} /><span><small>Active alarms</small><strong>{activeAlarms.length}</strong></span></article>
        <article><Route size={18} /><span><small>{presentation.network === "industrial" ? "Header pressure" : "Collection flow"}</small><strong>{presentation.network === "industrial" ? `${routes[0]?.outletPressureBar.toFixed(2) ?? "0.00"} bar` : `${routes.slice(0, 2).reduce((sum, route) => sum + route.outletFlowLMin, 0).toFixed(1)} L/min`}</strong></span></article>
      </div>
      {#if presentation.process}
        <WaterProcessPanel {body} {scope} {safetyTripped} {busyTarget} {onExecute} />
      {:else}
        <WaterPipelinePanel routes={routes.slice(0, 2)} />
      {/if}
    {:else if page === "process"}
      <WaterProcessPanel {body} {scope} {safetyTripped} {busyTarget} {onExecute} />
    {:else if page === "routes"}
      <WaterPipelinePanel {routes} />
    {:else if page === "pumps"}
      <PumpFleetPanel {pumps} heartbeatTimeoutMs={body.waterNetworks.heartbeatTimeoutMs} {busyTarget} {onExecute} />
    {:else if presentation.process}
      <WaterProcessPanel {body} {scope} {safetyTripped} {busyTarget} diagnostics {onExecute} />
    {:else}
      <PumpFleetPanel {pumps} heartbeatTimeoutMs={body.waterNetworks.heartbeatTimeoutMs} {busyTarget} diagnostics {onExecute} />
    {/if}
  </section>
{/if}

<style>
  .water-workspace { gap: 10px; }
  .water-area-header { border-left: 4px solid #287789; padding-left: 11px; }
  .water-overview-strip {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 7px;
  }
  .water-overview-strip article {
    display: flex;
    min-height: 64px;
    padding: 9px 10px;
    align-items: center;
    gap: 9px;
    border: 1px solid #d2dcd7;
    border-left: 4px solid #287789;
    border-radius: 3px;
    color: #296a77;
    background: #f6f9f8;
  }
  .water-overview-strip article:nth-child(2) { border-left-color: #49775c; color: #416d52; }
  .water-overview-strip article:nth-child(3) { border-left-color: #9b6b36; color: #8b5a2a; }
  .water-overview-strip article:nth-child(4) { border-left-color: #647085; color: #586478; }
  .water-overview-strip span,
  .water-overview-strip small,
  .water-overview-strip strong { display: block; min-width: 0; }
  .water-overview-strip small { color: #748079; font-size: 8px; }
  .water-overview-strip strong {
    margin-top: 5px;
    overflow: hidden;
    color: #33433c;
    font-size: 12px;
    text-overflow: ellipsis;
    text-transform: capitalize;
    white-space: nowrap;
  }
  @media (max-width: 900px) {
    .water-overview-strip { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  @media (max-width: 560px) {
    .water-overview-strip { grid-template-columns: minmax(0, 1fr); }
    .water-area-header { padding-left: 8px; }
  }
</style>

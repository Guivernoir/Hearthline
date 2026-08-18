<script lang="ts">
  import type { Component } from "svelte";
  import {
    ArrowLeft,
    ArrowRight,
    Beaker,
    ChevronDown,
    Droplets,
    Factory,
    FlaskConical,
    Map,
    MonitorCog,
    Network,
  } from "@lucide/svelte";
  import { findAppliancesForEnvironment } from "../../config/appliance-config";
  import type { ViewMode } from "../../shared/types";
  import type { BodyPreparationScope } from "../layout/body-preparation-area-layout";

  interface Building {
    id: BodyPreparationScope;
    label: string;
    zone: string;
    subtitle: string;
    detail: string;
    accent: string;
    icon: Component<any>;
    rioCount: number;
    hmiId: string;
    controllerId: string;
    switchId: string;
    tags: string[];
    logical: { x: number; y: number };
  }

  export let viewMode: ViewMode = "physical";
  export let onBack: () => void = () => {};
  export let onEnterBuilding: (building: BodyPreparationScope) => void = () => {};
  export let onOpenHmi: (id: string) => void = () => {};

  const buildings: Building[] = [
    {
      id: "water",
      label: "Water and Effluent Treatment",
      zone: "Area 01 / Utilities",
      subtitle: "Fresh process water and segregated return-water recovery",
      detail:
        "Treats incoming factory water, stores released process water, and recovers body- and glaze-derived return streams without combining their reuse inventories.",
      accent: "#39798a",
      icon: Droplets,
      rioCount: 2,
      hmiId: "area-01-wt-hmi-01",
      controllerId: "area-01-wt-vplc-01",
      switchId: "area-01-wt-sw-01",
      tags: ["water-treatment", "return-water", "water-handoff"],
      logical: { x: 85, y: 325 },
    },
    {
      id: "slip",
      label: "Slip Preparation",
      zone: "Area 01 / Body",
      subtitle: "Dry batching, blunging, conditioning, release, and transfer",
      detail:
        "Produces the released ceramic-slip batch consumed by Forming, including independent powder dosing, rheology checks, temperature trim, and controlled transfer.",
      accent: "#8a6844",
      icon: Beaker,
      rioCount: 2,
      hmiId: "area-01-hmi-01",
      controllerId: "area-01-vplc-01",
      switchId: "area-01-sw-01",
      tags: ["slip", "slip-handoff"],
      logical: { x: 470, y: 325 },
    },
    {
      id: "glaze",
      label: "Glaze Preparation",
      zone: "Area 01 / Glaze",
      subtitle: "Powder batching, wet milling, finishing, storage, and transfer",
      detail:
        "Prepares the liquid glaze from seven dry materials, process water, and dispersant before quality release and delivery to the glazing process.",
      accent: "#706589",
      icon: FlaskConical,
      rioCount: 1,
      hmiId: "area-01-gl-hmi-01",
      controllerId: "area-01-gl-vplc-01",
      switchId: "area-01-gl-sw-01",
      tags: ["glaze", "glaze-handoff"],
      logical: { x: 855, y: 325 },
    },
  ];

  const appliances = findAppliancesForEnvironment("Body Preparation");
  let selectedId: BodyPreparationScope | null = null;
  $: selected = buildings.find((building) => building.id === selectedId) ?? null;

  function fieldCount(building: Building) {
    return appliances.filter((appliance) =>
      building.tags.some((tag) => appliance.tags.includes(tag)),
    ).length;
  }
</script>

<svelte:head>
  <title>Body Preparation Complex | Hearthline</title>
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="brand-block">
      <button type="button" class="brand-back" aria-label="Back to ceramics process" title="Back to ceramics process" onclick={onBack}>
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark" aria-hidden="true"><Factory size={20} strokeWidth={1.8} /></span>
      <div class="brand-copy"><strong>Hearthline</strong><span>Architecture</span></div>
    </div>
    <div class="view-context" aria-label="Current view">
      <span>Factory / Ceramics process</span><ChevronDown size={14} strokeWidth={1.8} /><strong>Body Preparation</strong>
    </div>
    <div class="toolbar" aria-label="View tools">
      <div class="view-mode-control" aria-label="Architecture view">
        <button type="button" class:active={viewMode === "physical"} aria-pressed={viewMode === "physical"} onclick={() => (viewMode = "physical")}>
          <Map size={15} strokeWidth={1.9} /><span>Physical</span>
        </button>
        <button type="button" class:active={viewMode === "logical"} aria-pressed={viewMode === "logical"} onclick={() => (viewMode = "logical")}>
          <Network size={15} strokeWidth={1.9} /><span>Logical</span>
        </button>
      </div>
      <button type="button" disabled={!selected} aria-label="Open selected local HMI" title="Open selected local HMI" onclick={() => selected && onOpenHmi(selected.hmiId)}>
        <MonitorCog size={17} strokeWidth={1.9} />
      </button>
    </div>
  </header>

  <main class="workspace overview-workspace body-gateway-workspace">
    <section class:physical-view={viewMode === "physical"} class:logical-view={viewMode === "logical"} class="location-world body-gateway-world" aria-label="Body Preparation buildings">
      <div class="location-heading">
        <span>HEARTHLINE / FACTORY / OT-AREA-01</span>
        <h1>Body Preparation Complex</h1>
        <p>{viewMode === "physical" ? "Separate process buildings, local operator rooms, utility corridors, and monitored handoffs" : "Three independently controlled cells with explicit utility and material pipeline instrumentation"}</p>
      </div>

      {#if viewMode === "physical"}
        <svg class="body-gateway-drawing" viewBox="0 0 1200 700" aria-hidden="true">
          <rect class="body-campus-ground" x="35" y="145" width="1130" height="475"></rect>
          <path class="body-campus-fence" d="M35 620 V145 H1165 V620"></path>
          <rect class="body-campus-road" x="0" y="645" width="1200" height="55"></rect>
          <path class="body-campus-pipe-rack" d="M390 410 H460 M790 410 H845 M245 545 H1000"></path>
          <path class="body-campus-water-line" d="M390 365 H460 M390 385 H845"></path>
          <path class="body-campus-return-line" d="M460 505 H365 M845 505 H365"></path>
          <path class="body-campus-slip-handoff" d="M790 455 H1180"></path>
          <path class="body-campus-glaze-handoff" d="M1080 455 H1180"></path>

          <g class="body-campus-building water-building">
            <path d="M75 245 H390 V555 H75 Z"></path><path class="roof" d="M62 225 H403 V245 H62 Z"></path>
            <path class="door" d="M205 465 H265 V555 H205 Z"></path><path class="window" d="M110 295 H190 M275 295 H355"></path>
            <text x="118" y="438">WATER &amp; EFFLUENT TREATMENT</text>
          </g>
          <g class="body-campus-building slip-building">
            <path d="M460 205 H790 V555 H460 Z"></path><path class="roof" d="M447 185 H803 V205 H447 Z"></path>
            <path class="door" d="M595 465 H655 V555 H595 Z"></path><path class="window" d="M500 255 H580 M670 255 H750"></path>
            <text x="560" y="438">SLIP PREPARATION</text>
          </g>
          <g class="body-campus-building glaze-building">
            <path d="M845 245 H1125 V555 H845 Z"></path><path class="roof" d="M832 225 H1138 V245 H832 Z"></path>
            <path class="door" d="M955 465 H1015 V555 H955 Z"></path><path class="window" d="M875 295 H945 M1025 295 H1095"></path>
            <text x="925" y="438">GLAZE PREPARATION</text>
          </g>
          <g class="body-campus-local-control"><rect x="92" y="480" width="105" height="54"></rect><text x="108" y="512">LOCAL CONTROL</text></g>
          <g class="body-campus-local-control"><rect x="477" y="480" width="105" height="54"></rect><text x="493" y="512">LOCAL CONTROL</text></g>
          <g class="body-campus-local-control"><rect x="860" y="480" width="82" height="54"></rect><text x="867" y="512">LOCAL CONTROL</text></g>
          <text class="body-campus-label" x="55" y="170">BODY PREPARATION CONTROLLED AREA</text>
          <text class="body-campus-label" x="520" y="582">ABOVE-GROUND PIPE AND CABLE CORRIDOR</text>
          <text class="body-campus-label" x="1020" y="445">TO DOWNSTREAM AREAS</text>
        </svg>

        {#each buildings as building (building.id)}
          {@const Icon = building.icon}
          <button type="button" class:selected={selectedId === building.id} class={`body-building-node body-building-${building.id}`} style={`--node-accent: ${building.accent};`} onclick={() => (selectedId = building.id)} ondblclick={() => onEnterBuilding(building.id)}>
            <span class="node-icon"><Icon size={31} strokeWidth={1.8} /></span>
            <span><small>{building.zone}</small><strong>{building.label}</strong></span>
            <ArrowRight size={16} strokeWidth={1.9} />
          </button>
        {/each}
      {:else}
        <div class="body-control-boundary"><span><Network size={17} />FACTORY LEVEL 3 UPLINK BOUNDARY</span></div>
        <svg class="body-gateway-drawing" viewBox="0 0 1200 700" aria-hidden="true">
          <defs>
            <marker id="body-water-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path class="body-water-arrow" d="M0 0 L8 4 L0 8 Z"></path></marker>
            <marker id="body-return-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path class="body-return-arrow" d="M0 0 L8 4 L0 8 Z"></path></marker>
            <marker id="body-slip-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path class="body-slip-arrow" d="M0 0 L8 4 L0 8 Z"></path></marker>
            <marker id="body-glaze-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path class="body-glaze-arrow" d="M0 0 L8 4 L0 8 Z"></path></marker>
          </defs>
          <path class="body-control-backbone" d="M215 325 V260 M600 325 V260 M985 325 V260"></path>
          <path class="body-process-water-link" d="M345 405 H470" marker-end="url(#body-water-arrow)"></path>
          <path class="body-process-water-link" d="M215 475 V550 H985 V475" marker-end="url(#body-water-arrow)"></path>
          <path class="body-return-water-link" d="M600 475 V515 H365 V440 H345" marker-end="url(#body-return-arrow)"></path>
          <path class="body-return-water-link" d="M985 475 V535 H385 V455 H345" marker-end="url(#body-return-arrow)"></path>
          <path class="body-slip-handoff-link" d="M730 405 H780 V570 H1150" marker-end="url(#body-slip-arrow)"></path>
          <path class="body-glaze-handoff-link" d="M1115 430 H1150" marker-end="url(#body-glaze-arrow)"></path>
          <circle class="body-pipeline-sensor" cx="408" cy="405" r="6"></circle>
          <circle class="body-pipeline-sensor" cx="600" cy="550" r="6"></circle>
          <circle class="body-pipeline-sensor" cx="780" cy="515" r="6"></circle>
          <circle class="body-pipeline-sensor" cx="1128" cy="430" r="6"></circle>
          <text class="body-gateway-link-label" x="405" y="247">REDUNDANT CELL UPLINKS</text>
          <text class="body-gateway-link-label" x="365" y="394">WATER TO SLIP</text>
          <text class="body-gateway-link-label" x="635" y="545">WATER TO GLAZE</text>
          <text class="body-gateway-link-label" x="395" y="505">SEGREGATED RETURNS</text>
          <text class="body-gateway-link-label" x="900" y="565">SLIP TO FORMING</text>
          <text class="body-gateway-link-label" x="1118" y="416">TO GLAZING</text>
        </svg>
        {#each buildings as building (building.id)}
          {@const Icon = building.icon}
          <button type="button" class:selected={selectedId === building.id} class={`environment-node body-logical-building body-logical-${building.id}`} style={`left: ${building.logical.x}px; top: ${building.logical.y}px; --node-accent: ${building.accent};`} onclick={() => (selectedId = building.id)} ondblclick={() => onEnterBuilding(building.id)}>
            <span class="node-accent"></span><span class="environment-node-header"><span class="node-icon"><Icon size={20} /></span><small>{building.zone}</small></span>
            <strong>{building.label}</strong><span>{building.hmiId} / {building.controllerId}</span><em>{fieldCount(building)} field devices / {building.rioCount} RIO / {building.switchId}</em>
          </button>
        {/each}
      {/if}

      {#if selected}
        <aside class="environment-detail body-building-detail">
          <div class="environment-detail-copy"><span>{selected.zone}</span><h2>{selected.label}</h2><p>{selected.detail}</p></div>
          <div class="body-building-actions">
            <button type="button" class="open-local-hmi" onclick={() => onOpenHmi(selected.hmiId)}><MonitorCog size={16} />Open local HMI</button>
            <button type="button" class="enter-environment" onclick={() => onEnterBuilding(selected.id)}>Enter building<ArrowRight size={17} /></button>
          </div>
        </aside>
      {/if}
    </section>
  </main>
  <footer class="statusbar"><span class="status-state"><i></i>OT-AREA-01</span><span>3 process buildings / {appliances.length} configured appliances</span><span>{viewMode === "physical" ? "Physical" : "Logical"} gateway</span></footer>
</div>

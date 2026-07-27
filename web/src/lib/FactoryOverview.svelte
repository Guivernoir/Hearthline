<script lang="ts">
  import type { Component } from "svelte";
  import {
    ArrowLeft,
    ArrowRight,
    ChevronDown,
    Factory,
    Map,
    Network,
    ShieldCheck,
  } from "@lucide/svelte";
  import type { ViewMode } from "./types";

  interface FactoryEnvironment {
    id: "ot-dmz" | "process";
    label: string;
    subtitle: string;
    zone: string;
    detail: string;
    accent: string;
    icon: Component<any>;
    logical: { x: number; y: number };
  }

  export let onBack: () => void = () => {};
  export let onEnterEnvironment: (environmentId: string) => void = () => {};
  export let viewMode: ViewMode = "logical";

  const environments: FactoryEnvironment[] = [
    {
      id: "ot-dmz",
      label: "Factory OT DMZ",
      subtitle: "Local IT/OT boundary and controlled exchange",
      zone: "Level 3.5",
      detail:
        "Terminates the encrypted Central Office conduit and enforces both the IT-side and OT-side policies locally before any session reaches factory operations.",
      accent: "#b65034",
      icon: ShieldCheck,
      logical: { x: 285, y: 300 },
    },
    {
      id: "process",
      label: "OT Operations & Process",
      subtitle: "Level 3 services and segmented production areas",
      zone: "Levels 0–3",
      detail:
        "Contains factory-local supervisory services, engineering, historians, virtual PLCs, and the segmented ceramics process areas.",
      accent: "#267168",
      icon: Factory,
      logical: { x: 730, y: 300 },
    },
  ];

  let selectedId: FactoryEnvironment["id"] | null = null;
  $: selectedEnvironment =
    environments.find((environment) => environment.id === selectedId) ?? null;
</script>

<svelte:head>
  <title>Factory | Hearthline</title>
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="brand-block">
      <button
        type="button"
        class="brand-back"
        aria-label="Back to regional map"
        title="Back to regional map"
        onclick={onBack}
      >
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark" aria-hidden="true"><Network size={20} strokeWidth={1.8} /></span>
      <div class="brand-copy">
        <strong>Hearthline</strong>
        <span>Architecture</span>
      </div>
    </div>

    <div class="view-context" aria-label="Current view">
      <span>Factory</span>
      <ChevronDown size={14} strokeWidth={1.8} />
      <strong>Environment overview</strong>
    </div>

    <div class="toolbar" aria-label="View tools">
      <div class="view-mode-control" aria-label="Architecture view">
        <button
          type="button"
          class:active={viewMode === "physical"}
          aria-pressed={viewMode === "physical"}
          onclick={() => (viewMode = "physical")}
        >
          <Map size={15} strokeWidth={1.9} />
          <span>Physical</span>
        </button>
        <button
          type="button"
          class:active={viewMode === "logical"}
          aria-pressed={viewMode === "logical"}
          onclick={() => (viewMode = "logical")}
        >
          <Network size={15} strokeWidth={1.9} />
          <span>Logical</span>
        </button>
      </div>
    </div>
  </header>

  <main class="workspace overview-workspace">
    <section
      class:physical-view={viewMode === "physical"}
      class:logical-view={viewMode === "logical"}
      class="location-world factory-location-world"
      aria-label="Factory environments"
    >
      <div class="location-heading">
        <span>HEARTHLINE / FACTORY</span>
        <h1>Factory</h1>
        <p>{viewMode === "physical" ? "Factory perimeter and local operational placement" : "Local security boundaries and operational environments"}</p>
      </div>

      {#if viewMode === "physical"}
        <div class="factory-site-scene">
          <svg
            class="factory-site-art"
            viewBox="0 0 1200 700"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <rect class="factory-campus-ground" x="52" y="160" width="1096" height="430"></rect>
            <path class="factory-campus-fence" d="M52 590 V160 H1148 V590"></path>
            <rect class="factory-campus-road" x="0" y="620" width="1200" height="80"></rect>
            <path class="factory-campus-sidewalk" d="M0 598 H1200"></path>
            <path class="factory-campus-drive" d="M250 590 H390 L440 620 H200 Z"></path>

            <g class="factory-security-building">
              <path d="M125 270 H430 V555 H125 Z"></path>
              <path class="factory-security-roof" d="M112 250 H443 V270 H112 Z"></path>
              <path class="factory-security-window" d="M165 310 H250 M285 310 H390"></path>
              <path class="factory-security-door" d="M270 470 H330 V555 H270 Z"></path>
              <text x="165" y="445">FACTORY SECURITY &amp; EXCHANGE</text>
            </g>

            <g class="factory-production-hall">
              <path d="M500 270 L560 220 L620 270 L680 220 L740 270 L800 220 L860 270 L920 220 L980 270 H1095 V555 H500 Z"></path>
              <path class="factory-hall-window" d="M555 320 H680 M720 320 H845 M885 320 H1040"></path>
              <path class="factory-loading-door" d="M565 430 H690 V555 H565 Z M865 430 H990 V555 H865 Z"></path>
              <path class="factory-stack" d="M1015 190 H1050 V270 H1015 Z M1065 165 H1100 V270 H1065 Z"></path>
              <text x="705" y="395">CERAMICS PRODUCTION HALL</text>
            </g>

            <g class="factory-site-utilities">
              <rect x="470" y="545" width="655" height="30"></rect>
              <path d="M430 395 H500 M1095 350 H1148"></path>
              <text x="770" y="570">UTILITY AND MATERIAL CORRIDOR</text>
            </g>

            <g class="factory-site-gate">
              <rect x="72" y="540" width="74" height="50"></rect>
              <path d="M109 540 V510 M88 510 H130 M146 565 H205"></path>
              <text x="67" y="500">SECURITY GATE</text>
            </g>

            <path class="factory-inter-site-conduit" d="M0 300 H125"></path>
            <text class="factory-site-label" x="62" y="180">FACTORY SECURITY PERIMETER</text>
            <text class="factory-site-label" x="8" y="287">FROM CENTRAL OFFICE</text>
          </svg>

          {#each environments as environment (environment.id)}
            {@const Icon = environment.icon}
            <button
              type="button"
              class:selected={selectedId === environment.id}
              class={`site-environment-node factory-site-${environment.id}`}
              style={`--node-accent: ${environment.accent};`}
              onclick={() => (selectedId = environment.id)}
              ondblclick={() => onEnterEnvironment(environment.id)}
            >
              <span class="node-icon"><Icon size={32} strokeWidth={1.8} /></span>
              <span class="site-environment-copy">
                <small>{environment.zone}</small>
                <strong>{environment.label}</strong>
              </span>
              <ArrowRight size={16} strokeWidth={1.9} />
            </button>
          {/each}
        </div>
      {:else}
        <div class="site-boundary factory-logical-boundary">
          <span><Factory size={18} strokeWidth={1.8} />Factory security perimeter</span>
        </div>

        <svg class="overview-connections" viewBox="0 0 1200 700" aria-hidden="true">
          <defs>
            <marker id="factory-overview-arrow" markerWidth="9" markerHeight="9" refX="8" refY="4.5" orient="auto">
              <path d="M0,0 L9,4.5 L0,9 Z"></path>
            </marker>
          </defs>
          <path class="factory-external-link" d="M80 375 H285" marker-end="url(#factory-overview-arrow)"></path>
          <path class="factory-internal-link" d="M545 375 H730" marker-end="url(#factory-overview-arrow)"></path>
          <text class="factory-logical-label" x="80" y="352">ENCRYPTED CENTRAL OFFICE CONDUIT</text>
          <text class="factory-logical-label" x="574" y="352">INDEPENDENT FIREWALL POLICIES</text>
        </svg>

        {#each environments as environment (environment.id)}
          {@const Icon = environment.icon}
          <button
            type="button"
            class:selected={selectedId === environment.id}
            class="environment-node factory-environment-node"
            style={`left: ${environment.logical.x}px; top: ${environment.logical.y}px; --node-accent: ${environment.accent};`}
            onclick={() => (selectedId = environment.id)}
            ondblclick={() => onEnterEnvironment(environment.id)}
          >
            <span class="node-accent"></span>
            <span class="environment-node-header">
              <span class="node-icon"><Icon size={20} strokeWidth={1.8} /></span>
              <small>{environment.zone}</small>
            </span>
            <strong>{environment.label}</strong>
            <span>{environment.subtitle}</span>
          </button>
        {/each}
      {/if}

      {#if selectedEnvironment}
        <aside class="environment-detail">
          <div class="environment-detail-copy">
            <span>{selectedEnvironment.zone}</span>
            <h2>{selectedEnvironment.label}</h2>
            <p>{selectedEnvironment.detail}</p>
          </div>
          <button
            type="button"
            class="enter-environment"
            onclick={() => onEnterEnvironment(selectedEnvironment.id)}
          >
            Enter environment
            <ArrowRight size={17} strokeWidth={1.9} />
          </button>
        </aside>
      {/if}
    </section>
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>Factory architecture model</span>
    <span>{environments.length} environments</span>
    <span>{viewMode === "physical" ? "Physical" : "Logical"} view</span>
  </footer>
</div>

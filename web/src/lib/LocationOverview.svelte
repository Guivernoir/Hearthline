<script lang="ts">
  import type { Component } from "svelte";
  import {
    ArrowLeft,
    ArrowRight,
    Building2,
    ChevronDown,
    Globe2,
    House,
    Map,
    MonitorCog,
    Network,
    Router,
    Server,
    ShieldCheck,
    Wifi,
  } from "@lucide/svelte";
  import type { PlaceId, ViewMode } from "./types";

  interface Environment {
    id: string;
    label: string;
    subtitle: string;
    zone: string;
    detail: string;
    x: number;
    y: number;
    accent: string;
    icon: Component<any>;
    enterable?: boolean;
  }

  export let place: Exclude<PlaceId, "factory">;
  export let onBack: () => void = () => {};
  export let onEnterEnvironment: (environmentId: string) => void = () => {};
  export let viewMode: ViewMode = "logical";

  const customerEnvironments: Environment[] = [
    {
      id: "customer-lan",
      label: "Customer LAN",
      subtitle: "User devices and local access",
      zone: "Private network",
      detail: "Customer-managed endpoints share a private LAN behind the residential edge.",
      x: 145,
      y: 285,
      accent: "#3567a6",
      icon: Wifi,
      enterable: true,
    },
    {
      id: "customer-edge",
      label: "Customer Edge",
      subtitle: "Routing, NAT, and ISP handoff",
      zone: "Trust boundary",
      detail: "The edge translates private addresses and forwards public traffic toward the ISP.",
      x: 500,
      y: 285,
      accent: "#267168",
      icon: Router,
      enterable: true,
    },
    {
      id: "public-service",
      label: "Public Web Path",
      subtitle: "HTTPS path to the business DMZ",
      zone: "External service",
      detail: "Represents legitimate customer access and the untrusted traffic used for security testing.",
      x: 855,
      y: 285,
      accent: "#b65034",
      icon: Globe2,
      enterable: true,
    },
  ];

  const officeEnvironments: Environment[] = [
    {
      id: "it-dmz",
      label: "IT DMZ",
      subtitle: "Public services and isolation",
      zone: "Enterprise perimeter",
      detail: "Hosts internet-facing services while separating them from the internal enterprise network.",
      x: 130,
      y: 285,
      accent: "#b65034",
      icon: ShieldCheck,
      enterable: true,
    },
    {
      id: "business-it",
      label: "Business IT",
      subtitle: "Users, identity, and services",
      zone: "Enterprise",
      detail: "Contains the internal users, shared services, administration, and enterprise management plane.",
      x: 480,
      y: 285,
      accent: "#3567a6",
      icon: Server,
      enterable: true,
    },
    {
      id: "operations-intelligence",
      label: "Operations Intelligence",
      subtitle: "Network, security, and process analysis",
      zone: "Enterprise operations",
      detail: "Centralizes network governance, security monitoring, production analytics, and approved change decisions without directly controlling factory processes.",
      x: 830,
      y: 285,
      accent: "#267168",
      icon: MonitorCog,
      enterable: true,
    },
  ];

  $: isOffice = place === "office";
  $: environments = isOffice ? officeEnvironments : customerEnvironments;
  $: placeLabel = isOffice ? "Central Office" : "Customer Network";
  $: PlaceIcon = isOffice ? Building2 : House;
  let selectedId: string | null = null;
  $: selectedEnvironment = environments.find((environment) => environment.id === selectedId) ?? null;
</script>

<svelte:head>
  <title>{placeLabel} | Hearthline</title>
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
      <span>{placeLabel}</span>
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
      class="location-world"
      aria-label={`${placeLabel} environments`}
    >
      <div class="location-heading">
        <span>HEARTHLINE / {isOffice ? "CENTRAL OFFICE" : "CUSTOMER"}</span>
        <h1>{placeLabel}</h1>
        <p>{viewMode === "physical" ? "Site layout and physical placement" : "Environment boundaries and permitted direction of travel"}</p>
      </div>

      {#if viewMode === "physical"}
        {#if isOffice}
          <div class="office-site-scene">
            <svg
              class="office-site-art"
              viewBox="0 0 1200 700"
              preserveAspectRatio="none"
              aria-hidden="true"
            >
              <rect class="office-campus-ground" x="58" y="160" width="1084" height="424"></rect>
              <path class="office-campus-fence" d="M58 584 V160 H1142 V584"></path>
              <rect class="office-campus-road" x="0" y="616" width="1200" height="84"></rect>
              <path class="office-campus-sidewalk" d="M0 594 H1200"></path>
              <path class="office-campus-drive" d="M520 584 H680 L730 616 H470 Z"></path>

              <g class="office-campus-parking">
                <rect x="760" y="560" width="300" height="28"></rect>
                <path d="M800 560 V588 M845 560 V588 M890 560 V588 M935 560 V588 M980 560 V588 M1025 560 V588"></path>
                <path class="parking-access" d="M760 574 H690"></path>
                <text x="770" y="578">STAFF PARKING</text>
              </g>

              <g class="office-campus-zones">
                <rect class="office-campus-perimeter-zone" x="90" y="250" width="275" height="305"></rect>
                <rect class="office-campus-enterprise-zone" x="390" y="205" width="405" height="350"></rect>
                <rect class="office-campus-operations-zone" x="820" y="250" width="290" height="305"></rect>
                <path class="office-zone-roof" d="M80 250 H375 M380 205 H805 M810 250 H1120"></path>
                <path class="office-zone-window" d="M125 300 H225 M425 255 H535 M570 255 H680 M855 300 H965"></path>
                <path class="office-zone-window" d="M125 485 H225 M425 480 H535 M570 480 H680 M855 485 H965"></path>
                <path class="office-building-entry" d="M565 475 H625 V555 H565 Z"></path>
                <path class="office-entry-canopy" d="M545 468 H645 L632 482 H558 Z"></path>
                <text x="120" y="280">PERIMETER SERVICES</text>
                <text x="520" y="235">ENTERPRISE OFFICE</text>
                <text x="880" y="280">OPERATIONS CENTER</text>
              </g>

              <g class="office-campus-service">
                <rect x="72" y="345" width="44" height="88"></rect>
                <path class="office-provider-line" d="M0 389 H72 M116 389 H225"></path>
                <text x="70" y="335">WAN DEMARC</text>
              </g>

              <g class="office-campus-access">
                <rect x="1070" y="548" width="54" height="36"></rect>
                <path d="M1097 548 V520 M1081 520 H1113 M1124 566 H1142"></path>
                <text x="1030" y="512">SECURITY GATE</text>
              </g>

              <path class="office-campus-conduit" d="M225 395 H600 H965"></path>
              <path class="office-campus-conduit outbound" d="M965 395 H1142"></path>
              <text class="office-campus-label" x="80" y="180">CONTROLLED CAMPUS</text>
              <text class="office-campus-label" x="1032" y="638">SITE ACCESS</text>
            </svg>

            <div class="mobile-office-campus" aria-hidden="true">
              <span class="mobile-office-zone perimeter"></span>
              <span class="mobile-office-zone enterprise"></span>
              <span class="mobile-office-zone operations"></span>
            </div>

            {#each officeEnvironments as environment (environment.id)}
              {@const Icon = environment.icon}
              <button
                type="button"
                class:selected={selectedId === environment.id}
                class:enterable={environment.enterable}
                class={`site-environment-node site-${environment.id}`}
                style={`--node-accent: ${environment.accent};`}
                onclick={() => (selectedId = environment.id)}
                ondblclick={() => environment.enterable && onEnterEnvironment(environment.id)}
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
        <div class="customer-site-scene">
          <svg
            class="customer-site-art"
            viewBox="0 0 1200 700"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <rect class="site-lawn" x="72" y="176" width="1056" height="394"></rect>
            <path class="site-property-line" d="M72 570 V176 H1128 V570"></path>
            <rect class="site-road" x="0" y="605" width="1200" height="95"></rect>
            <path class="site-sidewalk" d="M0 580 H1200"></path>
            <path class="site-driveway" d="M642 570 L724 570 L770 605 L600 605 Z"></path>

            <g class="site-house">
              <path class="site-house-roof" d="M120 344 L350 205 L580 344 Z"></path>
              <rect class="site-house-wall" x="145" y="334" width="410" height="206"></rect>
              <rect class="site-house-door" x="445" y="432" width="58" height="108"></rect>
              <rect class="site-house-window" x="195" y="382" width="78" height="62"></rect>
              <rect class="site-house-window" x="320" y="382" width="78" height="62"></rect>
              <path class="site-house-foundation" d="M120 540 H580"></path>
              <text x="165" y="518">CUSTOMER LAN / RESIDENCE</text>
            </g>

            <g class="site-service">
              <rect class="site-edge-cabinet" x="620" y="315" width="245" height="225"></rect>
              <rect class="site-demarc" x="650" y="360" width="42" height="70"></rect>
              <path class="site-ethernet-path" d="M555 398 H650"></path>
              <path class="site-dsl-path" d="M692 395 H865"></path>
              <path class="site-utility-pole" d="M930 266 V520 M902 304 H958 M912 520 H948"></path>
              <path class="site-provider-path" d="M865 395 H930 M930 266 C1025 245 1090 248 1200 226"></path>
              <text x="650" y="345">CUSTOMER EDGE / DEMARC</text>
              <text x="870" y="548">PUBLIC ACCESS HANDOFF</text>
            </g>

            <text class="site-area-label" x="94" y="205">CUSTOMER PROPERTY</text>
            <text class="site-area-label" x="1026" y="632">PUBLIC ACCESS</text>
          </svg>

          <div class="mobile-site-house" aria-hidden="true">
            <span class="mobile-house-roof"></span>
            <span class="mobile-house-wall"></span>
            <span class="mobile-house-window window-left"></span>
            <span class="mobile-house-window window-right"></span>
            <span class="mobile-house-door"></span>
          </div>

          {#each customerEnvironments as environment (environment.id)}
            {@const Icon = environment.icon}
            <button
              type="button"
              class:selected={selectedId === environment.id}
              class:enterable={environment.enterable}
              class={`site-environment-node site-${environment.id}`}
              style={`--node-accent: ${environment.accent};`}
              onclick={() => (selectedId = environment.id)}
              ondblclick={() => environment.enterable && onEnterEnvironment(environment.id)}
            >
              <span class="node-icon"><Icon size={32} strokeWidth={1.8} /></span>
              <span class="site-environment-copy">
                <small>{environment.zone}</small>
                <strong>{environment.label}</strong>
              </span>
              {#if environment.enterable}
                <ArrowRight size={16} strokeWidth={1.9} />
              {/if}
            </button>
          {/each}
        </div>
        {/if}
      {:else}
        <div class="site-boundary">
          <span><PlaceIcon size={18} strokeWidth={1.8} />{isOffice ? "Operations campus" : "Customer premises"}</span>
        </div>

        <svg class="overview-connections" viewBox="0 0 1200 700" aria-hidden="true">
          <defs>
            <marker id="overview-arrow" markerWidth="9" markerHeight="9" refX="8" refY="4.5" orient="auto">
              <path d="M0,0 L9,4.5 L0,9 Z"></path>
            </marker>
          </defs>
          <g class="overview-logical-links">
            {#if isOffice}
              <path d="M480 360 H390" marker-end="url(#overview-arrow)"></path>
              <path d="M740 360 H830" marker-end="url(#overview-arrow)"></path>
            {:else}
              <path d="M405 360 H500" marker-end="url(#overview-arrow)"></path>
              <path d="M760 360 H855" marker-end="url(#overview-arrow)"></path>
            {/if}
          </g>
        </svg>

        {#each environments as environment (environment.id)}
          {@const Icon = environment.icon}
          <button
            type="button"
            class:selected={selectedId === environment.id}
            class="environment-node"
            style={`left: ${environment.x}px; top: ${environment.y}px; --node-accent: ${environment.accent};`}
            onclick={() => (selectedId = environment.id)}
            ondblclick={() => environment.enterable && onEnterEnvironment(environment.id)}
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
          {#if selectedEnvironment.enterable}
            <button
              type="button"
              class="enter-environment"
              onclick={() => onEnterEnvironment(selectedEnvironment.id)}
            >
              Enter environment
              <ArrowRight size={17} strokeWidth={1.9} />
            </button>
          {/if}
        </aside>
      {/if}
    </section>
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>Architecture model</span>
    <span>{environments.length} environments</span>
    <span>{viewMode === "physical" ? "Physical" : "Logical"} view</span>
  </footer>
</div>

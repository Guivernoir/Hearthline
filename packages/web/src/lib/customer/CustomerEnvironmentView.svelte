<script lang="ts">
  import { onMount, tick } from "svelte";
  import {
    ArrowLeft,
    Cable,
    ChevronDown,
    Grid2X2,
    Map,
    Maximize2,
    Minus,
    Network,
    Plus,
    RotateCcw,
    ShieldCheck,
    Wifi,
    X,
  } from "@lucide/svelte";
  import ApplianceConfigSummary from "../config/ApplianceConfigSummary.svelte";
  import { findAppliancesForNode } from "../config/appliance-config";
  import PhysicalDeviceMarker from "../shared/PhysicalDeviceMarker.svelte";
  import type { ViewMode } from "../shared/types";
  import {
    edgeNodes,
    publicPathNodes,
    type CustomerEnvironment,
    type EnvironmentNode,
  } from "./customer-environment-model";

  export let environment: CustomerEnvironment;
  export let onBack: () => void = () => {};
  export let onOpenAppliance: (id: string) => void = () => {};
  export let viewMode: ViewMode = "logical";

  const WORLD_WIDTH = 1800;
  const WORLD_HEIGHT = 900;
  const MIN_ZOOM = 0.22;
  const MAX_ZOOM = 1.6;
  const ZOOM_STEP = 0.1;

  $: isEdge = environment === "edge";
  $: nodes = isEdge ? edgeNodes : publicPathNodes;
  $: title = isEdge ? "Customer Edge" : "Public Web Path";
  $: subtitle =
    viewMode === "physical"
      ? isEdge
        ? "Residential service cabinet and provider handoff"
        : "Physical service corridor from customer premises to the business DMZ"
      : isEdge
        ? "PAT, routing, media conversion, and default-route behavior"
        : "DNS, routed transit, static NAT, policy, and public web delivery";

  let viewport: HTMLDivElement;
  let zoom = 0.72;
  let gridVisible = false;
  let selectedId: string | null = null;
  let dragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragScrollLeft = 0;
  let dragScrollTop = 0;
  let viewportWidth = 1;
  let viewportHeight = 1;

  $: selectedNode = nodes.find((node) => node.id === selectedId) ?? null;
  $: selectedAppliances = selectedNode
    ? findAppliancesForNode(
        isEdge ? "customer/customer-edge" : "customer/public-web-path",
        selectedNode.id,
        viewMode,
      )
    : [];
  $: worldPixelWidth = WORLD_WIDTH * zoom;
  $: worldPixelHeight = WORLD_HEIGHT * zoom;
  $: worldOffsetX = Math.max(0, (viewportWidth - worldPixelWidth) / 2);
  $: worldOffsetY = Math.max(0, (viewportHeight - worldPixelHeight) / 2);

  function clampZoom(value: number) {
    return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value));
  }

  async function setZoom(nextZoom: number, focalX?: number, focalY?: number) {
    if (!viewport) return;
    const targetZoom = clampZoom(Number(nextZoom.toFixed(2)));
    if (targetZoom === zoom) return;

    const focusX = focalX ?? viewport.clientWidth / 2;
    const focusY = focalY ?? viewport.clientHeight / 2;
    const currentOffsetX = Math.max(0, (viewport.clientWidth - WORLD_WIDTH * zoom) / 2);
    const currentOffsetY = Math.max(0, (viewport.clientHeight - WORLD_HEIGHT * zoom) / 2);
    const worldX = (viewport.scrollLeft + focusX - currentOffsetX) / zoom;
    const worldY = (viewport.scrollTop + focusY - currentOffsetY) / zoom;

    zoom = targetZoom;
    await tick();
    if (!viewport) return;

    const nextOffsetX = Math.max(0, (viewport.clientWidth - WORLD_WIDTH * zoom) / 2);
    const nextOffsetY = Math.max(0, (viewport.clientHeight - WORLD_HEIGHT * zoom) / 2);
    viewport.scrollLeft = worldX * zoom + nextOffsetX - focusX;
    viewport.scrollTop = worldY * zoom + nextOffsetY - focusY;
  }

  async function fitToView() {
    if (!viewport) return;
    const compactLayout = viewport.clientWidth < 620;
    const horizontalPadding = viewport.clientWidth < 720 ? 24 : 80;
    const verticalPadding = viewport.clientHeight < 620 ? 24 : 64;
    const fittedZoom = Math.min(
      (viewport.clientWidth - horizontalPadding) / WORLD_WIDTH,
      (viewport.clientHeight - verticalPadding) / WORLD_HEIGHT,
    );
    zoom = Number(
      clampZoom(compactLayout ? Math.max(0.65, fittedZoom) : fittedZoom).toFixed(2),
    );
    await tick();
    if (!viewport) return;
    viewport.scrollLeft = compactLayout
      ? 0
      : Math.max(0, (WORLD_WIDTH * zoom - viewport.clientWidth) / 2);
    viewport.scrollTop = Math.max(0, (WORLD_HEIGHT * zoom - viewport.clientHeight) / 2);
  }

  async function resetView() {
    zoom = 1;
    await tick();
    if (!viewport) return;
    viewport.scrollLeft = 0;
    viewport.scrollTop = 0;
  }

  function selectNode(event: MouseEvent, node: EnvironmentNode) {
    event.stopPropagation();
    selectedId = node.id;
  }

  function handleWheel(event: WheelEvent) {
    if (!(event.ctrlKey || event.metaKey)) return;
    event.preventDefault();
    const rect = viewport.getBoundingClientRect();
    void setZoom(
      zoom + (event.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP),
      event.clientX - rect.left,
      event.clientY - rect.top,
    );
  }

  function handlePointerDown(event: PointerEvent) {
    const target = event.target as Element;
    if (target.closest(".lan-device")) return;
    if (event.button === 0 || event.button === 1) {
      selectedId = null;
      event.preventDefault();
      dragging = true;
      dragStartX = event.clientX;
      dragStartY = event.clientY;
      dragScrollLeft = viewport.scrollLeft;
      dragScrollTop = viewport.scrollTop;
      viewport.setPointerCapture(event.pointerId);
    }
  }

  function handlePointerMove(event: PointerEvent) {
    if (!dragging) return;
    viewport.scrollLeft = dragScrollLeft - (event.clientX - dragStartX);
    viewport.scrollTop = dragScrollTop - (event.clientY - dragStartY);
  }

  function handlePointerUp(event: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    if (viewport.hasPointerCapture(event.pointerId)) {
      viewport.releasePointerCapture(event.pointerId);
    }
  }

  function canvasInteractions(node: HTMLDivElement) {
    node.addEventListener("wheel", handleWheel, { passive: false });
    node.addEventListener("pointerdown", handlePointerDown);
    node.addEventListener("pointermove", handlePointerMove);
    node.addEventListener("pointerup", handlePointerUp);
    node.addEventListener("pointercancel", handlePointerUp);
    return {
      destroy() {
        node.removeEventListener("wheel", handleWheel);
        node.removeEventListener("pointerdown", handlePointerDown);
        node.removeEventListener("pointermove", handlePointerMove);
        node.removeEventListener("pointerup", handlePointerUp);
        node.removeEventListener("pointercancel", handlePointerUp);
      },
    };
  }

  function syncViewport() {
    if (!viewport) return;
    viewportWidth = viewport.clientWidth;
    viewportHeight = viewport.clientHeight;
  }

  onMount(() => {
    const observer = new ResizeObserver(syncViewport);
    observer.observe(viewport);
    syncViewport();
    void fitToView();
    return () => observer.disconnect();
  });
</script>

<svelte:head>
  <title>{title} | Hearthline</title>
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="brand-block">
      <button
        type="button"
        class="brand-back"
        aria-label="Back to Customer Network"
        title="Back to Customer Network"
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
      <span>Customer Network</span>
      <ChevronDown size={14} strokeWidth={1.8} />
      <strong>{title}</strong>
    </div>

    <div class="toolbar" aria-label={`${title} tools`}>
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

      <span class="toolbar-divider"></span>

      <div class="zoom-control" aria-label="Zoom controls">
        <button type="button" aria-label="Zoom out" title="Zoom out" disabled={zoom <= MIN_ZOOM} onclick={() => setZoom(zoom - ZOOM_STEP)}>
          <Minus size={17} strokeWidth={1.9} />
        </button>
        <button type="button" class="zoom-value" aria-label="Reset zoom" title="Reset zoom" onclick={resetView}>
          {Math.round(zoom * 100)}%
        </button>
        <button type="button" aria-label="Zoom in" title="Zoom in" disabled={zoom >= MAX_ZOOM} onclick={() => setZoom(zoom + ZOOM_STEP)}>
          <Plus size={17} strokeWidth={1.9} />
        </button>
      </div>

      <button type="button" aria-label="Fit to view" title="Fit to view" onclick={fitToView}>
        <Maximize2 size={17} strokeWidth={1.9} />
      </button>
      <button type="button" aria-label="Reset view" title="Reset view" onclick={resetView}>
        <RotateCcw size={17} strokeWidth={1.9} />
      </button>
      <button
        type="button"
        class:active={gridVisible}
        aria-pressed={gridVisible}
        aria-label="Toggle reference grid"
        title="Toggle reference grid"
        onclick={() => (gridVisible = !gridVisible)}
      >
        <Grid2X2 size={17} strokeWidth={1.9} />
      </button>
    </div>
  </header>

  <main class="workspace">
    <div
      class:is-dragging={dragging}
      class="viewport lan-viewport"
      bind:this={viewport}
      use:canvasInteractions
      role="region"
      aria-label={`${title} architecture`}
    >
      <div
        class="canvas-size"
        style={`width: ${Math.max(worldPixelWidth, viewportWidth)}px; height: ${Math.max(worldPixelHeight, viewportHeight)}px;`}
      >
        <section
          class:grid-visible={gridVisible}
          class:physical-view={viewMode === "physical"}
          class:logical-view={viewMode === "logical"}
          class:edge-focus={isEdge}
          class:public-path-focus={!isEdge}
          class="customer-focus-world"
          style={`left: ${worldOffsetX}px; top: ${worldOffsetY}px; width: ${WORLD_WIDTH}px; height: ${WORLD_HEIGHT}px; transform: scale(${zoom});`}
          aria-label={`${viewMode} ${title} view`}
        >
          <div class="lan-heading">
            <span>HEARTHLINE / CUSTOMER / {isEdge ? "EDGE" : "PUBLIC PATH"}</span>
            <h1>{title}</h1>
            <p>{subtitle}</p>
          </div>

          <svg class="focus-drawing" viewBox={`0 0 ${WORLD_WIDTH} ${WORLD_HEIGHT}`} aria-hidden="true">
            <defs>
              <marker id="focus-arrow" markerWidth="9" markerHeight="9" refX="8" refY="4.5" orient="auto">
                <path d="M0,0 L9,4.5 L0,9 Z"></path>
              </marker>
            </defs>

            {#if isEdge}
              <g class="focus-physical-layer">
                <rect class="edge-room" x="55" y="175" width="1060" height="575"></rect>
                <rect class="edge-cabinet" x="360" y="280" width="680" height="345"></rect>
                <rect class="edge-outside" x="1135" y="175" width="610" height="575"></rect>
                <path class="focus-cable" d="M233 452 H517 M573 452 H857 M913 452 H1197"></path>
                <path class="focus-dsl" d="M1253 452 H1537"></path>
                <text class="focus-zone-label" x="85" y="210">CUSTOMER PREMISES / NETWORK UTILITY</text>
                <text class="focus-zone-label" x="1165" y="210">PROVIDER ACCESS</text>
                <text class="focus-zone-label" x="530" y="315">EDGE CABINET</text>
              </g>
              <g class="focus-logical-layer">
                <rect class="focus-zone focus-private" x="50" y="180" width="710" height="560"></rect>
                <rect class="focus-zone focus-provider" x="780" y="180" width="970" height="560"></rect>
                <text class="focus-zone-label" x="80" y="215">NAT INSIDE · 192.168.0.0/24</text>
                <text class="focus-zone-label" x="810" y="215">NAT OUTSIDE · 203.0.113.0/24</text>
                <path class="focus-flow" d="M300 452 H450" marker-end="url(#focus-arrow)"></path>
                <path class="focus-flow" d="M640 452 H790" marker-end="url(#focus-arrow)"></path>
                <path class="focus-flow" d="M980 452 H1130" marker-end="url(#focus-arrow)"></path>
                <path class="focus-flow" d="M1320 452 H1470" marker-end="url(#focus-arrow)"></path>
                <path class="focus-boundary" d="M770 245 V680"></path>
                <text class="focus-boundary-label" x="770" y="704">PAT BOUNDARY · 192.168.0.0/24 → 203.0.113.2</text>
              </g>
            {:else}
              <g class="focus-physical-layer">
                <rect class="path-ground" x="25" y="170" width="1750" height="590"></rect>
                <rect class="path-site path-customer-site" x="45" y="235" width="390" height="455"></rect>
                <rect class="path-site path-isp-site" x="455" y="235" width="565" height="455"></rect>
                <rect class="path-site path-business-site" x="1040" y="235" width="715" height="455"></rect>

                <path class="path-house" d="M75 370 L170 290 L265 370 V650 H75 Z"></path>
                <rect class="path-demarc-cabinet" x="315" y="345" width="80" height="225"></rect>

                <path class="path-isp-building" d="M490 280 H980 V650 H490 Z"></path>
                <path class="path-rack-row" d="M520 320 H635 V380 H520 Z M675 320 H790 V380 H675 Z M830 320 H945 V380 H830 Z"></path>
                <path class="path-rack-slots" d="M535 340 H620 M690 340 H775 M845 340 H930"></path>

                <path class="path-office" d="M1070 280 H1725 V650 H1070 Z"></path>
                <path class="path-business-boundary" d="M1285 280 V650 M1515 280 V650"></path>
                <path class="path-office-window" d="M1110 330 H1220 M1350 330 H1450 M1580 330 H1685"></path>

                <path class="focus-cable" d="M165 457 H395"></path>
                <path class="focus-dsl" d="M395 457 H655"></path>
                <path class="focus-cable" d="M655 457 H915"></path>
                <path class="focus-dsl" d="M915 457 H1175"></path>
                <path class="focus-cable" d="M1175 457 H1420 M1420 457 H1665"></path>
                <path class="path-corridor-secondary" d="M655 457 V607"></path>

                <text class="focus-zone-label" x="70" y="265">CUSTOMER PREMISES</text>
                <text class="focus-zone-label" x="480" y="265">ISP POINT OF PRESENCE</text>
                <text class="focus-zone-label" x="1065" y="265">BUSINESS PERIMETER AND PUBLIC DMZ</text>
                <text class="path-detail-label" x="315" y="330">ACCESS CPE</text>
                <text class="path-detail-label" x="1090" y="310">EDGE</text>
                <text class="path-detail-label" x="1310" y="310">SECURITY BOUNDARY</text>
                <text class="path-detail-label" x="1540" y="310">PUBLIC SERVICE BAY</text>
              </g>
              <g class="focus-logical-layer">
                <rect class="path-zone customer-zone" x="30" y="180" width="480" height="560"></rect>
                <rect class="path-zone isp-zone" x="520" y="180" width="500" height="560"></rect>
                <rect class="path-zone business-zone" x="1030" y="180" width="740" height="560"></rect>
                <text class="focus-zone-label" x="60" y="215">CUSTOMER / 203.0.113.0/24</text>
                <text class="focus-zone-label" x="550" y="215">ISP ROUTING AND SERVICES</text>
                <text class="focus-zone-label" x="1060" y="215">BUSINESS PERIMETER AND PUBLIC DMZ</text>
                <path class="focus-flow" d="M235 457 H285" marker-end="url(#focus-arrow)"></path>
                <path class="focus-flow" d="M475 457 H525" marker-end="url(#focus-arrow)"></path>
                <path class="focus-flow" d="M715 457 H795" marker-end="url(#focus-arrow)"></path>
                <path class="focus-flow" d="M985 457 H1065" marker-end="url(#focus-arrow)"></path>
                <path class="focus-flow" d="M1255 457 H1335" marker-end="url(#focus-arrow)"></path>
                <path class="focus-flow" d="M1525 457 H1575" marker-end="url(#focus-arrow)"></path>
                <path class="focus-service-link" d="M620 510 V650"></path>
                <text class="focus-policy-label" x="1125" y="350">STATIC NAT · 192.0.2.10 → 172.16.10.2</text>
                <text class="focus-policy-label" x="1370" y="350">HTTPS · HTTP REDIRECT ONLY</text>
                <text class="focus-policy-label" x="740" y="690">DNS · www.business.example → 192.0.2.10</text>
              </g>
            {/if}
          </svg>

          {#each nodes as node (node.id)}
            {@const Icon = node.icon}
            {@const position = viewMode === "physical" ? node.physical : node.logical}
            <button
              type="button"
              class:selected={selectedId === node.id}
              class:physical-device-marker={viewMode === "physical"}
              class="lan-device focus-device"
              style={`left: ${position.x}px; top: ${position.y}px; --node-accent: ${node.accent};`}
              aria-label={`Inspect ${node.label}, ${node.area}`}
              title={`Inspect ${node.label}`}
              onclick={(event) => selectNode(event, node)}
            >
              {#if viewMode === "physical"}
                <PhysicalDeviceMarker icon={Icon} label={node.label} />
              {:else}
                <span class="node-accent"></span>
                <span class="lan-device-header">
                  <span class="node-icon"><Icon size={19} strokeWidth={1.8} /></span>
                  <small>{node.area}</small>
                </span>
                <strong>{node.label}</strong>
                <span>{node.address}</span>
              {/if}
            </button>
          {/each}

          <div class="lan-key" aria-label={`${title} legend`}>
            {#if viewMode === "physical"}
              <span><Cable size={13} strokeWidth={1.8} /><i class="cable-key copper"></i>Physical link</span>
              <span><i class="cable-key dsl"></i>Provider access</span>
            {:else if isEdge}
              <span><i class="zone-key private"></i>NAT inside</span>
              <span><i class="zone-key provider"></i>NAT outside</span>
            {:else}
              <span><i class="cable-key copper"></i>Forward path</span>
              <span><ShieldCheck size={13} strokeWidth={1.8} />Policy boundary</span>
            {/if}
          </div>
        </section>
      </div>
    </div>

    {#if selectedNode}
      <aside class="lan-inspector" aria-label="Selected environment node">
        <div class="lan-inspector-header">
          <div>
            <span>{selectedNode.area}</span>
            <h2>{selectedNode.label}</h2>
          </div>
          <button type="button" aria-label="Close details" title="Close" onclick={() => (selectedId = null)}>
            <X size={17} strokeWidth={1.9} />
          </button>
        </div>
        <p>{selectedNode.role}</p>
        <dl>
          <div>
            <dt>Addressing</dt>
            <dd>{selectedNode.address}</dd>
          </div>
          <div>
            <dt>Environment</dt>
            <dd>{title}</dd>
          </div>
        </dl>
        <div class="lan-port-list">
          <span>Configuration facts</span>
          {#each selectedNode.facts as fact}
            <div><Wifi size={14} strokeWidth={1.8} />{fact}</div>
          {/each}
        </div>
        <ApplianceConfigSummary
          appliances={selectedAppliances}
          onOpen={onOpenAppliance}
        />
      </aside>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>{title} model</span>
    <span>{nodes.length} nodes / production-shaped architecture target</span>
    <span>{viewMode === "physical" ? "Physical" : "Logical"} / {Math.round(zoom * 100)}%</span>
  </footer>
</div>

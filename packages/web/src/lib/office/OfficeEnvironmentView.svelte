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
  import {
    findAppliancesForNode,
    isInteractiveSecurityConsole,
    isInteractiveWorkstation,
  } from "../config/appliance-config";
  import PhysicalDeviceMarker from "../shared/PhysicalDeviceMarker.svelte";
  import type { ViewMode } from "../shared/types";
  import { businessItNodes } from "./model/business-it";
  import { itDmzNodes } from "./model/it-dmz";
  import { operationsIntelligenceNodes } from "./model/operations-intelligence";
  import { otDmzNodes } from "./model/ot-dmz";
  import type { OfficeEnvironment, OfficeNode } from "./model/types";
  import OfficeEnvironmentDrawing from "./OfficeEnvironmentDrawing.svelte";

  export let environment: OfficeEnvironment;
  export let onBack: () => void = () => {};
  export let onOpenAppliance: (id: string) => void = () => {};
  export let onOpenSecurityConsole: (id: string) => void = () => {};
  export let onOpenWorkstation: (id: string) => void = () => {};
  export let siteLabel = "Central Office";
  export let viewMode: ViewMode = "logical";

  const WORLD_WIDTH = 1800;
  const WORLD_HEIGHT = 900;
  const MIN_ZOOM = 0.22;
  const MAX_ZOOM = 1.6;
  const ZOOM_STEP = 0.1;

  $: nodes =
    environment === "it-dmz"
      ? itDmzNodes
      : environment === "business-it"
        ? businessItNodes
        : environment === "operations-intelligence"
          ? operationsIntelligenceNodes
          : otDmzNodes;
  $: title =
    environment === "it-dmz"
      ? "Business IT DMZ"
      : environment === "business-it"
        ? "Business IT"
        : environment === "operations-intelligence"
          ? "Operations Intelligence"
          : "Factory OT DMZ";
  $: subtitle =
    viewMode === "physical"
      ? environment === "it-dmz"
        ? "WAN demarcation, perimeter rack, public service bay, and internal handoff"
        : environment === "business-it"
          ? "Office floor plan, secured infrastructure rooms, work areas, and controlled access"
          : environment === "operations-intelligence"
            ? "Central network, security, data, process-analysis, and change-governance workspaces"
            : "Factory-local secure-access, exchange, monitoring, and OT boundary facilities"
      : environment === "it-dmz"
        ? "Public static NAT, perimeter policy, DMZ isolation, and Business IT handoff"
        : environment === "business-it"
          ? "Collapsed core, six internal VLANs, shared services, and explicit trust boundaries"
          : environment === "operations-intelligence"
            ? "Brokered factory data, enterprise decision services, and governed change workflows"
            : "Independent factory policy boundaries and separated Level 3.5 service subzones";

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
  $: configView =
    environment === "ot-dmz" ? "factory/ot-dmz" : `office/${environment}`;
  $: selectedAppliances = selectedNode
    ? findAppliancesForNode(configView, selectedNode.id, viewMode)
    : [];
  $: selectedOperable = selectedAppliances.some(
    (appliance) =>
      isInteractiveWorkstation(appliance.id) ||
      isInteractiveSecurityConsole(appliance.id),
  );
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

  function selectNode(event: MouseEvent, node: OfficeNode) {
    event.stopPropagation();
    selectedId = node.id;
  }

  function openOperation(id: string) {
    if (isInteractiveWorkstation(id)) {
      onOpenWorkstation(id);
    } else if (isInteractiveSecurityConsole(id)) {
      onOpenSecurityConsole(id);
    }
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
        aria-label={`Back to ${siteLabel}`}
        title={`Back to ${siteLabel}`}
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
      <span>{siteLabel}</span>
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
          class:it-dmz-focus={environment === "it-dmz"}
          class:business-it-focus={environment === "business-it"}
          class:operations-intelligence-focus={environment === "operations-intelligence"}
          class:ot-dmz-focus={environment === "ot-dmz"}
          class="office-focus-world"
          style={`left: ${worldOffsetX}px; top: ${worldOffsetY}px; width: ${WORLD_WIDTH}px; height: ${WORLD_HEIGHT}px; transform: scale(${zoom});`}
          aria-label={`${viewMode} ${title} view`}
        >
          <div class="lan-heading">
            <span>HEARTHLINE / {siteLabel.toUpperCase()} / {environment.toUpperCase()}</span>
            <h1>{title}</h1>
            <p>{subtitle}</p>
          </div>

          <OfficeEnvironmentDrawing {environment} />

          {#each nodes as node (node.id)}
            {@const Icon = node.icon}
            {@const position = viewMode === "physical" ? node.physical : node.logical}
            <button
              type="button"
              class:selected={selectedId === node.id}
              class:physical-device-marker={viewMode === "physical"}
              class="lan-device office-device"
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
              {#if environment === "ot-dmz"}
                <span><i class="cable-key redundant"></i>Redundant path</span>
              {/if}
            {:else}
              {#if environment === "operations-intelligence"}
                <span><i class="cable-key data"></i>Brokered factory data</span>
                <span><i class="cable-key change"></i>Approved change workflow</span>
              {:else}
                <span><i class="cable-key copper"></i>Approved conduit</span>
                <span><ShieldCheck size={13} strokeWidth={1.8} />Policy boundary</span>
              {/if}
            {/if}
          </div>
        </section>
      </div>
    </div>

    {#if selectedNode}
      <aside class="lan-inspector" aria-label="Selected office node">
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
          <span>Architecture facts</span>
          {#each selectedNode.facts as fact}
            <div><Wifi size={14} strokeWidth={1.8} />{fact}</div>
          {/each}
        </div>
        <ApplianceConfigSummary
          appliances={selectedAppliances}
          onOpen={onOpenAppliance}
          onOperate={selectedOperable ? openOperation : null}
        />
      </aside>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>{title} model</span>
    <span>{nodes.length} nodes / architecture baseline</span>
    <span>{viewMode === "physical" ? "Physical" : "Logical"} / {Math.round(zoom * 100)}%</span>
  </footer>
</div>

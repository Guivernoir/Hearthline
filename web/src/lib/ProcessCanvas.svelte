<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Component } from "svelte";
  import {
    ArrowLeft,
    ArrowRight,
    Check,
    ChevronDown,
    Grid2X2,
    Hand,
    Layers3,
    Map,
    Maximize2,
    Minus,
    MousePointer2,
    Network,
    Plus,
    RotateCcw,
    X,
  } from "@lucide/svelte";
  import ApplianceConfigSummary from "./ApplianceConfigSummary.svelte";
  import PhysicalDeviceMarker from "./PhysicalDeviceMarker.svelte";
  import { findAppliancesForNode } from "./appliance-config";
  import { processIconByKey } from "./process-icons";
  import { findProcessArea, processView } from "./process-model";

  type Mode = "select" | "pan";
  type NodeKind = "boundary" | "platform" | "process";
  type ViewMode = "physical" | "logical";

  export let onBack: () => void = () => {};
  export let onEnterArea: (routeKey: string) => void = () => {};
  export let onOpenAppliance: (id: string) => void = () => {};
  export let viewMode: ViewMode = "logical";

  interface DiagramNode {
    id: string;
    label: string;
    subtitle: string;
    zone: string;
    x: number;
    y: number;
    width: number;
    height: number;
    accent: string;
    kind: NodeKind;
    icon: Component<any>;
    tags: string[];
    detail: string;
    routeKey?: string;
  }

  const WORLD_WIDTH = 1640;
  const WORLD_HEIGHT = 1080;
  const MIN_ZOOM = 0.2;
  const MAX_ZOOM = 1.6;
  const ZOOM_STEP = 0.1;
  const MINIMAP_WIDTH = 192;
  const MINIMAP_SCALE = MINIMAP_WIDTH / WORLD_WIDTH;

  const nodes: DiagramNode[] = [
    ...processView.supportNodes.map((node) => ({
      ...node,
      ...node.position,
      icon: processIconByKey[node.icon],
    })),
    ...processView.areas.map((area) => ({
      ...area,
      ...area.position,
      kind: "process" as const,
      icon: processIconByKey[area.icon],
    })),
  ];

  let viewport: HTMLDivElement;
  let zoom = 0.76;
  let mode: Mode = "select";
  let gridVisible = true;
  let selectedId: string | null = null;
  let inspectorVisible = false;
  let dragging = false;
  let spacePressed = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragScrollLeft = 0;
  let dragScrollTop = 0;
  let scrollLeft = 0;
  let scrollTop = 0;
  let viewportWidth = 1;
  let viewportHeight = 1;

  $: selectedNode = nodes.find((node) => node.id === selectedId) ?? null;
  $: selectedArea = selectedNode?.routeKey
    ? findProcessArea(selectedNode.routeKey)
    : null;
  $: selectedAppliances = selectedNode
    ? findAppliancesForNode("factory/process", selectedNode.id, viewMode)
    : [];
  $: worldPixelWidth = WORLD_WIDTH * zoom;
  $: worldPixelHeight = WORLD_HEIGHT * zoom;
  $: worldOffsetX = Math.max(0, (viewportWidth - worldPixelWidth) / 2);
  $: worldOffsetY = Math.max(0, (viewportHeight - worldPixelHeight) / 2);
  $: minimapViewport = {
    x: Math.max(0, ((scrollLeft - worldOffsetX) / zoom) * MINIMAP_SCALE),
    y: Math.max(0, ((scrollTop - worldOffsetY) / zoom) * MINIMAP_SCALE),
    width: Math.min(MINIMAP_WIDTH, (viewportWidth / zoom) * MINIMAP_SCALE),
    height: Math.min(WORLD_HEIGHT * MINIMAP_SCALE, (viewportHeight / zoom) * MINIMAP_SCALE),
  };

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
    syncViewport();
  }

  async function fitToView() {
    if (!viewport) return;

    const compactLayout = viewport.clientWidth < 620;
    const horizontalPadding = viewport.clientWidth < 720 ? 28 : 96;
    const verticalPadding = viewport.clientHeight < 620 ? 28 : 72;
    const fittedZoom = Math.min(
      (viewport.clientWidth - horizontalPadding) / WORLD_WIDTH,
      (viewport.clientHeight - verticalPadding) / WORLD_HEIGHT,
    );
    const nextZoom = clampZoom(compactLayout ? Math.max(0.65, fittedZoom) : fittedZoom);

    zoom = Number(nextZoom.toFixed(2));
    await tick();
    if (!viewport) return;
    viewport.scrollLeft = compactLayout
      ? 0
      : Math.max(0, (WORLD_WIDTH * zoom - viewport.clientWidth) / 2);
    viewport.scrollTop = Math.max(0, (WORLD_HEIGHT * zoom - viewport.clientHeight) / 2);
    syncViewport();
  }

  async function resetView() {
    zoom = 1;
    await tick();
    if (!viewport) return;
    viewport.scrollLeft = 0;
    viewport.scrollTop = 0;
    syncViewport();
  }

  async function centerOnNode(node: DiagramNode) {
    if (!viewport) return;
    viewport.scrollTo({
      left: (node.x + node.width / 2) * zoom + worldOffsetX - viewport.clientWidth / 2,
      top: (node.y + node.height / 2) * zoom + worldOffsetY - viewport.clientHeight / 2,
      behavior: "smooth",
    });
  }

  function selectNode(event: MouseEvent, node: DiagramNode) {
    if (mode === "pan" || spacePressed) return;
    event.stopPropagation();
    selectedId = node.id;
    inspectorVisible = true;
  }

  function handleWheel(event: WheelEvent) {
    if (!(event.ctrlKey || event.metaKey)) return;
    event.preventDefault();
    const direction = event.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    const rect = viewport.getBoundingClientRect();
    void setZoom(zoom + direction, event.clientX - rect.left, event.clientY - rect.top);
  }

  function handlePointerDown(event: PointerEvent) {
    const shouldPan = mode === "pan" || spacePressed || event.button === 1;
    if (!shouldPan) {
      const target = event.target as Element;
      if (!target.closest(".diagram-node")) {
        selectedId = null;
        inspectorVisible = false;
      }
      return;
    }

    event.preventDefault();
    dragging = true;
    dragStartX = event.clientX;
    dragStartY = event.clientY;
    dragScrollLeft = viewport.scrollLeft;
    dragScrollTop = viewport.scrollTop;
    viewport.setPointerCapture(event.pointerId);
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
    scrollLeft = viewport.scrollLeft;
    scrollTop = viewport.scrollTop;
    viewportWidth = viewport.clientWidth;
    viewportHeight = viewport.clientHeight;
  }

  function handleMinimapClick(event: MouseEvent) {
    const target = event.currentTarget as HTMLButtonElement;
    const rect = target.getBoundingClientRect();
    const worldX = (event.clientX - rect.left) / MINIMAP_SCALE;
    const worldY = (event.clientY - rect.top) / MINIMAP_SCALE;

    viewport.scrollTo({
      left: worldX * zoom + worldOffsetX - viewport.clientWidth / 2,
      top: worldY * zoom + worldOffsetY - viewport.clientHeight / 2,
      behavior: "smooth",
    });
  }

  function handleKeyDown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (target.matches("input, textarea, select")) return;

    if (event.code === "Space") {
      spacePressed = true;
      event.preventDefault();
    } else if (event.key === "+" || event.key === "=") {
      event.preventDefault();
      void setZoom(zoom + ZOOM_STEP);
    } else if (event.key === "-") {
      event.preventDefault();
      void setZoom(zoom - ZOOM_STEP);
    } else if (event.key === "0") {
      event.preventDefault();
      void resetView();
    } else if (event.key.toLowerCase() === "f") {
      event.preventDefault();
      void fitToView();
    } else if (event.key === "Escape") {
      selectedId = null;
      inspectorVisible = false;
    }
  }

  function handleKeyUp(event: KeyboardEvent) {
    if (event.code === "Space") {
      spacePressed = false;
    }
  }

  onMount(() => {
    const observer = new ResizeObserver(syncViewport);
    observer.observe(viewport);
    viewport.addEventListener("scroll", syncViewport, { passive: true });
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    syncViewport();
    void fitToView();

    return () => {
      observer.disconnect();
      viewport.removeEventListener("scroll", syncViewport);
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  });
</script>

<svelte:head>
  <title>Hearthline Architecture</title>
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="brand-block">
      <button
        type="button"
        class="brand-back"
        aria-label="Back to Factory"
        title="Back to Factory"
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
      <strong>Ceramics process</strong>
    </div>

    <div class="toolbar" aria-label="Canvas tools">
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

      <div class="segmented-control" aria-label="Interaction mode">
        <button
          type="button"
          class:active={mode === "select"}
          aria-pressed={mode === "select"}
          aria-label="Select mode"
          title="Select mode"
          onclick={() => (mode = "select")}
        >
          <MousePointer2 size={17} strokeWidth={1.9} />
        </button>
        <button
          type="button"
          class:active={mode === "pan"}
          aria-pressed={mode === "pan"}
          aria-label="Pan mode"
          title="Pan mode"
          onclick={() => (mode = "pan")}
        >
          <Hand size={17} strokeWidth={1.9} />
        </button>
      </div>

      <span class="toolbar-divider"></span>

      <div class="zoom-control" aria-label="Zoom controls">
        <button
          type="button"
          aria-label="Zoom out"
          title="Zoom out"
          disabled={zoom <= MIN_ZOOM}
          onclick={() => setZoom(zoom - ZOOM_STEP)}
        >
          <Minus size={17} strokeWidth={1.9} />
        </button>
        <button
          type="button"
          class="zoom-value"
          aria-label="Reset zoom to 100 percent"
          title="Reset zoom to 100%"
          onclick={() => resetView()}
        >
          {Math.round(zoom * 100)}%
        </button>
        <button
          type="button"
          aria-label="Zoom in"
          title="Zoom in"
          disabled={zoom >= MAX_ZOOM}
          onclick={() => setZoom(zoom + ZOOM_STEP)}
        >
          <Plus size={17} strokeWidth={1.9} />
        </button>
      </div>

      <button type="button" aria-label="Fit to view" title="Fit to view" onclick={fitToView}>
        <Maximize2 size={17} strokeWidth={1.9} />
      </button>
      <button type="button" aria-label="Reset view" title="Reset view" onclick={resetView}>
        <RotateCcw size={17} strokeWidth={1.9} />
      </button>

      <span class="toolbar-divider"></span>

      <button
        type="button"
        class:active={gridVisible}
        aria-pressed={gridVisible}
        aria-label="Toggle grid"
        title="Toggle grid"
        onclick={() => (gridVisible = !gridVisible)}
      >
        <Grid2X2 size={17} strokeWidth={1.9} />
      </button>
    </div>
  </header>

  <main class="workspace">
    <div
      class:pan-mode={mode === "pan" || spacePressed}
      class:is-dragging={dragging}
      class="viewport"
      bind:this={viewport}
      use:canvasInteractions
      role="region"
      aria-label="Hearthline architecture canvas"
    >
      <div
        class="canvas-size"
        style={`width: ${Math.max(worldPixelWidth, viewportWidth)}px; height: ${Math.max(worldPixelHeight, viewportHeight)}px;`}
      >
        <section
          class:grid-visible={gridVisible}
          class:physical-view={viewMode === "physical"}
          class:logical-view={viewMode === "logical"}
          class="diagram-world"
          style={`left: ${worldOffsetX}px; top: ${worldOffsetY}px; width: ${WORLD_WIDTH}px; height: ${WORLD_HEIGHT}px; transform: scale(${zoom});`}
          aria-label="OT process architecture"
        >
          <div class="canvas-heading">
            <span>HEARTHLINE / OT</span>
            <h1>Ceramics process architecture</h1>
            <p>{viewMode === "physical" ? "Production sequence and material flow" : "Control zones and network relationships"}</p>
          </div>

          <div class="diagram-legend" aria-label="Diagram legend">
            {#if viewMode === "logical"}
              <span><i class="legend-line network-line"></i>Control network</span>
            {:else}
              <span><i class="legend-line material-line"></i>Material flow</span>
              <span><i class="legend-line support-line"></i>Site / control link</span>
            {/if}
          </div>

          <svg
            class="connections"
            viewBox={`0 0 ${WORLD_WIDTH} ${WORLD_HEIGHT}`}
            aria-hidden="true"
          >
            <defs>
              <marker
                id="network-arrow"
                markerWidth="8"
                markerHeight="8"
                refX="7"
                refY="4"
                orient="auto"
              >
                <path d="M0,0 L8,4 L0,8 Z" fill="#4d7379"></path>
              </marker>
              <marker
                id="material-arrow"
                markerWidth="9"
                markerHeight="9"
                refX="8"
                refY="4.5"
                orient="auto"
              >
                <path d="M0,0 L9,4.5 L0,9 Z" fill="#c76432"></path>
              </marker>
            </defs>

            <g class="process-physical-background">
              <rect class="process-site-shell" x="25" y="90" width="1590" height="850"></rect>
              <rect class="process-support-gallery" x="25" y="90" width="1590" height="175"></rect>
              <rect class="process-production-hall" x="25" y="280" width="1590" height="660"></rect>
              <rect class="process-utility-corridor" x="45" y="615" width="1550" height="82"></rect>

              <rect class="process-cell process-cell-wet" x="40" y="370" width="285" height="230"></rect>
              <rect class="process-cell process-cell-forming" x="350" y="370" width="285" height="230"></rect>
              <rect class="process-cell process-cell-drying" x="660" y="370" width="285" height="230"></rect>
              <rect class="process-cell process-cell-thermal" x="970" y="370" width="285" height="230"></rect>
              <rect class="process-cell process-cell-finish" x="1280" y="370" width="285" height="230"></rect>

              <rect class="process-cell process-cell-logistics" x="40" y="710" width="285" height="210"></rect>
              <rect class="process-cell process-cell-quality" x="350" y="710" width="285" height="210"></rect>
              <rect class="process-cell process-cell-thermal" x="660" y="710" width="285" height="210"></rect>
              <rect class="process-cell process-cell-quality" x="970" y="710" width="285" height="210"></rect>
              <rect class="process-cell process-cell-thermal" x="1280" y="710" width="285" height="210"></rect>

              <path class="process-overhead-utility" d="M80 640 H1560"></path>
              <path class="process-floor-rail" d="M75 585 H1565 M75 902 H1565"></path>
              <path class="process-loading-bay" d="M55 750 H100 V885 H55 M265 750 H310 V885 H265"></path>
              <path class="process-exhaust-stack" d="M1120 365 V305 H1150 V365 M1430 705 V645 H1460 V705"></path>

              <text class="process-physical-label" x="50" y="112">CONTROL AND SITE SERVICES GALLERY</text>
              <text class="process-physical-label" x="50" y="310">CERAMICS PRODUCTION HALL</text>
              <text class="process-physical-label" x="70" y="666">UTILITY, MATERIAL, AND MAINTENANCE CORRIDOR</text>
              <text class="process-detail-label" x="70" y="736">PACKING / DISPATCH BAY</text>
              <text class="process-detail-label" x="1320" y="736">PRIMARY FIRING BAY</text>
            </g>

            <g class="network-connections">
              <path d="M260 165 H350" marker-end="url(#network-arrow)"></path>
              <path d="M570 165 H660" marker-end="url(#network-arrow)"></path>
              <path d="M880 165 H970" marker-end="url(#network-arrow)"></path>
              <path d="M1190 165 H1280" marker-end="url(#network-arrow)"></path>
              <path d="M1080 225 V355"></path>
              <path d="M180 355 H1420"></path>
              <path d="M180 690 H1420"></path>
              <path d="M1265 355 V690"></path>
              <path d="M180 355 V430"></path>
              <path d="M490 355 V430"></path>
              <path d="M800 355 V430"></path>
              <path d="M1110 355 V430"></path>
              <path d="M1420 355 V430"></path>
              <path d="M180 690 V760"></path>
              <path d="M490 690 V760"></path>
              <path d="M800 690 V760"></path>
              <path d="M1110 690 V760"></path>
              <path d="M1420 690 V760"></path>
            </g>

            <g class="material-connections">
              <path d="M215 495 H455" marker-end="url(#material-arrow)"></path>
              <path d="M525 495 H765" marker-end="url(#material-arrow)"></path>
              <path d="M835 495 H1075" marker-end="url(#material-arrow)"></path>
              <path d="M1145 495 H1385" marker-end="url(#material-arrow)"></path>
              <path d="M1420 530 V790" marker-end="url(#material-arrow)"></path>
              <path d="M1385 825 H1145" marker-end="url(#material-arrow)"></path>
              <path d="M1075 825 H835" marker-end="url(#material-arrow)"></path>
              <path d="M765 825 H525" marker-end="url(#material-arrow)"></path>
              <path d="M455 825 H215" marker-end="url(#material-arrow)"></path>
            </g>

            <g class="physical-support-connections">
              <path class="site-support-link" d="M177 156 H433"></path>
              <path class="site-support-link" d="M487 156 H743"></path>
              <path d="M797 156 H1053"></path>
              <path d="M1107 156 H1363"></path>
            </g>

            <text x="74" y="335" class="line-label network-label">CONTROL NETWORK</text>
            <text x="72" y="405" class="line-label material-label">MATERIAL FLOW</text>
          </svg>

          {#each nodes as node (node.id)}
            {@const Icon = node.icon}
            <button
              type="button"
              class:boundary-node={node.kind === "boundary"}
              class:platform-node={node.kind === "platform"}
              class:process-node={node.kind === "process"}
              class:selected={selectedId === node.id}
              class:physical-device-marker={viewMode === "physical"}
              class="diagram-node"
              style={`left: ${node.x}px; top: ${node.y}px; width: ${node.width}px; height: ${node.height}px; --node-accent: ${node.accent};`}
              aria-label={`${node.label}, ${node.subtitle}`}
              title={`Inspect ${node.label}`}
              onclick={(event) => selectNode(event, node)}
              ondblclick={() => node.routeKey ? onEnterArea(node.routeKey) : centerOnNode(node)}
            >
              {#if viewMode === "physical"}
                <PhysicalDeviceMarker icon={Icon} label={node.label} />
              {:else}
                <span class="node-accent"></span>
                <span class="node-header">
                  <span class="node-icon"><Icon size={19} strokeWidth={1.8} /></span>
                  <span class="node-zone">{node.zone}</span>
                </span>
                <strong>{node.label}</strong>
                <span class="node-subtitle">{node.subtitle}</span>
                <span class="node-tags">
                  {#each node.tags as tag}
                    <span>{tag}</span>
                  {/each}
                </span>
              {/if}
            </button>
          {/each}
        </section>
      </div>
    </div>

    {#if inspectorVisible && selectedNode}
      <aside class="inspector" aria-label="Selected asset details">
        <div class="inspector-header">
          <div>
            <span>{selectedNode.zone}</span>
            <h2>{selectedNode.label}</h2>
          </div>
          <button
            type="button"
            aria-label="Close inspector"
            title="Close inspector"
            onclick={() => (inspectorVisible = false)}
          >
            <X size={17} strokeWidth={1.9} />
          </button>
        </div>
        <p>{selectedNode.detail}</p>
        <dl>
          <div>
            <dt>Role</dt>
            <dd>{selectedNode.kind}</dd>
          </div>
          <div>
            <dt>Model</dt>
            <dd>Draft</dd>
          </div>
        </dl>
        <div class="inspector-tags">
          {#each selectedNode.tags as tag}
            <span><Check size={13} strokeWidth={2} />{tag}</span>
          {/each}
        </div>
        <ApplianceConfigSummary
          appliances={selectedAppliances}
          onOpen={onOpenAppliance}
        />
        {#if selectedArea}
          <button
            type="button"
            class="inspector-action"
            onclick={() => onEnterArea(selectedArea.routeKey)}
          >
            Enter process area
            <ArrowRight size={15} strokeWidth={1.9} />
          </button>
        {/if}
      </aside>
    {/if}

    <button
      type="button"
      class="minimap"
      aria-label="Architecture minimap"
      title="Center minimap location"
      onclick={handleMinimapClick}
      style={`--minimap-height: ${WORLD_HEIGHT * MINIMAP_SCALE}px;`}
    >
      <span class="minimap-title"><Layers3 size={13} strokeWidth={1.8} />Overview</span>
      <span class="minimap-world">
        {#each nodes as node (node.id)}
          <i
            class:process={node.kind === "process"}
            style={`left: ${node.x * MINIMAP_SCALE}px; top: ${node.y * MINIMAP_SCALE}px; width: ${Math.max(5, node.width * MINIMAP_SCALE)}px; height: ${Math.max(4, node.height * MINIMAP_SCALE)}px; --mini-accent: ${node.accent};`}
          ></i>
        {/each}
        <span
          class="minimap-viewport"
          style={`left: ${minimapViewport.x}px; top: ${minimapViewport.y}px; width: ${minimapViewport.width}px; height: ${minimapViewport.height}px;`}
        ></span>
      </span>
    </button>
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>Draft topology</span>
    <span>{viewMode === "physical" ? "Physical process" : "Logical architecture"} / {processView.areas.length} process areas</span>
    <span>{Math.round(zoom * 100)}% / {WORLD_WIDTH} x {WORLD_HEIGHT}</span>
  </footer>
</div>

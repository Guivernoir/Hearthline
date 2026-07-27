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
    X,
  } from "@lucide/svelte";
  import PhysicalDeviceMarker from "./PhysicalDeviceMarker.svelte";
  import { processIconByKey } from "./process-icons";
  import {
    findProcessArea,
    SUPPORTED_PROCESS_VIEW_SCHEMA,
  } from "./process-model";
  import type { ProcessEquipment } from "./process-model";
  import type { ViewMode } from "./types";

  interface EquipmentPosition {
    x: number;
    y: number;
  }

  interface EquipmentView extends ProcessEquipment, EquipmentPosition {}

  interface EquipmentPresentation {
    label: string;
    kind: string;
    role: string;
    icon: ProcessEquipment["icon"];
    configRef: string;
    facts: string[];
  }

  export let routeKey: string;
  export let onBack: () => void = () => {};
  export let viewMode: ViewMode = "logical";

  const WORLD_WIDTH = 1280;
  const WORLD_HEIGHT = 900;
  const MIN_ZOOM = 0.25;
  const MAX_ZOOM = 1.6;
  const ZOOM_STEP = 0.1;
  const NODE_WIDTH = 190;
  const NODE_HEIGHT = 105;

  const physicalSlotPositions: Record<ProcessEquipment["slot"], EquipmentPosition> = {
    controller: { x: 70, y: 155 },
    hmi: { x: 315, y: 155 },
    switch: { x: 70, y: 400 },
    "remote-io": { x: 315, y: 400 },
    "sensor-a": { x: 620, y: 210 },
    "sensor-b": { x: 620, y: 535 },
    "actuator-a": { x: 965, y: 210 },
    "actuator-b": { x: 965, y: 535 },
    safety: { x: 965, y: 710 },
  };

  const logicalSlotPositions: Record<ProcessEquipment["slot"], EquipmentPosition> = {
    switch: { x: 70, y: 390 },
    controller: { x: 315, y: 390 },
    hmi: { x: 315, y: 145 },
    "remote-io": { x: 620, y: 390 },
    "sensor-a": { x: 620, y: 185 },
    "sensor-b": { x: 620, y: 585 },
    "actuator-a": { x: 965, y: 185 },
    "actuator-b": { x: 965, y: 585 },
    safety: { x: 965, y: 710 },
  };

  let viewport: HTMLDivElement;
  let zoom = 0.9;
  let gridVisible = true;
  let selectedId: string | null = null;
  let dragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragScrollLeft = 0;
  let dragScrollTop = 0;
  let viewportWidth = 1;
  let viewportHeight = 1;

  $: area = findProcessArea(routeKey);
  $: equipment = (area?.equipment ?? []).map((item) => ({
    ...item,
    ...(viewMode === "physical"
      ? physicalSlotPositions[item.slot]
      : logicalSlotPositions[item.slot]),
  })) as EquipmentView[];
  $: selectedEquipment = equipment.find((item) => item.id === selectedId) ?? null;
  $: selectedPresentation = selectedEquipment
    ? equipmentPresentation(selectedEquipment)
    : null;
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
    const nextOffsetX = Math.max(0, (viewport.clientWidth - WORLD_WIDTH * zoom) / 2);
    const nextOffsetY = Math.max(0, (viewport.clientHeight - WORLD_HEIGHT * zoom) / 2);
    viewport.scrollLeft = worldX * zoom + nextOffsetX - focusX;
    viewport.scrollTop = worldY * zoom + nextOffsetY - focusY;
  }

  async function fitToView() {
    if (!viewport) return;
    const compact = viewport.clientWidth < 620;
    const horizontalPadding = compact ? 24 : 80;
    const verticalPadding = compact ? 24 : 64;
    const fitted = Math.min(
      (viewport.clientWidth - horizontalPadding) / WORLD_WIDTH,
      (viewport.clientHeight - verticalPadding) / WORLD_HEIGHT,
    );

    zoom = Number(clampZoom(compact ? Math.max(0.65, fitted) : fitted).toFixed(2));
    await tick();
    viewport.scrollLeft = compact
      ? 0
      : Math.max(0, (WORLD_WIDTH * zoom - viewport.clientWidth) / 2);
    viewport.scrollTop = Math.max(0, (WORLD_HEIGHT * zoom - viewport.clientHeight) / 2);
  }

  async function resetView() {
    zoom = 1;
    await tick();
    viewport.scrollLeft = 0;
    viewport.scrollTop = 0;
  }

  function equipmentCenter(item: EquipmentView) {
    return {
      x: item.x + NODE_WIDTH / 2,
      y: item.y + NODE_HEIGHT / 2,
    };
  }

  function upstreamFor(item: EquipmentView) {
    return viewMode === "physical" && item.physicalUpstream !== undefined
      ? item.physicalUpstream
      : item.upstream;
  }

  function equipmentPresentation(item: ProcessEquipment): EquipmentPresentation {
    if (viewMode === "physical" && item.physical) {
      return item.physical;
    }

    return {
      label: item.label,
      kind: item.kind,
      role: item.role,
      icon: item.icon,
      configRef: item.configRef,
      facts: item.facts,
    };
  }

  function connectionPath(item: EquipmentView) {
    const upstream = equipment.find((candidate) => candidate.id === upstreamFor(item));
    if (!upstream) return "";
    const source = equipmentCenter(upstream);
    const target = equipmentCenter(item);
    if (item.linkKind === "safety-status") {
      const lowerRoute = 810;
      return `M${source.x} ${source.y} V${lowerRoute} H${target.x} V${target.y}`;
    }
    const midpoint = source.x + (target.x - source.x) / 2;
    return `M${source.x} ${source.y} H${midpoint} V${target.y} H${target.x}`;
  }

  function handleWheel(event: WheelEvent) {
    if (!(event.ctrlKey || event.metaKey)) return;
    event.preventDefault();
    const rect = viewport.getBoundingClientRect();
    const direction = event.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP;
    void setZoom(zoom + direction, event.clientX - rect.left, event.clientY - rect.top);
  }

  function handlePointerDown(event: PointerEvent) {
    const target = event.target as Element;
    if (!target.closest(".process-area-device")) {
      selectedId = null;
    }
    if (event.button !== 1) return;
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

  onMount(() => {
    const observer = new ResizeObserver(() => {
      viewportWidth = viewport.clientWidth;
      viewportHeight = viewport.clientHeight;
    });
    observer.observe(viewport);
    viewportWidth = viewport.clientWidth;
    viewportHeight = viewport.clientHeight;
    void fitToView();
    return () => observer.disconnect();
  });
</script>

<svelte:head>
  <title>{area?.label ?? "Process area"} · Hearthline</title>
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="brand-block">
      <button
        type="button"
        class="brand-back"
        aria-label="Back to ceramics process"
        title="Back to ceramics process"
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
      <span>Factory / Ceramics process</span>
      <ChevronDown size={14} strokeWidth={1.8} />
      <strong>{area?.label ?? "Unknown area"}</strong>
    </div>

    <div class="toolbar" aria-label="Area view controls">
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
    {#if area}
      <div
        class:is-dragging={dragging}
        class="viewport lan-viewport"
        bind:this={viewport}
        use:canvasInteractions
        role="region"
        aria-label={`${area.label} equipment architecture`}
      >
        <div
          class="canvas-size"
          style={`width: ${Math.max(worldPixelWidth, viewportWidth)}px; height: ${Math.max(worldPixelHeight, viewportHeight)}px;`}
        >
          <section
            class:grid-visible={gridVisible}
            class:physical-view={viewMode === "physical"}
            class:logical-view={viewMode === "logical"}
            class="process-area-world"
            style={`left: ${worldOffsetX}px; top: ${worldOffsetY}px; width: ${WORLD_WIDTH}px; height: ${WORLD_HEIGHT}px; transform: scale(${zoom});`}
            aria-label={`${viewMode} ${area.label} view`}
          >
            <div class="lan-heading">
              <span>HEARTHLINE / FACTORY / {area.zone}</span>
              <h1>{area.label}</h1>
              <p>{area.subtitle}</p>
            </div>

            <svg class="process-area-drawing" viewBox={`0 0 ${WORLD_WIDTH} ${WORLD_HEIGHT}`} aria-hidden="true">
              <g class="process-area-physical-layer">
                <rect class="area-facility-shell" x="25" y="100" width="1230" height="720"></rect>
                <rect class="area-control-room" x="45" y="125" width="485" height="650"></rect>
                <rect class="area-field-room" x="555" y="125" width="680" height="650"></rect>
                <rect class="area-compute-bay" x="55" y="135" width="220" height="180"></rect>
                <rect class="area-operator-bay" x="290" y="135" width="235" height="180"></rect>
                <rect class="area-cabinet-row" x="55" y="340" width="470" height="245"></rect>
                <rect class="area-safety-bay" x="900" y="670" width="300" height="125"></rect>
                <path class="area-cable-tray" d="M165 270 V355 H410 V452 H695 V165 H1150"></path>
                <path class="area-material-line" d="M595 485 H1165"></path>
                <path class="area-floor-marking" d="M575 160 H1210 V650 H575 Z M850 160 V650"></path>
                <text class="area-zone-label" x="70" y="150">LEVEL 3 CONTROL COMPUTE</text>
                <text class="area-zone-label" x="305" y="150">LOCAL OPERATOR STATION</text>
                <text class="area-zone-label" x="70" y="365">CELL NETWORK AND DISTRIBUTED I/O</text>
                <text class="area-zone-label" x="585" y="155">FIELD EQUIPMENT BAY</text>
                <text class="area-zone-label" x="925" y="700">SAFETY / INTERLOCK BOUNDARY</text>
                <text class="area-detail-label" x="640" y="475">PROCESS MATERIAL PATH</text>
              </g>

              <g class="process-area-logical-layer">
                <rect class="area-logical-zone area-network-zone" x="35" y="315" width="245" height="270"></rect>
                <rect class="area-logical-zone area-control-zone" x="295" y="90" width="260" height="700"></rect>
                <rect class="area-logical-zone area-input-zone" x="575" y="90" width="280" height="700"></rect>
                <rect class="area-logical-zone area-output-zone" x="875" y="90" width="350" height="550"></rect>
                <rect class="area-logical-zone area-safety-zone" x="875" y="655" width="350" height="160"></rect>
                <text class="area-zone-label" x="55" y="345">CELL NETWORK</text>
                <text class="area-zone-label" x="315" y="120">CONTROL AND OPERATOR LAYER</text>
                <text class="area-zone-label" x="595" y="120">PROCESS INPUTS</text>
                <text class="area-zone-label" x="895" y="120">PROCESS OUTPUTS</text>
                <text class="area-zone-label" x="895" y="685">SAFETY AND PERMISSIVES</text>
              </g>

              <g class="process-area-connections">
                {#each equipment.filter((item) => upstreamFor(item)) as item (item.id)}
                  <path
                    class:process-io-link={item.linkKind === "io"}
                    class:process-safety-link={item.linkKind === "safety-status"}
                    d={connectionPath(item)}
                  ></path>
                {/each}
              </g>
            </svg>

            {#each equipment as item (item.id)}
              {@const presentation = equipmentPresentation(item)}
              {@const Icon = processIconByKey[presentation.icon]}
              <button
                type="button"
                class:selected={selectedId === item.id}
                class:physical-device-marker={viewMode === "physical"}
                class="lan-device process-area-device"
                style={`left: ${item.x}px; top: ${item.y}px; --node-accent: ${item.accent};`}
                aria-label={`Inspect ${presentation.label}, ${presentation.role}`}
                title={`Inspect ${presentation.label}`}
                onclick={() => (selectedId = item.id)}
              >
                {#if viewMode === "physical"}
                  <PhysicalDeviceMarker icon={Icon} label={presentation.label} />
                {:else}
                  <span class="node-accent"></span>
                  <span class="lan-device-header">
                    <span class="node-icon"><Icon size={19} strokeWidth={1.8} /></span>
                    <small>{presentation.kind}</small>
                  </span>
                  <strong>{presentation.label}</strong>
                  <span>{presentation.role}</span>
                {/if}
              </button>
            {/each}

            <div class="lan-key" aria-label={`${area.label} legend`}>
              {#if viewMode === "physical"}
                <span><Cable size={13} strokeWidth={1.8} /><i class="cable-key copper"></i>Industrial Ethernet</span>
                <span><i class="cable-key process-io"></i>Field I/O</span>
                <span><ShieldCheck size={13} strokeWidth={1.8} />Safety/status interface</span>
              {:else}
                <span><i class="cable-key copper"></i>Network relationship</span>
                <span><i class="cable-key process-io"></i>I/O binding</span>
                <span><ShieldCheck size={13} strokeWidth={1.8} />Safety/status interface</span>
              {/if}
            </div>
          </section>
        </div>
      </div>
    {:else}
      <div class="process-area-missing">
        <strong>Unknown process area</strong>
        <button type="button" onclick={onBack}>Return to ceramics process</button>
      </div>
    {/if}

    {#if selectedEquipment && selectedPresentation}
      <aside class="lan-inspector" aria-label="Selected process component">
        <div class="lan-inspector-header">
          <div>
            <span>{selectedPresentation.kind}</span>
            <h2>{selectedPresentation.label}</h2>
          </div>
          <button type="button" aria-label="Close inspector" title="Close inspector" onclick={() => (selectedId = null)}>
            <X size={17} strokeWidth={1.9} />
          </button>
        </div>
        <p>{selectedPresentation.role}</p>
        <dl>
          <div>
            <dt>Area</dt>
            <dd>{area?.zone}</dd>
          </div>
          <div>
            <dt>Config source</dt>
            <dd>{selectedPresentation.configRef}</dd>
          </div>
        </dl>
        <ul class="process-equipment-facts">
          {#each selectedPresentation.facts as fact}
            <li>{fact}</li>
          {/each}
        </ul>
      </aside>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>{area?.zone ?? "Unknown area"}</span>
    <span>{viewMode === "physical" ? "Physical equipment" : "Logical control relationships"} / {equipment.length} components</span>
    <span>{Math.round(zoom * 100)}% / schema {SUPPORTED_PROCESS_VIEW_SCHEMA}</span>
  </footer>
</div>

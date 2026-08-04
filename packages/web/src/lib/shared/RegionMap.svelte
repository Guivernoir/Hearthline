<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Component } from "svelte";
  import {
    ArrowRight,
    Building2,
    Factory,
    FlaskConical,
    Globe2,
    Grid2X2,
    House,
    Map,
    Maximize2,
    Minus,
    Network,
    Plus,
    RotateCcw,
    ServerCog,
    ShieldCheck,
    Wifi,
    X,
  } from "@lucide/svelte";
  import { processView } from "../process/process-model";
  import type { PlaceId, ViewMode } from "./types";

  interface Place {
    id: PlaceId;
    label: string;
    district: string;
    summary: string;
    x: number;
    y: number;
    accent: string;
    icon: Component<any>;
    environments: string[];
  }

  export let onEnter: (place: PlaceId) => void = () => {};
  export let onOpenSimulations: () => void = () => {};
  export let viewMode: ViewMode = "logical";

  const WORLD_WIDTH = 1400;
  const WORLD_HEIGHT = 820;
  const MIN_ZOOM = 0.35;
  const MAX_ZOOM = 1.6;
  const ZOOM_STEP = 0.1;

  const places: Place[] = [
    {
      id: "customer",
      label: "Customer Network",
      district: "Residential district",
      summary: "External users reach Hearthline's public services from an independently managed customer LAN.",
      x: 215,
      y: 515,
      accent: "#3567a6",
      icon: House,
      environments: ["Customer LAN", "Customer edge", "Public web path"],
    },
    {
      id: "office",
      label: "Central Office",
      district: "Operations campus",
      summary: "Enterprise services, public-service isolation, and the controlled handoff toward OT.",
      x: 635,
      y: 220,
      accent: "#267168",
      icon: Building2,
      environments: ["Business IT", "IT DMZ", "Operations Intelligence"],
    },
    {
      id: "factory",
      label: "Factory",
      district: "Industrial corridor",
      summary: "Segmented OT operations and the ceramics production process controlled by virtual PLCs.",
      x: 975,
      y: 510,
      accent: "#b65034",
      icon: Factory,
      environments: ["OT operations", "vPLC platform", `${processView.areas.length} process areas`],
    },
  ];

  let viewport: HTMLDivElement;
  let zoom = 0.8;
  let gridVisible = false;
  let selectedId: PlaceId | null = null;
  let dragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragScrollLeft = 0;
  let dragScrollTop = 0;
  let viewportWidth = 1;
  let viewportHeight = 1;

  $: selectedPlace = places.find((place) => place.id === selectedId) ?? null;
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

    const horizontalPadding = viewport.clientWidth < 720 ? 24 : 80;
    const verticalPadding = viewport.clientHeight < 620 ? 24 : 64;
    zoom = Number(
      clampZoom(
        Math.min(
          (viewport.clientWidth - horizontalPadding) / WORLD_WIDTH,
          (viewport.clientHeight - verticalPadding) / WORLD_HEIGHT,
        ),
      ).toFixed(2),
    );
    await tick();
    if (!viewport) return;
    viewport.scrollLeft = Math.max(0, (WORLD_WIDTH * zoom - viewport.clientWidth) / 2);
    viewport.scrollTop = Math.max(0, (WORLD_HEIGHT * zoom - viewport.clientHeight) / 2);
  }

  async function resetView() {
    zoom = 1;
    await tick();
    if (!viewport) return;
    viewport.scrollLeft = 0;
    viewport.scrollTop = 0;
  }

  function selectPlace(event: MouseEvent, place: Place) {
    event.stopPropagation();
    selectedId = place.id;
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
    if (target.closest(".place-marker")) return;

    if (event.button === 0 || event.button === 1) {
      if (event.button === 0 && !target.closest(".map-control")) {
        selectedId = null;
      }
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

  function mapInteractions(node: HTMLDivElement) {
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
  <title>Hearthline Regional Map</title>
</svelte:head>

<div class="app-shell">
  <header class="topbar">
    <div class="brand-block">
      <span class="brand-mark" aria-hidden="true"><Network size={20} strokeWidth={1.8} /></span>
      <div class="brand-copy">
        <strong>Hearthline</strong>
        <span>Architecture</span>
      </div>
    </div>

    <div class="view-context" aria-label="Current view">
      <span>Map</span>
      <strong>Regional architecture</strong>
    </div>

    <div class="toolbar" aria-label="Map tools">
      <button type="button" aria-label="Open simulations" title="Simulations" onclick={onOpenSimulations}>
        <FlaskConical size={17} strokeWidth={1.9} />
      </button>
      <span class="toolbar-divider"></span>
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
          onclick={resetView}
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

      <button type="button" aria-label="Fit map" title="Fit map" onclick={fitToView}>
        <Maximize2 size={17} strokeWidth={1.9} />
      </button>
      <button type="button" aria-label="Reset map" title="Reset map" onclick={resetView}>
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

  <main class="workspace map-workspace">
    <div
      class:is-dragging={dragging}
      class="viewport map-viewport"
      bind:this={viewport}
      use:mapInteractions
      role="region"
      aria-label="Hearthline regional map"
    >
      <div
        class="canvas-size"
        style={`width: ${Math.max(worldPixelWidth, viewportWidth)}px; height: ${Math.max(worldPixelHeight, viewportHeight)}px;`}
      >
        <section
          class:grid-visible={gridVisible}
          class:physical-view={viewMode === "physical"}
          class:logical-view={viewMode === "logical"}
          class="region-world"
          style={`left: ${worldOffsetX}px; top: ${worldOffsetY}px; width: ${WORLD_WIDTH}px; height: ${WORLD_HEIGHT}px; transform: scale(${zoom});`}
          aria-label="Regional sites"
        >
          <div class="map-heading">
            <span>HEARTHLINE / REGION</span>
            <h1>Operational landscape</h1>
            <p>{viewMode === "physical" ? "Sites and transport corridor" : "External, enterprise, and industrial trust zones"}</p>
          </div>

          <svg class="map-drawing" viewBox={`0 0 ${WORLD_WIDTH} ${WORLD_HEIGHT}`} aria-hidden="true">
            <path class="region-border" d="M60 165 L365 112 L605 150 L868 102 L1328 160 L1340 694 L1102 756 L826 704 L568 762 L78 698 Z"></path>

            <g class="map-parcels">
              <path d="M92 450 L398 392 L480 660 L116 686 Z"></path>
              <path d="M510 175 L825 145 L870 410 L530 430 Z"></path>
              <path d="M932 402 L1318 356 L1314 690 L944 714 Z"></path>
            </g>

            <g class="physical-map-layer">
              <path class="road road-major" d="M86 448 C310 410 492 444 650 482 C832 526 1050 430 1318 446"></path>
              <path class="road" d="M215 544 C180 515 160 478 160 435"></path>
              <path class="road" d="M650 482 C678 418 678 350 664 286"></path>
              <path class="road" d="M975 544 C940 516 930 486 950 463"></path>
              <path class="rail" d="M90 650 C390 612 790 682 1315 572"></path>
            </g>

            <g class="logical-map-layer">
              <defs>
                <marker id="map-arrow-public" markerWidth="9" markerHeight="9" refX="8" refY="4.5" orient="auto">
                  <path class="public-arrow" d="M0,0 L9,4.5 L0,9 Z"></path>
                </marker>
                <marker id="map-arrow-ot" markerWidth="9" markerHeight="9" refX="8" refY="4.5" orient="auto">
                  <path class="ot-arrow" d="M0,0 L9,4.5 L0,9 Z"></path>
                </marker>
              </defs>
              <path class="logical-link public-link" d="M273 544 C350 510 410 445 476 392" marker-end="url(#map-arrow-public)"></path>
              <path class="logical-link public-link" d="M510 373 C555 344 610 310 650 280" marker-end="url(#map-arrow-public)"></path>
              <path class="logical-link ot-link" d="M664 286 C686 326 808 349 1008 526" marker-end="url(#map-arrow-ot)"></path>
              <text x="327" y="463">HTTPS / PUBLIC SERVICE</text>
              <text x="805" y="374">BROKERED OT CONDUIT</text>
            </g>
          </svg>

          <div class="district-label customer-district">
            <span>RESIDENTIAL DISTRICT</span>
            <strong>External users</strong>
          </div>
          <div class="district-label office-district">
            <span>OPERATIONS CAMPUS</span>
            <strong>Central services</strong>
          </div>
          <div class="district-label factory-district">
            <span>INDUSTRIAL CORRIDOR</span>
            <strong>Production site</strong>
          </div>

          {#if viewMode === "logical"}
            <div class="internet-hub" aria-label="Public Internet">
              <span><Globe2 size={21} strokeWidth={1.8} /></span>
              <strong>Public Internet</strong>
            </div>
          {/if}

          {#each places as place (place.id)}
            {@const Icon = place.icon}
            <button
              type="button"
              class:selected={selectedId === place.id}
              class="place-marker"
              style={`left: ${place.x}px; top: ${place.y}px; --place-accent: ${place.accent};`}
              aria-label={`Select ${place.label}`}
              onclick={(event) => selectPlace(event, place)}
              ondblclick={() => onEnter(place.id)}
            >
              <span class="place-pin"><Icon size={25} strokeWidth={1.8} /></span>
              <span class="place-label">
                <strong>{place.label}</strong>
                <small>{place.district}</small>
              </span>
            </button>
          {/each}

          <div class="map-key" aria-label="Map legend">
            {#if viewMode === "logical"}
              <span><i class="key-line public"></i>Public route</span>
              <span><i class="key-line controlled"></i>Controlled conduit</span>
            {:else}
              <span><i class="key-line road-key"></i>Road</span>
              <span><i class="key-line rail-key"></i>Rail corridor</span>
            {/if}
          </div>
        </section>
      </div>
    </div>

    {#if selectedPlace}
      <aside class="place-inspector" aria-label="Selected location">
        <div class="place-inspector-header">
          <span class="place-inspector-icon" style={`--place-accent: ${selectedPlace.accent};`}>
            <svelte:component this={selectedPlace.icon} size={21} strokeWidth={1.8} />
          </span>
          <div>
            <span>{selectedPlace.district}</span>
            <h2>{selectedPlace.label}</h2>
          </div>
          <button
            type="button"
            aria-label="Close location details"
            title="Close"
            onclick={() => (selectedId = null)}
          >
            <X size={17} strokeWidth={1.9} />
          </button>
        </div>
        <p>{selectedPlace.summary}</p>
        <div class="environment-list">
          <span>Environments</span>
          {#each selectedPlace.environments as environment}
            <div>
              {#if selectedPlace.id === "customer"}
                <Wifi size={15} strokeWidth={1.8} />
              {:else if environment.includes("DMZ")}
                <ShieldCheck size={15} strokeWidth={1.8} />
              {:else}
                <ServerCog size={15} strokeWidth={1.8} />
              {/if}
              <strong>{environment}</strong>
            </div>
          {/each}
        </div>
        <button type="button" class="enter-place" onclick={() => onEnter(selectedPlace.id)}>
          Enter location
          <ArrowRight size={17} strokeWidth={1.9} />
        </button>
      </aside>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>Architecture model</span>
    <span>3 locations / 1 regional boundary</span>
    <span>{viewMode === "physical" ? "Physical" : "Logical"} / {Math.round(zoom * 100)}%</span>
  </footer>
</div>

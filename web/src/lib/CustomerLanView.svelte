<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { Component } from "svelte";
  import {
    ArrowLeft,
    Cable,
    ChevronDown,
    Grid2X2,
    House,
    Map,
    Maximize2,
    Minus,
    Monitor,
    Network,
    Plus,
    RotateCcw,
    Router,
    Wifi,
    X,
  } from "@lucide/svelte";
  import PhysicalDeviceMarker from "./PhysicalDeviceMarker.svelte";
  import type { ViewMode } from "./types";

  interface DevicePosition {
    x: number;
    y: number;
  }

  interface CustomerDevice {
    id: string;
    label: string;
    role: string;
    area: string;
    address: string;
    ports: string[];
    accent: string;
    icon: Component<any>;
    physical: DevicePosition;
    logical: DevicePosition;
  }

  export let onBack: () => void = () => {};
  export let viewMode: ViewMode = "logical";

  const WORLD_WIDTH = 1250;
  const WORLD_HEIGHT = 820;
  const MIN_ZOOM = 0.25;
  const MAX_ZOOM = 1.6;
  const ZOOM_STEP = 0.1;

  const devices: CustomerDevice[] = [
    {
      id: "pc-01",
      label: "Customer PC-01",
      role: "Customer workstation",
      area: "Home office",
      address: "192.168.0.2/24",
      ports: ["FastEthernet0 → SW-01 Fa0/1", "Gateway 192.168.0.1"],
      accent: "#3567a6",
      icon: Monitor,
      physical: { x: 145, y: 270 },
      logical: { x: 80, y: 240 },
    },
    {
      id: "pc-02",
      label: "Customer PC-02",
      role: "Customer workstation",
      area: "Living area",
      address: "192.168.0.3/24",
      ports: ["FastEthernet0 → SW-01 Fa0/2", "Gateway 192.168.0.1"],
      accent: "#3567a6",
      icon: Monitor,
      physical: { x: 145, y: 510 },
      logical: { x: 80, y: 490 },
    },
    {
      id: "sw-01",
      label: "Customer SW-01",
      role: "Layer 2 access switch",
      area: "Network cabinet",
      address: "VLAN 1 / Layer 2",
      ports: ["Fa0/1 PC-01", "Fa0/2 PC-02", "Gi0/1 RTR-01"],
      accent: "#267168",
      icon: Network,
      physical: { x: 500, y: 380 },
      logical: { x: 400, y: 365 },
    },
    {
      id: "rtr-01",
      label: "Customer RTR-01",
      role: "Default gateway and PAT edge",
      area: "Network cabinet",
      address: "192.168.0.1/24 · 203.0.113.2/24",
      ports: ["Gi0/0 SW-01 Gi0/1", "Gi0/1 INET-CPE-01 customer port"],
      accent: "#267168",
      icon: Router,
      physical: { x: 850, y: 380 },
      logical: { x: 780, y: 365 },
    },
  ];

  let viewport: HTMLDivElement;
  let zoom = 0.8;
  let gridVisible = false;
  let selectedId: string | null = null;
  let dragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragScrollLeft = 0;
  let dragScrollTop = 0;
  let viewportWidth = 1;
  let viewportHeight = 1;

  $: selectedDevice = devices.find((device) => device.id === selectedId) ?? null;
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

  function selectDevice(event: MouseEvent, device: CustomerDevice) {
    event.stopPropagation();
    selectedId = device.id;
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
  <title>Customer LAN | Hearthline</title>
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
      <strong>Customer LAN</strong>
    </div>

    <div class="toolbar" aria-label="Customer LAN tools">
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
      aria-label="Customer LAN architecture"
    >
      <div
        class="canvas-size"
        style={`width: ${Math.max(worldPixelWidth, viewportWidth)}px; height: ${Math.max(worldPixelHeight, viewportHeight)}px;`}
      >
        <section
          class:grid-visible={gridVisible}
          class:physical-view={viewMode === "physical"}
          class:logical-view={viewMode === "logical"}
          class="customer-lan-world"
          style={`left: ${worldOffsetX}px; top: ${worldOffsetY}px; width: ${WORLD_WIDTH}px; height: ${WORLD_HEIGHT}px; transform: scale(${zoom});`}
          aria-label={`${viewMode} Customer LAN view`}
        >
          <div class="lan-heading">
            <span>HEARTHLINE / CUSTOMER / LAN</span>
            <h1>Customer premises network</h1>
            <p>{viewMode === "physical" ? "Residential placement and physical cabling" : "Layer 2, routing, addressing, and ISP handoff"}</p>
          </div>

          <svg class="lan-drawing" viewBox={`0 0 ${WORLD_WIDTH} ${WORLD_HEIGHT}`} aria-hidden="true">
            <g class="lan-physical-layer">
              <path class="house-shell" d="M60 230 L625 72 L1190 230 V700 H60 Z"></path>
              <path class="house-roof-line" d="M60 230 H1190"></path>
              <path class="house-wall" d="M450 230 V700 M800 230 V700 M60 470 H450"></path>
              <text class="room-label" x="90" y="260">HOME OFFICE</text>
              <text class="room-label" x="90" y="500">LIVING AREA</text>
              <text class="room-label" x="480" y="260">LAN CABINET</text>
              <text class="room-label" x="830" y="260">CUSTOMER EDGE HANDOFF</text>
              <text class="premises-label" x="625" y="138">CUSTOMER PREMISES</text>

              <g class="physical-cables">
                <path d="M240 322 H420 V432 H595"></path>
                <path d="M240 562 H420 V432 H595"></path>
                <path d="M595 432 H945"></path>
                <path class="dsl-cable" d="M945 432 H1180"></path>
              </g>
              <text class="room-label" x="1050" y="418">TO CUSTOMER EDGE</text>
            </g>

            <g class="lan-logical-layer">
              <defs>
                <marker id="lan-arrow" markerWidth="9" markerHeight="9" refX="8" refY="4.5" orient="auto">
                  <path d="M0,0 L9,4.5 L0,9 Z"></path>
                </marker>
              </defs>
              <rect class="logical-zone private-zone" x="45" y="175" width="925" height="495"></rect>
              <rect class="logical-zone provider-zone" x="995" y="175" width="220" height="495"></rect>
              <text class="zone-label" x="80" y="210">PRIVATE CUSTOMER LAN · 192.168.0.0/24</text>
              <text class="zone-label" x="1020" y="210">CUSTOMER EDGE</text>

              <g class="logical-cables">
                <path d="M175 292 H340 V417 H495"></path>
                <path d="M175 542 H340 V417 H495"></path>
                <path d="M495 417 H875" marker-end="url(#lan-arrow)"></path>
                <path class="dsl-cable" d="M875 417 H1110" marker-end="url(#lan-arrow)"></path>
              </g>

              <text class="port-label" x="188" y="282">Fa0</text>
              <text class="port-label" x="188" y="558">Fa0</text>
              <text class="port-label" x="620" y="405">SW Gi0/1</text>
              <text class="port-label" x="725" y="405">RTR Gi0/0</text>
              <text class="port-label" x="930" y="405">RTR Gi0/1</text>

              <g class="nat-boundary">
                <path d="M982 245 V620"></path>
                <text x="982" y="640">ROUTED HANDOFF TO CUSTOMER EDGE</text>
              </g>
            </g>
          </svg>

          {#each devices as device (device.id)}
            {@const Icon = device.icon}
            {@const position = viewMode === "physical" ? device.physical : device.logical}
            <button
              type="button"
              class:selected={selectedId === device.id}
              class:physical-device-marker={viewMode === "physical"}
              class="lan-device"
              style={`left: ${position.x}px; top: ${position.y}px; --node-accent: ${device.accent};`}
              aria-label={`Inspect ${device.label}, ${device.area}`}
              title={`Inspect ${device.label}`}
              onclick={(event) => selectDevice(event, device)}
            >
              {#if viewMode === "physical"}
                <PhysicalDeviceMarker icon={Icon} label={device.label} />
              {:else}
                <span class="node-accent"></span>
                <span class="lan-device-header">
                  <span class="node-icon"><Icon size={19} strokeWidth={1.8} /></span>
                  <small>{device.area}</small>
                </span>
                <strong>{device.label}</strong>
                <span>{device.address}</span>
              {/if}
            </button>
          {/each}

          <div class="lan-key" aria-label="Customer LAN legend">
            {#if viewMode === "physical"}
              <span><i class="cable-key copper"></i><Cable size={13} strokeWidth={1.8} />Copper Ethernet</span>
              <span><i class="cable-key dsl"></i>Provider handoff</span>
            {:else}
              <span><i class="zone-key private"></i>Private LAN</span>
              <span><i class="zone-key provider"></i>ISP-facing</span>
            {/if}
          </div>
        </section>
      </div>
    </div>

    {#if selectedDevice}
      <aside class="lan-inspector" aria-label="Selected customer device">
        <div class="lan-inspector-header">
          <div>
            <span>{selectedDevice.area}</span>
            <h2>{selectedDevice.label}</h2>
          </div>
          <button
            type="button"
            aria-label="Close device details"
            title="Close"
            onclick={() => (selectedId = null)}
          >
            <X size={17} strokeWidth={1.9} />
          </button>
        </div>
        <p>{selectedDevice.role}</p>
        <dl>
          <div>
            <dt>Addressing</dt>
            <dd>{selectedDevice.address}</dd>
          </div>
          <div>
            <dt>Placement</dt>
            <dd>{selectedDevice.area}</dd>
          </div>
        </dl>
        <div class="lan-port-list">
          <span>Interfaces and paths</span>
          {#each selectedDevice.ports as port}
            <div><Wifi size={14} strokeWidth={1.8} />{port}</div>
          {/each}
        </div>
      </aside>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>Customer LAN model</span>
    <span>{devices.length} nodes / 3 documented links</span>
    <span>{viewMode === "physical" ? "Physical" : "Logical"} / {Math.round(zoom * 100)}%</span>
  </footer>
</div>

<script lang="ts">
  import {
    ArrowLeft,
    ChevronDown,
    Grid2X2,
    Hand,
    Map,
    Maximize2,
    Minus,
    MousePointer2,
    Network,
    Plus,
    RotateCcw,
  } from "@lucide/svelte";

  type Mode = "select" | "pan";
  type ViewMode = "physical" | "logical";

  export let onBack: () => void;
  export let viewMode: ViewMode;
  export let mode: Mode;
  export let zoom: number;
  export let minZoom: number;
  export let maxZoom: number;
  export let zoomStep: number;
  export let gridVisible: boolean;
  export let onZoom: (zoom: number) => void | Promise<void>;
  export let onFit: () => void | Promise<void>;
  export let onReset: () => void | Promise<void>;
</script>

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
      <button type="button" aria-label="Zoom out" title="Zoom out" disabled={zoom <= minZoom} onclick={() => onZoom(zoom - zoomStep)}>
        <Minus size={17} strokeWidth={1.9} />
      </button>
      <button type="button" class="zoom-value" aria-label="Reset zoom to 100 percent" title="Reset zoom to 100%" onclick={onReset}>
        {Math.round(zoom * 100)}%
      </button>
      <button type="button" aria-label="Zoom in" title="Zoom in" disabled={zoom >= maxZoom} onclick={() => onZoom(zoom + zoomStep)}>
        <Plus size={17} strokeWidth={1.9} />
      </button>
    </div>

    <button type="button" aria-label="Fit to view" title="Fit to view" onclick={onFit}>
      <Maximize2 size={17} strokeWidth={1.9} />
    </button>
    <button type="button" aria-label="Reset view" title="Reset view" onclick={onReset}>
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

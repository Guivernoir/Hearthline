<script lang="ts">
  import { onMount } from "svelte";
  import {
    Check,
    Clipboard,
    Code2,
    FileCode2,
    LoaderCircle,
    Settings2,
    X,
  } from "@lucide/svelte";
  import {
    loadHmiControlProgram,
    type HmiControlProgramDocument,
    type HmiControlProgramState,
  } from "./hmi-api";

  export let applianceId: string;
  export let runtime: HmiControlProgramState;
  export let onClose: () => void = () => {};

  let document: HmiControlProgramDocument | null = null;
  let active: "source" | "binding" = "source";
  let copied = false;
  let error = "";

  $: displayedSource = active === "source" ? document?.source ?? "" : document?.bindingYaml ?? "";
  $: displayedPath = active === "source" ? runtime.sourcePath : runtime.bindingPath;

  onMount(async () => {
    try {
      document = await loadHmiControlProgram(applianceId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Cannot load control source";
    }
  });

  async function copySource() {
    if (!displayedSource) return;
    try {
      await navigator.clipboard.writeText(displayedSource);
      copied = true;
      window.setTimeout(() => (copied = false), 1_600);
    } catch {
      copied = false;
    }
  }
</script>

<div class="control-program-overlay" role="presentation" onclick={(event) => event.currentTarget === event.target && onClose()}>
  <div class="control-program-panel" role="dialog" aria-modal="true" aria-label="Controller source">
    <header>
      <div><span><Code2 size={19} /></span><strong>{runtime.program}</strong><small>{runtime.language}</small></div>
      <button type="button" aria-label="Close control source" title="Close" onclick={onClose}><X size={18} /></button>
    </header>

    <dl class="control-program-runtime">
      <div><dt>Controller</dt><dd>{document?.controller ?? applianceId}</dd></div>
      <div><dt>Task</dt><dd>{runtime.task}</dd></div>
      <div><dt>Current step</dt><dd>{runtime.currentStep}</dd></div>
      <div><dt>Scan</dt><dd>{runtime.scanIntervalMs} ms</dd></div>
      <div><dt>Watchdog</dt><dd>{runtime.watchdogMs} ms</dd></div>
      <div><dt>Revision</dt><dd title={runtime.revision}>{runtime.revision.slice(0, 12)}</dd></div>
    </dl>

    <div class="control-program-toolbar">
      <div role="tablist" aria-label="Control document">
        <button class:active={active === "source"} type="button" role="tab" aria-selected={active === "source"} onclick={() => (active = "source")}><FileCode2 size={15} />Structured Text</button>
        <button class:active={active === "binding"} type="button" role="tab" aria-selected={active === "binding"} onclick={() => (active = "binding")}><Settings2 size={15} />I/O binding</button>
      </div>
      <button type="button" aria-label="Copy displayed control document" title="Copy document" disabled={!displayedSource} onclick={() => void copySource()}>
        {#if copied}<Check size={16} />{:else}<Clipboard size={16} />{/if}
      </button>
    </div>

    <div class="control-program-path">{displayedPath}</div>
    {#if error}
      <div class="control-program-error">{error}</div>
    {:else if !document}
      <div class="control-program-loading"><LoaderCircle class="spin" size={22} />Loading validated source</div>
    {:else}
      <pre><code>{displayedSource}</code></pre>
    {/if}
  </div>
</div>

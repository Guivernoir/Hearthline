<script lang="ts">
  import { afterUpdate } from "svelte";
  import { FileUp, LoaderCircle, Pause, Play, RotateCcw, Save, StepForward } from "@lucide/svelte";
  import type { HmiAction, HmiControlStation, HmiRobotState } from "../hmi-api";

  export let robot: HmiRobotState;
  export let station: HmiControlStation;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  let source = robot.program.source;
  let programName = robot.program.name;
  let fileInput: HTMLInputElement;
  let activeRow: HTMLElement | null = null;
  let lastActiveLine: number | null = null;

  $: setupAuthority = station.selectedMode === "setup" && station.setupAuthenticated;
  $: canExecute = setupAuthority && robot.motionEnabled && !busyTarget;

  afterUpdate(() => {
    if (robot.program.activeLine !== lastActiveLine) {
      lastActiveLine = robot.program.activeLine;
      const container = activeRow?.parentElement;
      if (activeRow && container) {
        container.scrollTop = Math.max(0, activeRow.offsetTop - container.clientHeight / 2);
      }
    }
  });

  async function loadFile(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    source = await file.text();
    programName = file.name.replace(/\.g$/i, "");
  }

  function trackActive(node: HTMLElement, active: boolean) {
    if (active) activeRow = node;
    return {
      update(next: boolean) {
        if (next) activeRow = node;
      },
    };
  }
</script>

<div class="robot-program-workspace">
  <section class="robot-program-browser">
    <header>
      <span><strong>{robot.program.name}</strong><small>{robot.program.sourcePath}</small></span>
      <div><i class:running={robot.program.running}></i><strong>{robot.program.running ? "running" : robot.program.paused ? "paused" : "ready"}</strong></div>
    </header>
    <div class="robot-program-toolbar">
      <button type="button" title="Run program" aria-label="Run robot program" disabled={!canExecute || robot.program.running} onclick={() => onExecute({ kind: "run-robot-program" }, "robot-program-run")}>{#if busyTarget === "robot-program-run"}<LoaderCircle class="spin" size={14} />{:else}<Play size={14} />{/if}</button>
      <button type="button" title="Pause program" aria-label="Pause robot program" disabled={!robot.program.running || Boolean(busyTarget)} onclick={() => onExecute({ kind: "pause-robot-program" }, "robot-program-pause")}><Pause size={14} /></button>
      <button type="button" title="Execute next line" aria-label="Step robot program" disabled={!canExecute || robot.program.running} onclick={() => onExecute({ kind: "step-robot-program" }, "robot-program-step")}><StepForward size={14} /></button>
      <button type="button" title="Reset program" aria-label="Reset robot program" disabled={Boolean(busyTarget)} onclick={() => onExecute({ kind: "reset-robot-program" }, "robot-program-reset")}><RotateCcw size={14} /></button>
      <span>Line {robot.program.activeLine ?? "-"} / cycle {robot.program.cycleCount}</span>
    </div>
    <ol class="robot-program-lines" aria-label="Robot program source">
      {#each robot.program.lines as line}
        <li use:trackActive={line.active} class:active={line.active} class:executable={line.operation}>
          <span>{line.number}</span><code>{line.source || " "}</code><small>{line.operation ?? ""}</small>
        </li>
      {/each}
    </ol>
  </section>

  <section class="robot-program-editor">
    <header><span><Save size={15} />Program source</span><button type="button" title="Open G program" aria-label="Open G program file" disabled={!setupAuthority} onclick={() => fileInput.click()}><FileUp size={15} /></button></header>
    <input bind:this={fileInput} class="visually-hidden" type="file" accept=".g,text/plain" onchange={loadFile} />
    <label><span>Program name</span><input bind:value={programName} disabled={!setupAuthority} /></label>
    <textarea bind:value={source} spellcheck="false" disabled={!setupAuthority} aria-label="Robot G program source"></textarea>
    <button class="robot-primary-command" type="button" disabled={!setupAuthority || Boolean(busyTarget)} onclick={() => onExecute({ kind: "load-robot-program", name: programName, source }, "robot-program-load")}>{#if busyTarget === "robot-program-load"}<LoaderCircle class="spin" size={14} />{:else}<Save size={14} />{/if}Parse and load</button>
  </section>
</div>

<script lang="ts">
  import { LoaderCircle, Pause, Play } from "@lucide/svelte";
  import type {
    HmiAction,
    HmiPreparationTrainState,
  } from "../../hmi-api";

  export let train: HmiPreparationTrainState;
  export let compact = false;
  export let safetyTripped = false;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  $: phaseIndex = train.phases.findIndex((phase) => phase.key === train.phase);
  $: startTarget = `start-${train.id}`;
  $: holdTarget = `hold-${train.id}`;

  function label(value: string) {
    return value.replaceAll("-", " ");
  }
</script>

<section class:compact class="body-train-control">
  <header>
    <div><i class:running={train.running} class:held={train.held}></i><span><small>{train.label}</small><strong>{train.held ? "Held" : label(train.phase)}</strong></span></div>
    <dl>
      <div><dt>Phase</dt><dd>{train.phaseElapsedProcessMinutes.toFixed(0)} / {train.phaseTargetProcessMinutes.toFixed(0)} min</dd></div>
      <div><dt>Cycles</dt><dd>{train.cycleCount}</dd></div>
    </dl>
    <div class="body-train-actions">
      <button
        type="button"
        title={train.held ? "Resume train" : "Start train"}
        disabled={train.running || train.phase === "faulted" || safetyTripped || Boolean(busyTarget)}
        onclick={() => onExecute({ kind: "start-preparation-train", train: train.id }, startTarget)}
      >{#if busyTarget === startTarget}<LoaderCircle class="spin" size={14} />{:else}<Play size={14} />{/if}<span>{train.held ? "Resume" : "Start"}</span></button>
      <button
        type="button"
        title="Hold train"
        disabled={!train.running || Boolean(busyTarget)}
        onclick={() => onExecute({ kind: "hold-preparation-train", train: train.id }, holdTarget)}
      >{#if busyTarget === holdTarget}<LoaderCircle class="spin" size={14} />{:else}<Pause size={14} />{/if}<span>Hold</span></button>
    </div>
  </header>
  <div class="body-train-progress"><span style={`width: ${train.phaseProgressPercent}%`}></span></div>
  {#if !compact}
    <ol>
      {#each train.phases as phase, index}
        <li class:active={phase.key === train.phase} class:complete={train.running && index < phaseIndex}><i>{index + 1}</i><span>{phase.label}</span></li>
      {/each}
    </ol>
  {/if}
</section>

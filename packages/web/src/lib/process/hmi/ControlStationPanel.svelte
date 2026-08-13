<script lang="ts">
  import { AlertTriangle, KeyRound, LockKeyhole, ShieldCheck } from "@lucide/svelte";
  import type { HmiControlMode, HmiControlStation } from "./hmi-api";

  export let station: HmiControlStation;
  export let busy = false;
  export let onSelect: (mode: HmiControlMode, password?: string) => void = () => {};

  let password = "";

  function select(mode: HmiControlMode) {
    if (mode === "setup") {
      onSelect(mode, password);
      password = "";
      return;
    }
    onSelect(mode);
  }
</script>

{#if station.positions.length > 0}
  <section class:setup-active={station.sensorBypassActive} class="hmi-mode-panel" aria-label="Keyed control selector">
    <header>
      <span><KeyRound size={16} />Keyed selector</span>
      <strong>{station.target}</strong>
    </header>
    <div class="hmi-mode-selector" role="group" aria-label="Control mode">
      {#each station.positions as mode}
        <button
          type="button"
          class:active={station.selectedMode === mode}
          class:setup={mode === "setup"}
          aria-pressed={station.selectedMode === mode}
          disabled={busy || mode === "setup"}
          onclick={() => select(mode)}
        >
          <i></i><span>{mode}</span>
        </button>
      {/each}
    </div>
    <div class="hmi-setup-auth">
      <label>
        <span><LockKeyhole size={14} />Setup credential</span>
        <input bind:value={password} type="password" autocomplete="current-password" disabled={busy} />
      </label>
      <button type="button" disabled={busy || password.length === 0} onclick={() => select("setup")}>Unlock setup</button>
    </div>
    <div class="hmi-protection-state">
      {#if station.sensorBypassActive}
        <span class="bypass"><AlertTriangle size={14} />Sensor permissives bypassed</span>
      {:else}
        <span><ShieldCheck size={14} />Sensor permissives active</span>
      {/if}
      {#each station.retainedProtections as protection}
        <span><ShieldCheck size={14} />{protection.replaceAll("-", " ")}</span>
      {/each}
    </div>
  </section>
{/if}

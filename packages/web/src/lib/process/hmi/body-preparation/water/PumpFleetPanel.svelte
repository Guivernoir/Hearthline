<script lang="ts">
  import { Activity, CircleAlert, Radio, RotateCcw, Wrench } from "@lucide/svelte";
  import type { HmiAction, HmiWaterPumpState } from "../../hmi-api";

  export let pumps: HmiWaterPumpState[] = [];
  export let heartbeatTimeoutMs = 1500;
  export let busyTarget = "";
  export let diagnostics = false;
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  $: groups = [...new Set(pumps.map((pump) => pump.groupId))].map((id) => ({
    id,
    pumps: pumps.filter((pump) => pump.groupId === id),
  }));
</script>

<section class="water-pump-board" aria-label="Pump fleet">
  <header><span><Activity size={17} />Duplex pump fleet</span><small>Heartbeat timeout {heartbeatTimeoutMs} ms</small></header>
  <div class="water-pump-groups">
    {#each groups as group}
      <article>
        <header><strong>{group.pumps[0]?.service}</strong><small>{group.id}</small></header>
        <div class="water-pump-pair">
          {#each group.pumps as pump}
            <div class:failed={!pump.heartbeatOk} class:running={pump.runningFeedback}>
              <div class="water-pump-name"><span class="water-heartbeat"><Radio size={14} /></span><strong>{pump.label}</strong><small>{pump.preferredDuty ? "Preferred duty" : "Standby"}</small></div>
              <dl>
                <div><dt>Command</dt><dd>{pump.commanded ? "Run" : "Stop"}</dd></div>
                <div><dt>Feedback</dt><dd>{pump.runningFeedback ? "Running" : "Stopped"}</dd></div>
                <div><dt>Heartbeat</dt><dd>{pump.heartbeatOk ? `Healthy #${pump.heartbeatSequence}` : `Lost ${pump.heartbeatAgeMs} ms`}</dd></div>
                <div><dt>Maintenance</dt><dd>{pump.maintenance}</dd></div>
              </dl>
              {#if pump.maintenance === "required"}
                <button class="water-maintenance" type="button" disabled={Boolean(busyTarget)} onclick={() => onExecute({ kind: "dispatch-water-pump-maintenance", pumpId: pump.id }, pump.id)}><Wrench size={14} />Dispatch maintenance</button>
              {:else if pump.maintenance === "dispatched"}
                <p class="water-dispatched"><Wrench size={13} />Maintenance dispatched</p>
              {/if}
              {#if diagnostics}
                <button class="water-fault-toggle" type="button" disabled={Boolean(busyTarget)} onclick={() => onExecute({ kind: "set-water-pump-failure", pumpId: pump.id, failed: pump.heartbeatOk }, pump.id)}>
                  {#if pump.heartbeatOk}<CircleAlert size={14} />Fail heartbeat{:else}<RotateCcw size={14} />Restore heartbeat{/if}
                </button>
              {/if}
            </div>
          {/each}
        </div>
      </article>
    {/each}
  </div>
</section>

<style>
  .water-pump-board { min-width: 0; }
  .water-pump-board > header {
    display: flex;
    min-height: 38px;
    align-items: center;
    justify-content: space-between;
    gap: 9px;
    border-bottom: 1px solid #d2dcd7;
  }
  .water-pump-board > header span { display: flex; align-items: center; gap: 6px; color: #315f69; font-size: 10px; font-weight: 850; }
  .water-pump-board > header small { color: #75817a; font-size: 8px; }
  .water-pump-groups { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; padding-top: 8px; }
  .water-pump-groups > article { overflow: hidden; border: 1px solid #ccd7d2; border-radius: 3px; background: #f8faf9; }
  .water-pump-groups > article > header { display: flex; min-height: 38px; padding: 6px 9px; align-items: center; justify-content: space-between; gap: 8px; border-bottom: 1px solid #dce3df; }
  .water-pump-groups > article > header strong { color: #394942; font-size: 9px; }
  .water-pump-groups > article > header small { color: #78827c; font-size: 7px; }
  .water-pump-pair { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 1px; background: #d9e1dc; }
  .water-pump-pair > div { min-width: 0; padding: 8px; border-top: 4px solid #9ba8a0; background: #f9fbfa; }
  .water-pump-pair > div.running { border-top-color: #398663; }
  .water-pump-pair > div.failed { border-top-color: #a34034; background: #fbf1ef; }
  .water-pump-name { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 2px 6px; }
  .water-pump-name strong { overflow: hidden; color: #37463f; font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .water-pump-name small { grid-column: 2; color: #78837c; font-size: 7px; }
  .water-heartbeat { display: grid; width: 25px; height: 25px; grid-row: span 2; place-items: center; border: 1px solid #8da298; border-radius: 50%; color: #347c5d; background: #e9f2ed; }
  .failed .water-heartbeat { border-color: #a85b50; color: #a03e32; background: #f5dfdb; }
  .water-pump-pair dl { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); margin: 8px 0 0; gap: 1px; background: #e0e6e2; }
  .water-pump-pair dl div { min-height: 39px; padding: 6px; background: #f7f9f8; }
  .water-pump-pair dt { color: #78827c; font-size: 7px; }
  .water-pump-pair dd { margin: 5px 0 0; overflow-wrap: anywhere; color: #43534b; font-size: 8px; font-weight: 800; text-transform: capitalize; }
  .water-maintenance,
  .water-fault-toggle { display: flex; width: 100%; min-height: 30px; margin-top: 7px; align-items: center; justify-content: center; gap: 5px; border: 1px solid #a9743d; border-radius: 3px; color: #845322; background: #fff9ef; font-size: 8px; font-weight: 850; cursor: pointer; }
  .water-fault-toggle { border-color: #9e6257; color: #8d3c32; background: #fff; }
  .water-maintenance:disabled,
  .water-fault-toggle:disabled { opacity: .45; cursor: not-allowed; }
  .water-dispatched { display: flex; min-height: 30px; margin: 7px 0 0; align-items: center; justify-content: center; gap: 5px; color: #416d52; background: #eaf3ee; font-size: 8px; font-weight: 800; }
  @media (max-width: 900px) { .water-pump-groups { grid-template-columns: minmax(0, 1fr); } }
  @media (max-width: 520px) {
    .water-pump-board > header { padding: 7px 0; align-items: flex-start; flex-direction: column; }
    .water-pump-pair { grid-template-columns: minmax(0, 1fr); }
  }
</style>

<script lang="ts">
  import { onMount } from "svelte";
  import {
    ArrowRight,
    CheckCircle2,
    Database,
    LoaderCircle,
    Network,
    Send,
    Server,
    TriangleAlert,
  } from "@lucide/svelte";
  import {
    loadHistorianStatus,
    publishHmiTelemetry,
    type HistorianStatus,
  } from "../../hmi-api";
  import type { ScenarioReport, ScenarioTraceEntry } from "../../../../simulation/simulation-api";

  export let applianceId: string;

  let status: HistorianStatus | null = null;
  let publication: ScenarioReport | null = null;
  let publishing = false;
  let error = "";

  $: latest = status?.replica.latest ?? null;
  $: failed = Boolean(error || status?.lastError || (publication && !publication.expectation_met));

  onMount(() => {
    void refresh();
    const poll = window.setInterval(() => void refresh(), 1_000);
    return () => window.clearInterval(poll);
  });

  async function refresh() {
    try {
      status = await loadHistorianStatus(applianceId);
      error = "";
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Cannot read historian state";
    }
  }

  async function publish() {
    if (publishing) return;
    publishing = true;
    error = "";
    try {
      publication = await publishHmiTelemetry(applianceId);
      await refresh();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Telemetry publication failed";
    } finally {
      publishing = false;
    }
  }

  function routeTrace(report: ScenarioReport | null | undefined): ScenarioTraceEntry[] {
    return report?.trace.filter((entry) =>
      entry.kind === "transmission" || entry.kind === "media" || entry.kind === "delivery" || entry.kind === "drop"
    ) ?? [];
  }
</script>

<section class:failed class="hmi-telemetry hmi-historian" aria-label="Operations historian pipeline">
  <header>
    <span><Database size={16} />Historian pipeline</span>
    <button type="button" disabled={publishing || !latest} onclick={() => void publish()}>
      {#if publishing}<LoaderCircle class="spin" size={14} />{:else}<Send size={14} />{/if}
      Publish
    </button>
  </header>

  {#if !status}
    <div class="hmi-empty"><LoaderCircle class="spin" size={22} /><span>Reading historian state</span></div>
  {:else}
    <div class="historian-flow">
      <div>
        <Server size={15} />
        <span><small>Level 3 primary</small><strong>{status.local.storedRecords} / {status.local.capacity}</strong><em>{status.local.applianceId}</em></span>
      </div>
      <ArrowRight size={16} />
      <div class:stale={status.pendingRecords > 0}>
        <Database size={15} />
        <span><small>OT DMZ replica</small><strong>{status.replica.storedRecords} / {status.replica.capacity}</strong><em>{status.replica.applianceId}</em></span>
      </div>
    </div>

    <dl class="historian-metrics">
      <div><dt>Sample</dt><dd>{status.sampleIntervalMs} ms</dd></div>
      <div><dt>Pending</dt><dd>{status.pendingRecords}</dd></div>
      <div><dt>Attempts</dt><dd>{status.replicationAttempts}</dd></div>
      <div><dt>Dropped</dt><dd>{status.droppedUnreplicated}</dd></div>
    </dl>

    {#if error || status.lastError}
      <div class="historian-alert"><TriangleAlert size={15} /><span>{error || status.lastError}</span></div>
    {:else if latest}
      <div class="historian-latest">
        <div><CheckCircle2 size={15} /><span><strong>Replica current</strong><small>Sequence {latest.sequence} / process time {latest.capturedAtMs} ms</small></span></div>
        <code>{latest.payload}</code>
      </div>
    {:else}
      <div class="hmi-empty"><Database size={22} /><span>Awaiting first replicated sample</span></div>
    {/if}

    <div class="historian-routes">
      {#each [status.lastCollection, status.lastReplication] as transfer}
        {#if transfer}
          <details>
            <summary>
              <Network size={13} />{transfer.scenario_label}
              <em class:failed={!transfer.expectation_met}>{transfer.expectation_met ? "passed" : "failed"}</em>
            </summary>
            <ol>
              {#each routeTrace(transfer) as entry}
                <li class:drop={entry.kind === "drop"}><i>{entry.sequence + 1}</i><span><strong>{entry.component}</strong><small>{entry.summary}</small></span></li>
              {/each}
            </ol>
          </details>
        {/if}
      {/each}
    </div>

    {#if publication}
      <div class:failed={!publication.expectation_met} class="historian-publication">
        <strong>{publication.expectation_met ? "Published to Central Office" : "Publication failed"}</strong>
        <span>{publication.appliance_count} appliances / {publication.link_count} links / {publication.duration_us} us</span>
        <details>
          <summary><Network size={13} />Northbound route</summary>
          <ol>
            {#each routeTrace(publication) as entry}
              <li class:drop={entry.kind === "drop"}><i>{entry.sequence + 1}</i><span><strong>{entry.component}</strong><small>{entry.summary}</small></span></li>
            {/each}
          </ol>
        </details>
      </div>
    {/if}
  {/if}
</section>

<script lang="ts">
  import {
    Cable,
    CheckCircle2,
    CircleDot,
    Clock3,
    Factory,
    Route,
    Send,
    ShieldCheck,
    ShieldAlert,
    ShieldX,
    XCircle,
  } from "@lucide/svelte";
  import type {
    ScenarioExpectation,
    ScenarioContinuityFault,
    ScenarioReport,
    ScenarioTraceEntry,
    ScenarioTraceKind,
    TraceFilter,
  } from "./simulation-api";

  export let report: ScenarioReport | null = null;
  export let error = "";
  export let expectation: ScenarioExpectation | null = null;
  export let traceFilter: TraceFilter = "all";
  export let onOpenAppliance: (id: string) => void = () => {};

  $: filteredTrace = report
    ? report.trace.filter((entry) => includesTrace(entry, traceFilter))
    : [];
  $: resultHeading = report
    ? `${report.expectation_mode === "continuity"
        ? report.expectation.outcome === "dropped"
          ? "Fail-closed expectation"
          : "Continuity expectation"
        : report.expectation_mode === "recovery"
          ? "Recovery expectation"
          : report.expectation_mode === "isolation"
            ? "Isolation expectation"
          : report.expectation_mode === "autonomy"
            ? "Autonomy expectation"
          : "Expectation"} ${
        report.status === "passed" ? "met" : "failed"
      }`
    : "Awaiting execution";

  function includesTrace(entry: ScenarioTraceEntry, filter: TraceFilter) {
    if (filter === "all") return true;
    if (filter === "network") {
      return (
        entry.kind === "transmission" ||
        entry.kind === "delivery" ||
        entry.kind === "observation"
      );
    }
    if (filter === "media") return entry.kind === "media";
    return entry.kind === "drop";
  }

  function formatTime(microseconds: number) {
    if (microseconds < 1_000) return `${microseconds} us`;
    return `${(microseconds / 1_000).toFixed(3)} ms`;
  }

  function kindLabel(kind: ScenarioTraceKind) {
    return kind.replace("-", " ");
  }

  function faultLabel(fault: ScenarioContinuityFault) {
    return fault.type === "sync-link-loss"
      ? "HA sync loss"
      : "Standby state loss";
  }
</script>

<section class="trace-panel" aria-label="Simulation trace">
  <header class="trace-heading">
    <div>
      <span>Deterministic trace</span>
      <h2>{resultHeading}</h2>
    </div>
    {#if report}
      <span class:failed={report.status === "failed"} class="result-state">
        {#if report.status === "passed"}
          <CheckCircle2 size={16} strokeWidth={2} />
        {:else}
          <XCircle size={16} strokeWidth={2} />
        {/if}
        {report.status}
      </span>
    {/if}
  </header>

  {#if error}
    <div class="simulation-error" role="alert"><ShieldX size={17} />{error}</div>
  {/if}

  {#if report}
    <div class="trace-metrics">
      <span><Clock3 size={14} /> <b>{formatTime(report.duration_us)}</b> duration</span>
      <span><Route size={14} /> <b>{report.statistics.transmissions}</b> transmissions</span>
      <span><Cable size={14} /> <b>{report.statistics.media_transits}</b> media</span>
      <span><CircleDot size={14} /> <b>{report.statistics.drops}</b> drops</span>
    </div>

    {#if report.security}
      <div class={`simulation-security-event ${report.security.disposition}`}>
        <ShieldAlert size={16} />
        <span><strong>{report.security.severity} / {report.security.technique}</strong>{report.security.evidence}</span>
        <small>{report.security.disposition} / {report.security.defender}</small>
      </div>
    {/if}

    {#if report.continuity}
      <div
        class:degraded={report.continuity.faults.length > 0}
        class="simulation-continuity-event"
      >
        {#if report.continuity.faults.length > 0}
          <ShieldAlert size={17} />
          <span>
            <small>Injected fault</small>
            <strong>{faultLabel(report.continuity.faults[0])} / {formatTime(report.continuity.faults[0].at_us)}</strong>
          </span>
        {:else}
          <ShieldCheck size={17} />
          <span><small>Session state</small><strong>{report.continuity.synchronized_sessions} synchronized</strong></span>
        {/if}
        {#if report.continuity.faults.length > 0}
          <span>
            <small>State / sync</small>
            <strong>{report.continuity.synchronized_sessions} -> {report.continuity.sessions_after_continuation} / {report.continuity.sync_operational_at_failure ? "up" : "down"}</strong>
          </span>
        {:else}
          <span><small>Sync at failure</small><strong>{report.continuity.sync_operational_at_failure ? "up" : "down"}</strong></span>
        {/if}
        <span>
          <small>Last heartbeat</small>
          <strong>{formatTime(report.continuity.last_heartbeat_us)}</strong>
        </span>
        <span><small>Peer promotion</small><strong>{formatTime(report.continuity.promotion_at_us)}</strong></span>
      </div>
    {/if}

    {#if report.ha_isolation}
      <div class="simulation-continuity-event degraded">
        <ShieldCheck size={17} />
        <span>
          <small>HA isolation</small>
          <strong>Sync loss / {formatTime(report.ha_isolation.isolation_at_us)}</strong>
        </span>
        <span>
          <small>Ownership / sync</small>
          <strong>{report.ha_isolation.active_members} active / {report.ha_isolation.sync_operational ? "up" : "down"}</strong>
        </span>
        <span>
          <small>Standby fenced</small>
          <strong>{formatTime(report.ha_isolation.promotion_inhibited_at_us)}</strong>
        </span>
        <span>
          <small>Peer failure</small>
          <strong>{report.ha_isolation.peer_failure_confirmed ? "confirmed" : "unconfirmed"}</strong>
        </span>
      </div>
    {/if}

    {#if report.local_autonomy}
      <div class="simulation-continuity-event degraded local-autonomy-event">
        <Factory size={17} />
        <span>
          <small>Inter-site</small>
          <strong>{report.local_autonomy.outage_connections.length} handoffs down</strong>
        </span>
        <span>
          <small>Local path</small>
          <strong>{report.local_autonomy.local_path_connections.length} links / {report.local_autonomy.local_path_operational ? "up" : "down"}</strong>
        </span>
        <span>
          <small>Safety / command</small>
          <strong>{report.local_autonomy.safety_reset_applied ? "reset" : "latched"} / {report.local_autonomy.command_applied ? "applied" : "denied"}</strong>
        </span>
        <span>
          <small>Actuator</small>
          <strong>{report.local_autonomy.actuator} / {report.local_autonomy.actuator_state}</strong>
        </span>
      </div>
      <div class="simulation-control-trace" aria-label="Local control trace">
        {#each report.local_autonomy.control_trace as entry (entry.sequence)}
          <span>
            <small>{entry.stage}</small>
            <button
              type="button"
              title={`Open ${entry.component} configuration`}
              onclick={() => onOpenAppliance(entry.component)}
            >{entry.component}</button>
            <em>{entry.detail}</em>
          </span>
        {/each}
      </div>
    {/if}

    <div class="trace-filters segmented-control" aria-label="Trace filter">
      {#each ["all", "network", "media", "drops"] as filter}
        <button
          type="button"
          class:active={traceFilter === filter}
          onclick={() => (traceFilter = filter as TraceFilter)}
        >
          {filter}
        </button>
      {/each}
    </div>

    <div class="trace-list">
      {#each filteredTrace as entry (entry.sequence)}
        <article class:drop={entry.kind === "drop"} class:delivery={entry.kind === "delivery"}>
          <span class={`trace-icon ${entry.kind}`} aria-hidden="true">
            {#if entry.kind === "media"}
              <Cable size={15} strokeWidth={1.9} />
            {:else if entry.kind === "transmission"}
              <Send size={15} strokeWidth={1.9} />
            {:else if entry.kind === "delivery"}
              <CheckCircle2 size={15} strokeWidth={1.9} />
            {:else if entry.kind === "drop"}
              <ShieldX size={15} strokeWidth={1.9} />
            {:else}
              <CircleDot size={15} strokeWidth={1.9} />
            {/if}
          </span>
          <div>
            <header>
              <button
                type="button"
                title={`Open ${entry.component} configuration`}
                onclick={() => onOpenAppliance(entry.component)}
              >
                {entry.component}
              </button>
              <span>{kindLabel(entry.kind)}</span>
              <time>{formatTime(entry.time_us)}</time>
            </header>
            <p>{entry.summary}</p>
          </div>
        </article>
      {/each}
    </div>
  {:else if !error}
    <div class="trace-empty">
      <Send size={24} strokeWidth={1.6} />
      <strong>Run pending</strong>
      <span>{expectation?.component ?? "No target"} / {expectation?.service ?? expectation?.outcome ?? "No outcome"}</span>
    </div>
  {/if}
</section>

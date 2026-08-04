<script lang="ts">
  import { onMount } from "svelte";
  import {
    Activity,
    ArrowLeft,
    CheckCircle2,
    Clock3,
    FileText,
    LoaderCircle,
    Network,
    RefreshCw,
    Route,
    ShieldAlert,
    ShieldCheck,
    Trash2,
  } from "@lucide/svelte";
  import {
    acknowledgeSecurityEvent,
    clearSecurityConsole,
    loadSecurityConsole,
    type SecurityConsoleSession,
    type SecurityEventRecord,
  } from "./security-api";

  export let applianceId: string;
  export let onBack: () => void = () => {};
  export let onOpenConfig: (id: string) => void = () => {};

  type EventFilter = "all" | "active" | "acknowledged";

  let session: SecurityConsoleSession | null = null;
  let selectedId: number | null = null;
  let eventFilter: EventFilter = "all";
  let loading = true;
  let busy = false;
  let error = "";

  $: visibleEvents = filterEvents(session?.events ?? [], eventFilter);
  $: selected = visibleEvents.find((record) => record.id === selectedId) ??
    visibleEvents[0] ??
    null;

  onMount(() => {
    void refresh();
  });

  async function refresh() {
    if (busy) return;
    busy = true;
    error = "";
    try {
      install(await loadSecurityConsole(applianceId));
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Cannot load SOC session";
    } finally {
      loading = false;
      busy = false;
    }
  }

  async function acknowledge(record: SecurityEventRecord) {
    if (record.acknowledged || busy) return;
    busy = true;
    error = "";
    try {
      await acknowledgeSecurityEvent(record.id);
      install(await loadSecurityConsole(applianceId));
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Cannot acknowledge event";
    } finally {
      busy = false;
    }
  }

  async function clearEvents() {
    if (!session?.events.length || busy) return;
    busy = true;
    error = "";
    try {
      install(await clearSecurityConsole(applianceId));
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Cannot clear SOC session";
    } finally {
      busy = false;
    }
  }

  function install(next: SecurityConsoleSession) {
    session = next;
    const filtered = filterEvents(next.events, eventFilter);
    if (!filtered.some((record) => record.id === selectedId)) {
      selectedId = filtered[0]?.id ?? null;
    }
  }

  function selectFilter(filter: EventFilter) {
    eventFilter = filter;
    const filtered = filterEvents(session?.events ?? [], filter);
    selectedId = filtered.some((record) => record.id === selectedId)
      ? selectedId
      : filtered[0]?.id ?? null;
  }

  function filterEvents(records: SecurityEventRecord[], filter: EventFilter) {
    return records.filter((record) => {
      if (filter === "active") return !record.acknowledged;
      if (filter === "acknowledged") return record.acknowledged;
      return true;
    });
  }

  function display(value: string) {
    return value
      .replaceAll("-", " ")
      .replace(/\bhttp\b/gi, "HTTP")
      .replace(/\bwaf\b/gi, "WAF")
      .replace(/\bsql\b/gi, "SQL");
  }
</script>

<svelte:head>
  <title>Central SOC Console | Hearthline</title>
</svelte:head>

<div class="app-shell security-shell">
  <header class="topbar">
    <div class="brand-block">
      <button type="button" class="brand-back" aria-label="Back to Operations Intelligence" title="Back to Operations Intelligence" onclick={onBack}>
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark security-mark" aria-hidden="true"><ShieldCheck size={20} /></span>
      <div class="brand-copy"><strong>Central SOC Console</strong><span>Security operations</span></div>
    </div>
    <div class="view-context" aria-label="Current security console">
      <span>Operations Intelligence</span><Network size={14} /><strong>{applianceId}</strong>
    </div>
    <div class="toolbar" aria-label="Security console tools">
      <button type="button" aria-label="Refresh events" title="Refresh events" disabled={busy} onclick={() => void refresh()}>
        <RefreshCw class={busy ? "spin" : ""} size={17} />
      </button>
      <button type="button" aria-label="Clear event queue" title="Clear event queue" disabled={busy || !session?.events.length} onclick={() => void clearEvents()}>
        <Trash2 size={17} />
      </button>
      <button type="button" aria-label="View configuration" title="View configuration" onclick={() => onOpenConfig(applianceId)}>
        <FileText size={17} />
      </button>
    </div>
  </header>

  <main class="security-workspace">
    {#if loading}
      <div class="security-loading"><LoaderCircle class="spin" size={28} /><span>Opening security session</span></div>
    {:else if error && !session}
      <div class="security-loading error"><ShieldAlert size={28} /><strong>Security console unavailable</strong><span>{error}</span></div>
    {:else if session}
      <section class="security-overview" aria-label="Security session summary">
        <header>
          <span><Activity size={16} />Detection queue</span>
          <strong>SESSION {session.sequence.toString().padStart(4, "0")}</strong>
        </header>
        <dl>
          <div><dt>Active</dt><dd class="active-count">{session.activeCount}</dd></div>
          <div><dt>Acknowledged</dt><dd>{session.acknowledgedCount}</dd></div>
          <div><dt>Total evidence</dt><dd>{session.events.length}</dd></div>
          <div><dt>Console</dt><dd>{session.consoleId}</dd></div>
        </dl>
      </section>

      {#if error}
        <div class="security-inline-error" role="alert">{error}</div>
      {/if}

      <div class="security-console-layout">
        <aside class="security-event-queue" aria-label="Security event queue">
          <header><span>Events</span><strong>{session.activeCount} unacknowledged</strong></header>
          <div class="security-event-filters" role="group" aria-label="Filter security events">
            <button type="button" class:active={eventFilter === "all"} aria-pressed={eventFilter === "all"} onclick={() => selectFilter("all")}>
              <span>All</span><strong>{session.events.length}</strong>
            </button>
            <button type="button" class:active={eventFilter === "active"} aria-pressed={eventFilter === "active"} onclick={() => selectFilter("active")}>
              <span>Active</span><strong>{session.activeCount}</strong>
            </button>
            <button type="button" class:active={eventFilter === "acknowledged"} aria-pressed={eventFilter === "acknowledged"} onclick={() => selectFilter("acknowledged")}>
              <span>Acknowledged</span><strong>{session.acknowledgedCount}</strong>
            </button>
          </div>
          {#if visibleEvents.length}
            <div class="security-event-list">
              {#each visibleEvents as record (record.id)}
                <button
                  type="button"
                  class:selected={selected?.id === record.id}
                  class:acknowledged={record.acknowledged}
                  class={`severity-${record.event.severity}`}
                  onclick={() => (selectedId = record.id)}
                >
                  <span class="event-severity"><i></i>{record.event.severity}</span>
                  <strong>{display(record.event.technique)}</strong>
                  <small>SEC-{record.id.toString().padStart(6, "0")}</small>
                  <span>{record.event.detector}</span>
                </button>
              {/each}
            </div>
          {:else}
            <div class="security-empty">
              <ShieldCheck size={30} />
              <strong>No {eventFilter === "all" ? "session" : eventFilter} events</strong>
              <span>{session.events.length} total detections</span>
            </div>
          {/if}
        </aside>

        <section class="security-investigation" aria-label="Selected security event">
          {#if selected}
            <header>
              <div>
                <span>{selected.event.tactic} / {selected.event.severity}</span>
                <h1>{display(selected.event.technique)}</h1>
              </div>
              <span class:acknowledged={selected.acknowledged} class="event-state">
                {selected.acknowledged ? "Acknowledged" : "Active"}
              </span>
            </header>

            <div class="security-evidence">
              <section>
                <header><ShieldAlert size={15} /><span>Control evidence</span></header>
                <strong>{display(selected.event.disposition)}</strong>
                <p>{selected.event.evidence}</p>
              </section>
              <section>
                <header><Route size={15} /><span>Observed path</span></header>
                <dl>
                  <div><dt>Source</dt><dd>{selected.event.source_ip}</dd></div>
                  <div><dt>Destination</dt><dd>{selected.event.destination_ip}</dd></div>
                  <div><dt>Detector</dt><dd>{selected.event.detector}</dd></div>
                  <div><dt>Control</dt><dd>{selected.event.control}</dd></div>
                </dl>
              </section>
            </div>

            <footer>
              <span><Clock3 size={14} />Observed at {selected.event.observed_at_us} us</span>
              <span>{selected.event.scenario_id}</span>
              <button type="button" disabled={busy || selected.acknowledged} onclick={() => void acknowledge(selected)}>
                <CheckCircle2 size={16} />
                <span>{selected.acknowledged ? "Acknowledged" : "Acknowledge"}</span>
              </button>
            </footer>
          {:else}
            <div class="security-empty investigation-empty"><ShieldCheck size={34} /><strong>Queue clear</strong><span>No selected evidence</span></div>
          {/if}
        </section>
      </div>
    {/if}
  </main>

  <footer class="statusbar security-statusbar">
    <span class="status-state"><i></i>Security session</span>
    <span>{session?.activeCount ?? 0} active / {session?.events.length ?? 0} total</span>
    <span>{busy ? "Synchronizing" : "Local API connected"}</span>
  </footer>
</div>

<script lang="ts">
  import { onMount } from "svelte";
  import {
    Activity,
    AlarmClock,
    ArrowLeft,
    Check,
    CircleGauge,
    FileText,
    Gauge,
    LoaderCircle,
    Network,
    RotateCcw,
    ShieldAlert,
    ShieldCheck,
    Waves,
  } from "@lucide/svelte";
  import {
    loadHmiSnapshot,
    runHmiAction,
    type HmiAction,
    type HmiActionReport,
    type HmiSignal,
    type HmiSnapshot,
  } from "./hmi-api";

  export let applianceId: string;
  export let onBack: () => void = () => {};
  export let onOpenConfig: (id: string) => void = () => {};

  let snapshot: HmiSnapshot | null = null;
  let report: HmiActionReport | null = null;
  let loading = true;
  let busyTarget = "";
  let error = "";

  $: activeAlarms = snapshot?.alarms.filter((alarm) => alarm.active) ?? [];
  $: safetyTripped = snapshot?.safety.some((safety) => safety.tripLatched) ?? true;

  onMount(async () => {
    try {
      snapshot = await loadHmiSnapshot(applianceId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Cannot load HMI";
    } finally {
      loading = false;
    }
  });

  async function execute(action: HmiAction, target: string) {
    if (busyTarget) return;
    busyTarget = target;
    error = "";
    try {
      report = await runHmiAction(applianceId, action);
      snapshot = report.snapshot;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "HMI action failed";
    } finally {
      busyTarget = "";
    }
  }

  function signalPercent(signal: HmiSignal) {
    const span = signal.maximum - signal.minimum;
    if (span <= 0) return 0;
    return Math.min(100, Math.max(0, ((signal.value - signal.minimum) / span) * 100));
  }

  function isDiscreteSignal(signal: HmiSignal) {
    return signal.unit === "state" && signal.minimum === 0 && signal.maximum === 1;
  }

  function signalValue(signal: HmiSignal) {
    if (isDiscreteSignal(signal)) {
      return signal.value >= 0.5 ? "ACTIVE" : "INACTIVE";
    }
    return Number.isInteger(signal.value)
      ? signal.value.toFixed(0)
      : signal.value.toFixed(1);
  }

  function displayName(value: string) {
    return value.replaceAll("-", " ");
  }
</script>

<svelte:head>
  <title>{snapshot?.label ?? "HMI"} | Hearthline</title>
</svelte:head>

<div class="app-shell hmi-shell">
  <header class="topbar">
    <div class="brand-block">
      <button type="button" class="brand-back" aria-label="Back to process area" title="Back to process area" onclick={onBack}>
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark hmi-mark" aria-hidden="true"><Activity size={20} /></span>
      <div class="brand-copy"><strong>{snapshot?.label ?? applianceId}</strong><span>Operator interface</span></div>
    </div>
    <div class="view-context" aria-label="Current process area">
      <span>{snapshot?.environment ?? "Process area"}</span><Network size={14} /><strong>{snapshot?.zone ?? applianceId}</strong>
    </div>
    <div class="toolbar" aria-label="HMI tools">
      <button type="button" aria-label="View configuration" title="View configuration" onclick={() => onOpenConfig(applianceId)}>
        <FileText size={17} />
      </button>
    </div>
  </header>

  <main class="hmi-workspace">
    {#if loading}
      <div class="hmi-loading"><LoaderCircle class="spin" size={28} /><span>Connecting to operator interface</span></div>
    {:else if error && !snapshot}
      <div class="hmi-loading error"><ShieldAlert size={28} /><strong>HMI unavailable</strong><span>{error}</span></div>
    {:else if snapshot}
      <header class:tripped={safetyTripped} class="hmi-runtime-header">
        <div>
          <span class="hmi-state-icon">
            {#if safetyTripped}<ShieldAlert size={20} />{:else}<ShieldCheck size={20} />{/if}
          </span>
          <span><small>Process state</small><strong>{safetyTripped ? "Safety trip latched" : "Ready for operation"}</strong></span>
        </div>
        <dl>
          <div><dt>Controller</dt><dd>{snapshot.controller}</dd></div>
          <div><dt>Remote I/O</dt><dd>{snapshot.remoteIo}</dd></div>
          <div><dt>Sequence</dt><dd>{snapshot.sequence}</dd></div>
        </dl>
      </header>

      {#if error}
        <div class="hmi-toast error">{error}</div>
      {:else if report}
        <div class:denied={report.status === "denied"} class="hmi-toast">{report.message}</div>
      {/if}

      <div class="hmi-layout">
        <section class="hmi-process-panel" aria-label="Process overview">
          <header><span><Waves size={17} />Process overview</span><small>{snapshot.role}</small></header>

          <div class="hmi-signal-grid">
            {#each snapshot.signals as signal}
              <article class:bad-quality={!signal.qualityGood} class="hmi-instrument">
                <div
                  class:discrete={isDiscreteSignal(signal)}
                  class:active={isDiscreteSignal(signal) && signal.value >= 0.5}
                  class="hmi-gauge"
                  style={`--level: ${signalPercent(signal)}%`}
                >
                  <span></span><Gauge size={21} />
                </div>
                <div>
                  <small>{signal.label}</small>
                  <strong>
                    {signalValue(signal)}
                    {#if !isDiscreteSignal(signal)}<em>{displayName(signal.unit)}</em>{/if}
                  </strong>
                  <span>
                    {isDiscreteSignal(signal) ? "Binary input" : `${signal.minimum}–${signal.maximum}`}
                    / {signal.qualityGood ? "GOOD" : "BAD"}
                  </span>
                </div>
              </article>
            {/each}
          </div>

          <section class="hmi-actuator-section" aria-label="Actuator controls">
            <header><span><CircleGauge size={16} />Field outputs</span><small>{safetyTripped ? "Inhibited" : "Command enabled"}</small></header>
            <div class="hmi-actuator-grid">
              {#each snapshot.actuators as actuator}
                <article class="hmi-actuator">
                  <div>
                    <small>{actuator.label}</small>
                    <strong>{actuator.currentState}</strong>
                    <span>{actuator.commandTag}</span>
                  </div>
                  <div class="hmi-state-control" aria-label={`${actuator.label} state`}>
                    {#each actuator.states as state}
                      <button
                        type="button"
                        class:active={state === actuator.currentState}
                        aria-pressed={state === actuator.currentState}
                        disabled={safetyTripped || Boolean(busyTarget) || state === actuator.currentState}
                        onclick={() => void execute({ kind: "command", tag: actuator.commandTag, value: state }, actuator.commandTag)}
                      >
                        {#if busyTarget === actuator.commandTag}<LoaderCircle class="spin" size={13} />{/if}
                        {displayName(state)}
                      </button>
                    {/each}
                  </div>
                </article>
              {/each}
            </div>
          </section>

          <section class="hmi-safety-section" aria-label="Safety status">
            <header><span><ShieldCheck size={16} />Safety circuit</span><small>{snapshot.safety.length} interface</small></header>
            {#each snapshot.safety as safety}
              <div class:tripped={safety.tripLatched} class="hmi-safety-row">
                <div>
                  <strong>{safety.label}</strong>
                  <span>{safety.tripLatched ? "RESET REQUIRED" : "HEALTHY"}</span>
                </div>
                <ul>
                  {#each safety.permissives as permissive}
                    <li class:satisfied={permissive.satisfied}><i></i>{permissive.tag}</li>
                  {/each}
                </ul>
                <button
                  type="button"
                  disabled={!safety.tripLatched || Boolean(busyTarget)}
                  onclick={() => void execute({ kind: "reset-safety", safetyId: safety.componentId }, safety.componentId)}
                >
                  {#if busyTarget === safety.componentId}<LoaderCircle class="spin" size={14} />{:else}<RotateCcw size={14} />{/if}
                  Reset
                </button>
              </div>
            {/each}
          </section>
        </section>

        <aside class="hmi-event-panel" aria-label="Alarms and command trace">
          <section class="hmi-alarm-list">
            <header><span><AlarmClock size={16} />Alarms</span><strong>{activeAlarms.length} active</strong></header>
            {#if snapshot.alarms.length === 0}
              <div class="hmi-empty"><ShieldCheck size={22} /><span>No alarm records</span></div>
            {:else}
              {#each [...snapshot.alarms].reverse() as alarm}
                <article class:inactive={!alarm.active} class:acknowledged={alarm.acknowledged} class="hmi-alarm">
                  <i></i>
                  <div><strong>{alarm.code}</strong><span>{alarm.message}</span><small>{alarm.source}</small></div>
                  <button
                    type="button"
                    aria-label={`Acknowledge ${alarm.code}`}
                    title="Acknowledge alarm"
                    disabled={alarm.acknowledged || Boolean(busyTarget)}
                    onclick={() => void execute({ kind: "acknowledge-alarm", alarmId: alarm.id }, alarm.id)}
                  >
                    {#if busyTarget === alarm.id}<LoaderCircle class="spin" size={14} />{:else}<Check size={15} />{/if}
                  </button>
                </article>
              {/each}
            {/if}
          </section>

          <section class="hmi-trace">
            <header><span><Network size={16} />Last command path</span><strong class:denied={report?.status === "denied"}>{report?.status ?? "idle"}</strong></header>
            {#if report?.trace.length}
              <ol>
                {#each report.trace as entry}
                  <li><i>{entry.sequence + 1}</i><div><strong>{entry.component}</strong><span>{entry.stage}</span><small>{entry.detail}</small></div></li>
                {/each}
              </ol>
            {:else}
              <div class="hmi-empty"><Network size={22} /><span>No command trace</span></div>
            {/if}
          </section>

          <section class="hmi-audit">
            <header><span><Activity size={16} />Operator audit</span><strong>{snapshot.audit.length}</strong></header>
            {#each snapshot.audit.slice(-6).reverse() as entry}
              <div><i>{entry.sequence}</i><span><strong>{entry.action}</strong><small>{entry.target}</small></span><em>{entry.result}</em></div>
            {/each}
          </section>
        </aside>
      </div>

    {/if}
  </main>

  <footer class="statusbar hmi-statusbar">
    <span class="status-state"><i></i>{snapshot?.id ?? "Operator interface"}</span>
    <span>{snapshot?.controller ?? "Controller unavailable"}</span>
    <span>{activeAlarms.length} active alarm{activeAlarms.length === 1 ? "" : "s"}</span>
  </footer>
</div>

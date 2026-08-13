<script lang="ts">
  import { onMount } from "svelte";
  import {
    Activity,
    AlarmClock,
    ArrowLeft,
    Check,
    CircleGauge,
    Code2,
    FileText,
    FlaskConical,
    Gauge,
    LoaderCircle,
    Network,
    Pause,
    Play,
    Power,
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
    type HmiControlMode,
    type HmiSignal,
    type HmiSnapshot,
    type HmiProcessFault,
  } from "./hmi-api";
  import ControlProgramPanel from "./ControlProgramPanel.svelte";
  import ControlStationPanel from "./ControlStationPanel.svelte";
  import MachinePcWorkspace from "./machine/MachinePcWorkspace.svelte";
  import MouldVisualization from "./machine/MouldVisualization.svelte";
  import RobotPendant from "./machine/RobotPendant.svelte";

  export let applianceId: string;
  export let onBack: () => void = () => {};
  export let onOpenConfig: (id: string) => void = () => {};

  let snapshot: HmiSnapshot | null = null;
  let report: HmiActionReport | null = null;
  let loading = true;
  let busyTarget = "";
  let error = "";
  let showControlProgram = false;

  const processFaults: { id: HmiProcessFault; label: string }[] = [
    { id: "slip-supply-loss", label: "Slip supply" },
    { id: "compressed-air-loss", label: "Compressed air" },
    { id: "mould-overpressure", label: "Mould pressure" },
    { id: "vacuum-loss", label: "Vacuum" },
    { id: "robot-pickup-failure", label: "Robot pickup" },
  ];

  $: activeAlarms = snapshot?.alarms.filter((alarm) => alarm.active) ?? [];
  $: safetyTripped = snapshot?.safety.some((safety) => safety.tripLatched) ?? true;
  $: process = snapshot?.process ?? null;
  $: station = snapshot?.controlStation ?? null;
  $: isMachinePc = station?.stationType === "machine-pc";
  $: isMouldPanel = station?.stationType === "mould-panel";
  $: isRobotPendant = station?.stationType === "robot-joystick";
  $: localMould = isMouldPanel
    ? snapshot?.moulds.find((mould) => mould.target === station?.target) ?? null
    : null;
  $: processFaulted = process?.phase === "faulted";
  $: canStartMould = snapshot?.permissions.includes("start-mould") ?? false;
  $: canResetSafety = snapshot?.permissions.includes("reset-safety") ?? false;
  $: canInjectFaults = snapshot?.permissions.includes("inject-faults") ?? false;
  $: activePhaseIndex = localMould?.phases.findIndex((phase) => phase.key === localMould?.phase) ?? -1;

  onMount(() => {
    void refreshSnapshot(true);
    const poll = window.setInterval(() => void refreshSnapshot(false), 500);
    return () => window.clearInterval(poll);
  });

  async function refreshSnapshot(initial: boolean) {
    if (busyTarget) return;
    try {
      snapshot = await loadHmiSnapshot(applianceId);
      if (!initial) error = "";
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Cannot load HMI";
    } finally {
      if (initial) loading = false;
    }
  }

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

  function commandValue(actuator: HmiSnapshot["actuators"][number], state: string) {
    return state === actuator.currentState ? actuator.safeState : state;
  }

  function manualCommandEnabled(tag: string) {
    if (!station) return true;
    if (["mould-panel", "robot-joystick"].includes(station.stationType)) {
      return station.selectedMode !== "auto";
    }
    const mouldOneValve = [
      "area-02-water-01-command",
      "area-02-air-01-command",
      "area-02-vac-01-command",
    ].includes(tag);
    const match = tag.match(/^area-02-m(0[1-4])-/);
    const target = mouldOneValve ? "mould-01" : match ? `mould-${match[1]}` : null;
    if (!target) return true;
    return snapshot?.stationStatus.some(
      (candidate) => candidate.target === target && candidate.selectedMode === "manual",
    ) ?? false;
  }
</script>

<svelte:head>
  <title>{snapshot?.label ?? "HMI"} | Hearthline</title>
</svelte:head>

<div class:scada-workstation={snapshot?.interfaceKind === "scada-workstation"} class="app-shell hmi-shell">
  <header class="topbar">
    <div class="brand-block">
      <button type="button" class="brand-back" aria-label="Back to process area" title="Back to process area" onclick={onBack}>
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark hmi-mark" aria-hidden="true"><Activity size={20} /></span>
      <div class="brand-copy"><strong>{snapshot?.label ?? applianceId}</strong><span>{isMachinePc ? "Embedded machine PC / SCADA" : isRobotPendant ? "Robot motion pendant" : isMouldPanel ? "Mould local HMI" : snapshot?.interfaceKind === "scada-workstation" ? "SCADA workstation" : "Operator interface"}</span></div>
    </div>
    <div class="view-context" aria-label="Current process area">
      <span>{snapshot?.environment ?? "Process area"}</span><Network size={14} /><strong>{snapshot?.zone ?? applianceId}</strong>
    </div>
    <div class="toolbar" aria-label="HMI tools">
      {#if snapshot?.controlProgram}
        <button type="button" aria-label="View executing control source" title="Control source" onclick={() => (showControlProgram = true)}>
          <Code2 size={17} />
        </button>
      {/if}
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
      <header class:tripped={safetyTripped || processFaulted} class="hmi-runtime-header">
        <div>
          <span class="hmi-state-icon">
            {#if safetyTripped || processFaulted}<ShieldAlert size={20} />{:else}<ShieldCheck size={20} />{/if}
          </span>
          <span><small>Process state</small><strong>{safetyTripped ? "Safety trip latched" : processFaulted ? "Process faulted" : process ? displayName(process.phase) : "Ready for operation"}</strong></span>
        </div>
        <dl>
          <div><dt>Controller</dt><dd>{snapshot.controller}</dd></div>
          <div><dt>Remote I/O</dt><dd>{snapshot.remoteIoStations.length}</dd></div>
          <div><dt>PLC scans</dt><dd>{process?.scanCount ?? snapshot.sequence}</dd></div>
          <div><dt>Cycles</dt><dd>{process?.cycleCount ?? "-"}</dd></div>
        </dl>
      </header>

      {#if error}
        <div class="hmi-toast error">{error}</div>
      {:else if report}
        <div class:denied={report.status === "denied"} class="hmi-toast">{report.message}</div>
      {/if}

      <div class:machine-pc={isMachinePc} class="hmi-layout">
        {#if isMachinePc}
          <MachinePcWorkspace
            {snapshot}
            {report}
            {busyTarget}
            onExecute={(action: HmiAction, target: string) => void execute(action, target)}
          />
        {:else}
        <section class="hmi-process-panel" aria-label="Process overview">
          <header><span><Waves size={17} />{snapshot.interfaceKind === "scada-workstation" ? "Cell overview" : "Module overview"}</span><small>{snapshot.role}</small></header>

          {#if isMouldPanel && localMould}
            <MouldVisualization
              signals={snapshot.signals}
              actuators={snapshot.actuators}
              stations={snapshot.stationStatus}
              mould={localMould}
            />
          {/if}

          {#if isRobotPendant && station && snapshot.robot}
            <ControlStationPanel
              {station}
              busy={Boolean(busyTarget)}
              onSelect={(mode: HmiControlMode, password?: string) => void execute(
                { kind: "set-control-mode", mode, ...(password ? { password } : {}) },
                `mode-${mode}`,
              )}
            />
            <RobotPendant
              {station}
              robot={snapshot.robot}
              guardedCell={snapshot.guardedCell}
              actuators={snapshot.actuators}
              safety={snapshot.safety}
              {busyTarget}
              onExecute={(action: HmiAction, target: string) => void execute(action, target)}
            />
          {/if}

          {#if station && station.positions.length > 0 && !isRobotPendant}
            <ControlStationPanel
              {station}
              busy={Boolean(busyTarget)}
              onSelect={(mode: HmiControlMode, password?: string) => void execute(
                { kind: "set-control-mode", mode, ...(password ? { password } : {}) },
                `mode-${mode}`,
              )}
            />
          {/if}

          {#if localMould}
            <section class:tripped={localMould.phase === "faulted"} class="hmi-cycle-section" aria-label="Automatic forming cycle">
              <header>
                <span><Activity size={16} />Mould production</span>
                <div class="hmi-cycle-actions">
                  {#if canStartMould}
                    <button
                      type="button"
                      disabled={localMould.running || localMould.phase === "faulted" || safetyTripped || station?.selectedMode !== "auto" || Boolean(busyTarget)}
                      onclick={() => void execute({ kind: "start-mould" }, "start-mould")}
                    >
                      {#if busyTarget === "start-mould"}<LoaderCircle class="spin" size={14} />{:else}<Play size={14} />{/if}
                      Start
                    </button>
                    <button
                      type="button"
                      disabled={!localMould.running || Boolean(localMould.stopRequest) || Boolean(busyTarget)}
                      onclick={() => void execute({ kind: "stop-mould-after-phase" }, "stop-mould-after-phase")}
                    >
                      {#if busyTarget === "stop-mould-after-phase"}<LoaderCircle class="spin" size={14} />{:else}<Pause size={14} />{/if}
                      Stop
                    </button>
                    <button
                      type="button"
                      disabled={(!localMould.running && !localMould.paused) || localMould.stopRequest === "after-cycle" || Boolean(busyTarget)}
                      onclick={() => void execute({ kind: "end-mould-after-cycle" }, "end-mould-after-cycle")}
                    >
                      {#if busyTarget === "end-mould-after-cycle"}<LoaderCircle class="spin" size={14} />{:else}<Power size={14} />{/if}
                      End
                    </button>
                  {/if}
                </div>
              </header>
              <ol class="hmi-cycle-track">
                {#each localMould.phases as phase, index}
                  <li
                    class:active={phase.key === localMould.phase}
                    class:complete={localMould.running && index > 0 && index < activePhaseIndex}
                  >
                    <i>{index + 1}</i><span>{phase.label}</span>
                  </li>
                {/each}
              </ol>
              {#if canInjectFaults}
                <div class="hmi-fault-controls">
                  <span><FlaskConical size={15} />Simulation disturbances</span>
                  <div>
                    {#each processFaults as fault}
                      <button
                        type="button"
                        class:active={localMould.fault === fault.id}
                        disabled={Boolean(busyTarget)}
                        aria-pressed={localMould.fault === fault.id}
                        onclick={() => void execute(
                          { kind: "set-process-fault", fault: fault.id, active: localMould.fault !== fault.id },
                          `fault-${fault.id}`,
                        )}
                      >
                        {#if busyTarget === `fault-${fault.id}`}<LoaderCircle class="spin" size={13} />{/if}
                        {fault.label}
                      </button>
                    {/each}
                  </div>
                </div>
              {/if}
            </section>
          {/if}

          {#if !isRobotPendant}
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
            <header><span><CircleGauge size={16} />Field outputs</span><small>{safetyTripped ? "Inhibited" : localMould?.running ? "Automatic sequence" : "Command enabled"}</small></header>
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
                        disabled={safetyTripped || Boolean(localMould?.running) || !manualCommandEnabled(actuator.commandTag) || Boolean(busyTarget)}
                        onclick={() => void execute({ kind: "command", tag: actuator.commandTag, value: commandValue(actuator, state) }, actuator.commandTag)}
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
          {/if}

          <section class="hmi-safety-section" aria-label="Safety status">
            <header><span><ShieldCheck size={16} />Safety circuit</span><small>{snapshot.safety.length} interface</small></header>
            {#each snapshot.safety as safety}
              <div class:tripped={safety.tripLatched} class="hmi-safety-row">
                <div>
                  <strong>{safety.label}</strong>
                  <span>{safety.tripLatched ? "RESET REQUIRED" : safety.permissives.some((item) => !item.satisfied) ? "INHIBITED" : "HEALTHY"}</span>
                </div>
                <ul>
                  {#each safety.permissives as permissive}
                    <li class:satisfied={permissive.satisfied}><i></i>{permissive.tag}</li>
                  {/each}
                </ul>
                <button
                  type="button"
                  disabled={!canResetSafety || !safety.tripLatched || Boolean(busyTarget)}
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
        {/if}
      </div>

    {/if}
  </main>

  <footer class="statusbar hmi-statusbar">
    <span class="status-state"><i></i>{snapshot?.id ?? "Operator interface"}</span>
    <span>{snapshot?.controller ?? "Controller unavailable"}</span>
    <span>{activeAlarms.length} active alarm{activeAlarms.length === 1 ? "" : "s"}</span>
  </footer>
</div>

{#if showControlProgram && snapshot?.controlProgram}
  <ControlProgramPanel {applianceId} runtime={snapshot.controlProgram} onClose={() => (showControlProgram = false)} />
{/if}

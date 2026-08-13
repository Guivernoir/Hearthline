<script lang="ts">
  import {
    Activity,
    AlarmClock,
    BarChart3,
    Blocks,
    Check,
    ChevronRight,
    Cylinder,
    Factory,
    History,
    LayoutDashboard,
    LoaderCircle,
    Network,
    RotateCcw,
    Settings2,
    ShieldCheck,
    DoorOpen,
    DoorClosed,
    ArrowLeftRight,
  } from "@lucide/svelte";
  import type { Component } from "svelte";
  import HistorianPanel from "../HistorianPanel.svelte";
  import RecipeParameterPanel from "../RecipeParameterPanel.svelte";
  import type {
    HmiAction,
    HmiActionReport,
    HmiActuator,
    HmiMouldProcessState,
    HmiSignal,
    HmiSnapshot,
  } from "../hmi-api";
  import MouldVisualization from "./MouldVisualization.svelte";
  import SupervisoryWorkspace from "./SupervisoryWorkspace.svelte";

  export let snapshot: HmiSnapshot;
  export let report: HmiActionReport | null = null;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  type Page = "overview" | "slip-tank" | "production" | "trends" | "logs" | string;
  type NavigationItem = { id: Page; label: string; icon: Component };

  let page: Page = "overview";
  const fixedNavigation: NavigationItem[] = [
    { id: "overview", label: "Overview", icon: LayoutDashboard },
    { id: "slip-tank", label: "Slip tank", icon: Cylinder },
    { id: "production", label: "Production", icon: Factory },
    { id: "cell-safety", label: "Cell safety", icon: ShieldCheck },
    { id: "system", label: "System", icon: Blocks },
    { id: "trends", label: "Trends", icon: BarChart3 },
    { id: "logs", label: "Logs", icon: History },
  ];

  $: selectedMould = snapshot.moulds.find((mould) => mould.target === page) ?? null;
  $: selectedSignals = selectedMould ? signalsForMould(selectedMould.target) : [];
  $: selectedActuators = selectedMould ? actuatorsForMould(selectedMould.target) : [];
  $: tankSignals = snapshot.signals.filter((signal) => [
    "area-02-lt-01",
    "area-02-dt-01",
    "area-02-vis-01",
    "area-02-tt-01",
    "area-02-ft-01",
    "area-02-pt-01",
  ].includes(signal.tag));
  $: activeAlarms = snapshot.alarms.filter((alarm) => alarm.active);
  $: tankLevel = signal("area-02-lt-01")?.value ?? 0;

  function signalsForMould(target: string) {
    const index = Number(target.slice(-2));
    const tags = index === 1
      ? ["area-02-pt-02", "area-02-tt-02", "area-02-pos-01", "area-02-pos-02", "area-02-mt-02", "area-02-m01-inc-01"]
      : ["pt-01", "tt-01", "pos-01", "pos-02", "mt-01", "inc-01"].map((suffix) => `area-02-m${String(index).padStart(2, "0")}-${suffix}`);
    return snapshot.signals.filter((candidate) => tags.includes(candidate.tag));
  }

  function actuatorsForMould(target: string) {
    const index = Number(target.slice(-2));
    const tag = `area-02-m${String(index).padStart(2, "0")}-manifold-01-command`;
    return snapshot.actuators.filter((candidate) => candidate.commandTag === tag);
  }

  function signal(tag: string) {
    return snapshot.signals.find((candidate) => candidate.tag === tag);
  }

  function signalValue(item: HmiSignal) {
    const value = Number.isInteger(item.value) ? item.value.toFixed(0) : item.value.toFixed(1);
    return `${value} ${item.unit.replaceAll("-", " ")}`;
  }

  function display(value: string) {
    return value.replaceAll("-", " ");
  }

  function manualCommandEnabled(target: string) {
    return snapshot.stationStatus.some(
      (station) => station.target === target && station.selectedMode === "manual",
    );
  }

  function commandValue(actuator: HmiActuator, state: string) {
    return state === actuator.currentState ? actuator.safeState : state;
  }
</script>

<div class="machine-pc-layout">
  <nav class="machine-pc-navigation" aria-label="Machine PC sections">
    <div class="machine-pc-nav-group">
      {#each fixedNavigation as item}
        <button class:active={page === item.id} type="button" title={item.label} aria-label={item.label} onclick={() => (page = item.id)}>
          <item.icon size={19} /><span>{item.label}</span>
          {#if item.id === "logs" && activeAlarms.length}<i>{activeAlarms.length}</i>{/if}
        </button>
      {/each}
    </div>
    <div class="machine-pc-nav-group moulds">
      <small>Mould stations</small>
      {#each snapshot.moulds as mould, index}
        <button class:active={page === mould.target} type="button" onclick={() => (page = mould.target)}>
          <span class:running={mould.running} class:faulted={mould.fault}>{index + 1}</span>
          <strong>{mould.label}</strong>
          <small>{display(mould.operatingState)}</small>
        </button>
      {/each}
    </div>
  </nav>

  <section class="machine-pc-page">
    <header class="machine-pc-page-header">
      <div><small>Forming cell</small><ChevronRight size={13} /><strong>{selectedMould?.label ?? fixedNavigation.find((item) => item.id === page)?.label ?? "Overview"}</strong></div>
      <span><i></i>Live / {snapshot.controller}</span>
    </header>

    {#if page === "overview"}
      <div class="machine-pc-overview">
        <header><span><LayoutDashboard size={18} />Cell selection</span><small>Select one asset or operations workspace</small></header>
        <div class="machine-pc-asset-grid">
          {#each snapshot.moulds as mould, index}
            <button type="button" onclick={() => (page = mould.target)}>
              <span class:running={mould.running} class:faulted={mould.fault}><Settings2 size={22} /></span>
              <div><small>Mould station {index + 1}</small><strong>{mould.label}</strong><em>{display(mould.operatingState)} / cycle {mould.cycleCount}</em></div>
              <ChevronRight size={18} />
            </button>
          {/each}
          <button type="button" onclick={() => (page = "slip-tank")}>
            <span><Cylinder size={22} /></span><div><small>Material supply</small><strong>Slip tank</strong><em>{tankLevel.toFixed(1)} percent / {signal("area-02-tt-01")?.value.toFixed(1)} C</em></div><ChevronRight size={18} />
          </button>
          <button type="button" onclick={() => (page = "production")}>
            <span><Factory size={22} /></span><div><small>Operations</small><strong>Production</strong><em>{snapshot.moulds.reduce((total, mould) => total + mould.cycleCount, 0)} completed pieces</em></div><ChevronRight size={18} />
          </button>
          {#if snapshot.guardedCell}
            <button type="button" onclick={() => (page = "cell-safety")}>
              <span class:faulted={snapshot.guardedCell.guard.resetRequired}><ShieldCheck size={22} /></span><div><small>Guarded cell</small><strong>Safety and handoffs</strong><em>Gate {snapshot.guardedCell.guard.position} / {snapshot.guardedCell.handoffStations.filter((station) => station.state.includes("moving")).length} transfers moving</em></div><ChevronRight size={18} />
            </button>
          {/if}
          <button type="button" onclick={() => (page = "trends")}>
            <span><BarChart3 size={22} /></span><div><small>History</small><strong>Process trends</strong><em>Local and replicated process records</em></div><ChevronRight size={18} />
          </button>
          {#if snapshot.supervisory}
            <button type="button" onclick={() => (page = "system")}>
              <span><Blocks size={22} /></span><div><small>System model</small><strong>Assets and deployment</strong><em>{snapshot.supervisory.assets.length} instances / {snapshot.supervisory.deploymentNodes.length} nodes</em></div><ChevronRight size={18} />
            </button>
          {/if}
          <button type="button" onclick={() => (page = "logs")}>
            <span class:faulted={activeAlarms.length > 0}><History size={22} /></span><div><small>Diagnostics</small><strong>Alarms and logs</strong><em>{activeAlarms.length} active alarm{activeAlarms.length === 1 ? "" : "s"}</em></div><ChevronRight size={18} />
          </button>
        </div>
      </div>
    {:else if selectedMould}
      <div class="machine-pc-mould-page">
        <MouldVisualization
          signals={selectedSignals}
          actuators={selectedActuators}
          stations={snapshot.stationStatus}
          mould={selectedMould}
        />
        <RecipeParameterPanel
          parameters={snapshot.parameters}
          recipes={snapshot.recipes}
          activeRecipe={snapshot.activeRecipe}
          stations={snapshot.stationStatus}
          targetFilter={selectedMould.target}
          {busyTarget}
          onParameter={(parameterId: string, value: number) => onExecute(
            { kind: "set-parameter", parameterId, value },
            parameterId,
          )}
          onRecipe={(recipeId: string) => onExecute(
            { kind: "select-recipe", recipeId },
            `recipe-${recipeId}`,
          )}
        />
        {#if selectedActuators.length}
          <section class="machine-pc-faceplate" aria-label={`${selectedMould.label} valve service`}>
            <header><span><Settings2 size={16} />Valve service</span><small>{manualCommandEnabled(selectedMould.target) ? "Local manual authorization" : "Local selector must be manual"}</small></header>
            {#each selectedActuators as actuator}
              <div><strong>{display(actuator.currentState)}</strong><span>{actuator.commandTag}</span></div>
              <nav>
                {#each actuator.states as state}
                  <button
                    class:active={state === actuator.currentState}
                    type="button"
                    aria-pressed={state === actuator.currentState}
                    disabled={!manualCommandEnabled(selectedMould.target) || Boolean(busyTarget)}
                    onclick={() => onExecute(
                      { kind: "command", tag: actuator.commandTag, value: commandValue(actuator, state) },
                      actuator.commandTag,
                    )}
                  >{display(state)}</button>
                {/each}
              </nav>
            {/each}
          </section>
        {/if}
        {#if selectedMould.controlCabinet && selectedMould.utilityCabinet}
          <section class="machine-pc-cabinet" aria-label={`${selectedMould.label} cabinet state`}>
            <header><span><Network size={16} />External control / integrated utilities</span><strong>{display(selectedMould.utilityCabinet.activeState)}</strong></header>
            <div class="machine-pc-cabinet-summary">
              <article><small>Control cabinet</small><strong>{selectedMould.controlCabinet.remoteIo}</strong><span>{selectedMould.controlCabinet.enclosureRating} / {selectedMould.controlCabinet.controlVoltageVdc} VDC</span><em>{selectedMould.controlCabinet.modules.length} modules</em></article>
              <article><small>Integrated utility section</small><strong>{selectedMould.utilityCabinet.actuator}</strong><span>{selectedMould.utilityCabinet.enclosureRating} / {selectedMould.utilityCabinet.controlVoltageVdc} VDC</span><em>{display(selectedMould.utilityCabinet.activeState)}</em></article>
            </div>
            <div class="machine-pc-circuits">
              {#each selectedMould.utilityCabinet.circuits as circuit}
                <article class:active={circuit.state !== selectedMould.utilityCabinet.isolationState}><i></i><div><strong>{circuit.label}</strong><small>{display(circuit.medium)} / {circuit.source}</small></div><span>{display(circuit.state)}</span><em>{circuit.nominalPressure ?? "-"}</em></article>
              {/each}
            </div>
          </section>
        {/if}
      </div>
    {:else if page === "slip-tank"}
      <div class="machine-pc-tank-page">
        <section class="slip-tank-mimic">
          <header><span><Cylinder size={17} />Prepared slip buffer</span><strong>{tankLevel.toFixed(1)}%</strong></header>
          <div>
            <svg viewBox="0 0 440 390" role="img" aria-label={`Slip tank ${tankLevel.toFixed(1)} percent full`}>
              <path class="tank-shell" d="M105 52 H335 V300 Q335 344 220 362 Q105 344 105 300 Z"></path>
              <clipPath id="tank-fill-clip"><path d="M115 62 H325 V296 Q325 333 220 350 Q115 333 115 296 Z"></path></clipPath>
              <rect class="tank-fill" x="115" y={350 - 2.88 * tankLevel} width="210" height={2.88 * tankLevel} clip-path="url(#tank-fill-clip)"></rect>
              <path class="tank-agitator" d="M220 20 V285 M190 270 L220 286 L250 270"></path>
              <path class="tank-pipe" d="M55 92 H105 M335 290 H390 M390 290 V332"></path>
              <circle class="tank-drive" cx="220" cy="24" r="19"></circle>
              <text x="42" y="80">BODY PREPARATION</text><text x="348" y="278">MOULD HEADER</text>
            </svg>
            <div class="machine-pc-value-list">
              {#each tankSignals as item}<div><small>{item.label}</small><strong>{signalValue(item)}</strong><span>{item.qualityGood ? "GOOD" : "BAD"}</span></div>{/each}
            </div>
          </div>
        </section>
      </div>
    {:else if page === "cell-safety" && snapshot.guardedCell}
      <section class="machine-pc-cell-safety">
        <header>
          <span><ShieldCheck size={17} />Guarded-cell safety</span>
          <strong class:tripped={snapshot.guardedCell.guard.resetRequired}>{snapshot.guardedCell.guard.resetRequired ? "RESET REQUIRED" : snapshot.guardedCell.guard.closedPermissive ? "MOTION PERMITTED" : "MOTION INHIBITED"}</strong>
        </header>
        <div class="cell-gate-workspace">
          <div class:open={snapshot.guardedCell.guard.position === "open"} class="cell-gate-mimic" aria-label={`Access gate ${snapshot.guardedCell.guard.position}`}>
            <span class="cell-gate-post left"></span><span class="cell-gate-panel"></span><span class="cell-gate-post right"></span><i></i>
          </div>
          <div class="cell-gate-status">
            <small>Personnel access gate</small><strong>{snapshot.guardedCell.guard.position}</strong><span>{snapshot.guardedCell.guard.positionSensor}</span>
            <nav>
              <button type="button" disabled={snapshot.guardedCell.guard.position === "closed" || Boolean(busyTarget)} onclick={() => onExecute({ kind: "set-guard-door", open: false }, "guard-close")}><DoorClosed size={15} />Close</button>
              <button type="button" disabled={snapshot.guardedCell.guard.position === "open" || Boolean(busyTarget)} onclick={() => onExecute({ kind: "set-guard-door", open: true }, "guard-open")}><DoorOpen size={15} />Open</button>
              <button type="button" disabled={!snapshot.guardedCell.guard.resetRequired || !snapshot.guardedCell.guard.closedPermissive || Boolean(busyTarget)} onclick={() => onExecute({ kind: "reset-safety", safetyId: snapshot.guardedCell!.guard.safetyComponent }, "guard-reset")}><RotateCcw size={15} />Reset</button>
            </nav>
          </div>
        </div>
        <div class="cell-transfer-list">
          {#each snapshot.guardedCell.handoffStations as station, index}
            <article class:moving={station.state.includes("moving")} class:stopped={station.state === "stopped"}>
              <header><span><ArrowLeftRight size={15} />Mould {index + 1} handoff</span><strong>{display(station.state)}</strong></header>
              <div class="cell-transfer-track"><i style={`left: calc(${station.progressPercent}% - 18px)`}></i><span></span><b></b></div>
              <dl><div><dt>Robot side</dt><dd class:active={station.inCellConfirmed}>{station.inCellConfirmed ? "MADE" : "CLEAR"}</dd></div><div><dt>Operator side</dt><dd class:active={station.operatorSideConfirmed}>{station.operatorSideConfirmed ? "MADE" : "CLEAR"}</dd></div><div><dt>Piece</dt><dd>{station.piecePresent ? "PRESENT" : "EMPTY"}</dd></div><div><dt>Travel</dt><dd>{station.progressPercent.toFixed(0)}%</dd></div></dl>
              <small>{station.actuator}</small>
            </article>
          {/each}
        </div>
      </section>
    {:else if page === "production"}
      <section class="machine-pc-production">
        <header><span><Factory size={17} />Production status</span><strong>{snapshot.moulds.filter((mould) => mould.productionEnabled).length} enabled</strong></header>
        <div class="production-summary"><div><small>Total pieces</small><strong>{snapshot.moulds.reduce((total, mould) => total + mould.cycleCount, 0)}</strong></div><div><small>Producing</small><strong>{snapshot.moulds.filter((mould) => mould.running).length}</strong></div><div><small>Paused</small><strong>{snapshot.moulds.filter((mould) => mould.paused).length}</strong></div><div><small>Faulted</small><strong>{snapshot.moulds.filter((mould) => mould.fault).length}</strong></div></div>
        <div class="production-table">
          {#each snapshot.moulds as mould}
            <button type="button" onclick={() => (page = mould.target)}><span class:running={mould.running}></span><strong>{mould.label}</strong><em>{display(mould.operatingState)}</em><small>{display(mould.phase)}</small><b>{mould.cycleCount} cycles</b><ChevronRight size={15} /></button>
          {/each}
        </div>
      </section>
    {:else if page === "system" && snapshot.supervisory}
      <SupervisoryWorkspace supervisory={snapshot.supervisory} />
    {:else if page === "trends"}
      <div class="machine-pc-history"><HistorianPanel applianceId={snapshot.id} /></div>
    {:else if page === "logs"}
      <div class="machine-pc-logs">
        <section><header><span><AlarmClock size={16} />Alarm history</span><strong>{activeAlarms.length} active</strong></header>{#if snapshot.alarms.length}{#each [...snapshot.alarms].reverse() as alarm}<article class:inactive={!alarm.active}><i></i><div><strong>{alarm.code}</strong><span>{alarm.message}</span><small>{alarm.source}</small></div><button type="button" aria-label={`Acknowledge ${alarm.code}`} disabled={alarm.acknowledged || Boolean(busyTarget)} onclick={() => onExecute({ kind: "acknowledge-alarm", alarmId: alarm.id }, alarm.id)}>{#if busyTarget === alarm.id}<LoaderCircle class="spin" size={14} />{:else}<Check size={15} />{/if}</button></article>{/each}{:else}<div class="hmi-empty"><Check size={22} /><span>No alarm records</span></div>{/if}</section>
        <section><header><span><Activity size={16} />Operator audit</span><strong>{snapshot.audit.length}</strong></header>{#each [...snapshot.audit].reverse() as entry}<article><b>{entry.sequence}</b><div><strong>{display(entry.action)}</strong><span>{entry.target}</span></div><em>{entry.result}</em></article>{/each}</section>
        <section><header><span><Network size={16} />Last command path</span><strong>{report?.status ?? "idle"}</strong></header>{#if report?.trace.length}{#each report.trace as entry}<article><b>{entry.sequence + 1}</b><div><strong>{entry.component}</strong><span>{entry.stage}</span><small>{entry.detail}</small></div></article>{/each}{:else}<div class="hmi-empty"><Network size={22} /><span>No command trace</span></div>{/if}</section>
      </div>
    {/if}
  </section>
</div>

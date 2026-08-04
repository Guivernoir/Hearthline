<script lang="ts">
  import { onMount } from "svelte";
  import {
    ArrowLeft,
    Network,
    PackageOpen,
    Play,
    RefreshCw,
    RotateCcw,
    TerminalSquare,
  } from "@lucide/svelte";
  import {
    loadSimulationCatalog,
    runSimulation,
    type ScenarioPacket,
    type ScenarioReport,
    type ScenarioSummary,
    type ScenarioConnectionState,
    type ScenarioFirstHopState,
    type ScenarioFirewallHaState,
    type ScenarioLinkAggregationState,
    type ScenarioSpanningTreeState,
    type FirstHopRole,
    type FirewallHaRole,
    type TraceFilter,
  } from "./simulation-api";
  import ExecutionTopology from "./ExecutionTopology.svelte";
  import PacketComposer from "./PacketComposer.svelte";
  import SimulationTrace from "./SimulationTrace.svelte";
  import {
    applyScenarioRecovery,
    isScenarioRecoveryApplied,
    transitionFirewallHaRole,
  } from "./simulation-state";

  export let onBack: () => void = () => {};
  export let onOpenAppliance: (id: string) => void = () => {};

  let scenarios: ScenarioSummary[] = [];
  let selected: ScenarioSummary | null = null;
  let packet: ScenarioPacket | null = null;
  let connectionStates: ScenarioConnectionState[] = [];
  let firstHopStates: ScenarioFirstHopState[] = [];
  let firewallHaStates: ScenarioFirewallHaState[] = [];
  let linkAggregationStates: ScenarioLinkAggregationState[] = [];
  let spanningTreeStates: ScenarioSpanningTreeState[] = [];
  let report: ScenarioReport | null = null;
  let loading = true;
  let running = false;
  let error = "";
  let traceFilter: TraceFilter = "all";

  $: recoveryApplied = isScenarioRecoveryApplied(selected?.recovery ?? null, {
    connectionStates,
    firstHopStates,
    firewallHaStates,
  });
  $: controlledContract = Boolean(
    selected?.continuity || selected?.ha_isolation || selected?.local_autonomy,
  );
  $: activeExpectation =
    selected?.continuity
      ? selected.continuity.expectation
      : selected?.ha_isolation
      ? selected.ha_isolation.expectation
      : selected?.recovery && recoveryApplied
      ? selected.recovery.expectation
      : selected?.expectation ?? null;

  onMount(async () => {
    try {
      const catalog = await loadSimulationCatalog();
      scenarios = catalog.scenarios;
      if (scenarios[0]) selectScenario(scenarios[0]);
    } catch (reason) {
      error =
        reason instanceof Error ? reason.message : "Simulation API unavailable";
    } finally {
      loading = false;
    }
  });

  function selectScenario(scenario: ScenarioSummary) {
    selected = scenario;
    packet = structuredClone(scenario.packet);
    connectionStates = structuredClone(scenario.connection_states);
    firstHopStates = structuredClone(scenario.first_hop_states);
    firewallHaStates = structuredClone(scenario.firewall_ha_states);
    linkAggregationStates = structuredClone(scenario.link_aggregation_states);
    spanningTreeStates = structuredClone(scenario.spanning_tree_states);
    report = null;
    error = "";
    traceFilter = "all";
  }

  function selectScenarioById(id: string) {
    const scenario = scenarios.find((candidate) => candidate.id === id);
    if (scenario) selectScenario(scenario);
  }

  function resetPacket() {
    if (!selected) return;
    packet = structuredClone(selected.packet);
    connectionStates = structuredClone(selected.connection_states);
    firstHopStates = structuredClone(selected.first_hop_states);
    firewallHaStates = structuredClone(selected.firewall_ha_states);
    linkAggregationStates = structuredClone(selected.link_aggregation_states);
    spanningTreeStates = structuredClone(selected.spanning_tree_states);
    report = null;
    error = "";
  }

  async function executeScenario() {
    if (!selected || !packet || running) return;
    running = true;
    error = "";
    try {
      report = await runSimulation(
        selected.id,
        controlledContract ? null : packet,
        controlledContract
          ? null
          : connectionStates.map((connection) => ({
              connection: connection.id,
              operational: connection.operational,
            })),
        controlledContract
          ? null
          : firstHopStates.map(({ appliance, interface: port, role }) => ({
              appliance,
              interface: port,
              role,
            })),
        controlledContract
          ? null
          : firewallHaStates.map(({ appliance, role }) => ({ appliance, role })),
      );
      connectionStates = structuredClone(report.connection_states);
      firstHopStates = structuredClone(report.first_hop_states);
      firewallHaStates = structuredClone(report.firewall_ha_states);
      linkAggregationStates = structuredClone(report.link_aggregation_states);
      spanningTreeStates = structuredClone(report.spanning_tree_states);
    } catch (reason) {
      report = null;
      error =
        reason instanceof Error ? reason.message : "Simulation execution failed";
    } finally {
      running = false;
    }
  }

  function updateConnectionState(id: string, operational: boolean) {
    connectionStates = connectionStates.map((connection) =>
      connection.id === id ? { ...connection, operational } : connection,
    );
    firewallHaStates = firewallHaStates.map((state) =>
      state.sync_connection === id
        ? { ...state, sync_operational: operational }
        : state,
    );
    spanningTreeStates = [];
    linkAggregationStates = [];
    report = null;
    error = "";
  }

  function updateFirewallHaRole(
    appliance: string,
    role: FirewallHaRole,
  ) {
    const transitioned = transitionFirewallHaRole(
      firewallHaStates,
      firstHopStates,
      appliance,
      role,
    );
    if (!transitioned) return;
    firewallHaStates = transitioned.firewallHaStates;
    firstHopStates = transitioned.firstHopStates;
    report = null;
    error = "";
  }

  function updateFirstHopRole(
    appliance: string,
    port: string,
    role: FirstHopRole,
  ) {
    const selectedState = firstHopStates.find(
      (state) => state.appliance === appliance && state.interface === port,
    );
    if (!selectedState) return;
    firstHopStates = firstHopStates.map((state) => {
      const sameGroup =
        state.protocol === selectedState.protocol &&
        state.group === selectedState.group &&
        state.virtual_ip === selectedState.virtual_ip;
      if (role === "active" && sameGroup) {
        return { ...state, role: state === selectedState ? "active" : "standby" };
      }
      return state === selectedState ? { ...state, role } : state;
    });
    report = null;
    error = "";
  }

  function applyRecovery() {
    if (!selected?.recovery || running) return;
    ({ connectionStates, firstHopStates, firewallHaStates } =
      applyScenarioRecovery(selected.recovery, {
        connectionStates,
        firstHopStates,
        firewallHaStates,
      }));
    spanningTreeStates = [];
    linkAggregationStates = [];
    report = null;
    error = "";
  }
</script>

<svelte:head>
  <title>Simulation workspace | Hearthline</title>
</svelte:head>

<div class="app-shell simulation-shell">
  <header class="topbar">
    <div class="brand-block">
      <button
        type="button"
        class="brand-back"
        aria-label="Back to architecture"
        title="Back"
        onclick={onBack}
      >
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark simulation-mark" aria-hidden="true">
        <TerminalSquare size={20} strokeWidth={1.8} />
      </span>
      <div class="brand-copy">
        <strong>Hearthline</strong>
        <span>Simulation</span>
      </div>
    </div>

    <div class="view-context" aria-label="Current view">
      <span>Scenario</span>
      <strong>{selected?.label ?? "Simulation workspace"}</strong>
    </div>

    <div class="toolbar simulation-toolbar">
      <button
        type="button"
        aria-label="Reset packet"
        title="Reset packet"
        disabled={!packet || running}
        onclick={resetPacket}
      >
        <RotateCcw size={17} strokeWidth={1.9} />
      </button>
      {#if selected?.recovery}
        <button
          type="button"
          class="recovery-command"
          aria-label="Apply scenario recovery"
          title={selected.recovery.label}
          disabled={running || recoveryApplied}
          onclick={applyRecovery}
        >
          <RefreshCw size={16} strokeWidth={1.9} />
          <span>{recoveryApplied ? "Recovered" : "Recover"}</span>
        </button>
      {/if}
      <button
        type="button"
        class="run-command"
        disabled={!packet || running}
        onclick={executeScenario}
      >
        <Play size={16} strokeWidth={2} />
        <span>{running ? "Running" : "Run scenario"}</span>
      </button>
    </div>
  </header>

  <main class="simulation-workspace">
    <aside class="scenario-rail" aria-label="Simulation scenarios">
      <header>
        <span>Scenario catalog</span>
        <h1>Configured runs</h1>
      </header>
      {#if loading}
        <div class="scenario-state">Loading scenarios</div>
      {:else}
        <nav>
          {#each scenarios as scenario (scenario.id)}
            <button
              type="button"
              class:active={selected?.id === scenario.id}
              onclick={() => selectScenario(scenario)}
            >
              <Network size={18} strokeWidth={1.8} />
              <span>
                <strong>{scenario.label}</strong>
                <small>{scenario.category}</small>
              </span>
            </button>
          {/each}
        </nav>
      {/if}
    </aside>

    <div class="mobile-scenario-picker">
      <label for="mobile-scenario">Scenario</label>
      <select
        id="mobile-scenario"
        disabled={loading || scenarios.length === 0}
        value={selected?.id ?? ""}
        onchange={(event) => selectScenarioById(event.currentTarget.value)}
      >
        {#each scenarios as scenario (scenario.id)}
          <option value={scenario.id}>{scenario.label}</option>
        {/each}
      </select>
    </div>

    <section class="packet-panel" aria-label="Packet composer">
      {#if selected && packet}
        <header class="panel-heading">
          <div>
            <span>Packet composer</span>
            <h2>{selected.label}</h2>
            <p>{selected.summary}</p>
          </div>
          <span class="schema-version">schema {selected.schema_version}</span>
        </header>

        <PacketComposer
          bind:packet
          locked={controlledContract}
          onSubmit={() => void executeScenario()}
        />

        <ExecutionTopology
          participants={selected.participants}
          connections={connectionStates}
          firstHopStates={firstHopStates}
          {firewallHaStates}
          {linkAggregationStates}
          {spanningTreeStates}
          disabled={running || controlledContract}
          {onOpenAppliance}
          onConnectionChange={updateConnectionState}
          onFirstHopChange={updateFirstHopRole}
          onFirewallHaChange={updateFirewallHaRole}
        />
      {:else if !loading}
        <div class="simulation-empty"><PackageOpen size={24} />No scenario selected</div>
      {/if}
    </section>

    <SimulationTrace
      {report}
      {error}
      expectation={activeExpectation}
      bind:traceFilter
      {onOpenAppliance}
    />
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>Rust simulation API</span>
    <span>{report ? `${report.appliance_count} appliances / ${report.link_count} links` : `${scenarios.length} scenarios`}</span>
    <span>{running ? "Executing" : report ? `${report.statistics.events} trace entries` : "Ready"}</span>
  </footer>
</div>

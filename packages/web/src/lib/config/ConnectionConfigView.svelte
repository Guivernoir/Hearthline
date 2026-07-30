<script lang="ts">
  import { onMount } from "svelte";
  import {
    ArrowLeft,
    Cable,
    Network,
    Router,
  } from "@lucide/svelte";
  import YamlEditor from "./YamlEditor.svelte";
  import {
    SUPPORTED_CONNECTION_SCHEMA,
    findAppliance,
    findConnection,
    installCatalog,
    type FrontendConnection,
  } from "./appliance-config";
  import {
    configurationApiAvailable,
    saveConnection,
  } from "./config-api";

  export let connectionId: string;
  export let onBack: () => void = () => {};
  export let onOpenAppliance: (id: string) => void = () => {};

  let loadedId = "";
  let connection: FrontendConnection | null = null;
  let apiWritable = false;
  let saving = false;
  let saveError = "";

  $: if (loadedId !== connectionId) {
    loadedId = connectionId;
    connection = findConnection(connectionId);
    saveError = "";
  }

  onMount(async () => {
    apiWritable = await configurationApiAvailable();
  });

  async function saveConfiguration(sourceYaml: string) {
    if (!connection || !apiWritable) return false;
    saving = true;
    saveError = "";
    try {
      const catalog = await saveConnection(
        connection.id,
        sourceYaml,
        connection.revision,
      );
      installCatalog(catalog);
      connection =
        catalog.connections.find((candidate) => candidate.id === connectionId) ??
        null;
      return true;
    } catch (error) {
      saveError =
        error instanceof Error ? error.message : "Configuration save failed";
      return false;
    } finally {
      saving = false;
    }
  }

  function applianceLabel(id: string) {
    return findAppliance(id)?.label ?? id;
  }
</script>

<svelte:head>
  <title>{connection?.label ?? "Connection configuration"} | Hearthline</title>
</svelte:head>

<div class="app-shell config-shell">
  <header class="topbar">
    <div class="brand-block">
      <button
        type="button"
        class="brand-back"
        aria-label="Back to appliance configuration"
        title="Back"
        onclick={onBack}
      >
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark" aria-hidden="true"><Network size={20} strokeWidth={1.8} /></span>
      <div class="brand-copy">
        <strong>Hearthline</strong>
        <span>Configuration</span>
      </div>
    </div>

    <div class="view-context" aria-label="Current view">
      <span>Connection</span>
      <strong>{connection?.label ?? connectionId}</strong>
    </div>
  </header>

  <main class="config-workspace">
    {#if connection}
      <header class="config-heading">
        <div>
          <span>{connection.transport} / {connection.medium}</span>
          <h1>{connection.label}</h1>
          <p>{connection.endpointA.appliance} to {connection.endpointB.appliance}</p>
        </div>
        <span class:down={!connection.initialOperational} class="config-lifecycle">
          {connection.initialOperational ? connection.lifecycle : "down"}
        </span>
      </header>

      <div class="config-layout">
        <aside class="config-metadata" aria-label="Connection summary">
          <dl>
            <div>
              <dt>Stable ID</dt>
              <dd>{connection.id}</dd>
            </div>
            <div>
              <dt>Transport</dt>
              <dd>{connection.transport}</dd>
            </div>
            <div>
              <dt>Medium</dt>
              <dd>{connection.medium} / {connection.mediumDetail}</dd>
            </div>
            <div>
              <dt>Link capacity</dt>
              <dd>{connection.capacityMbps.toLocaleString()} Mbps</dd>
            </div>
            <div>
              <dt>Effective MTU / Latency</dt>
              <dd>{connection.effectiveMtu} bytes / {connection.latencyMs} ms</dd>
            </div>
            <div>
              <dt>Duplex / Direction</dt>
              <dd>{connection.negotiatedDuplex} / {connection.direction}</dd>
            </div>
            <div>
              <dt>Source</dt>
              <dd>{connection.sourcePath}</dd>
            </div>
          </dl>

          <section class="config-endpoints">
            <span>Endpoints</span>
            {#each [connection.endpointA, connection.endpointB] as endpoint}
              <button
                type="button"
                onclick={() => onOpenAppliance(endpoint.appliance)}
              >
                <Router size={14} strokeWidth={1.9} />
                <span>
                  <b>{applianceLabel(endpoint.appliance)}</b>
                  <small>{endpoint.appliance} / {endpoint.interface}</small>
                  <small>{endpoint.hardware} / {endpoint.speedMbps.toLocaleString()} Mbps / MTU {endpoint.mtu}</small>
                  <small>Admin {endpoint.administrativeState} / initial oper {endpoint.initialOperationalState}</small>
                </span>
              </button>
            {/each}
          </section>

          <section>
            <span>Modeled behavior</span>
            <small>{connection.configuredOperational ? "Connection enabled" : "Connection administratively down"}</small>
            <small>{connection.initialOperational ? "Both endpoint ports are initially usable" : "At least one link condition is down"}</small>
            <small>{connection.lossEvery ? `Drops every ${connection.lossEvery} frames` : "No deterministic loss configured"}</small>
            <small>{connection.direction === "bidirectional" ? "Both endpoint directions accepted" : `${connection.direction} transit only`}</small>
            <small>Negotiated duplex: {connection.negotiatedDuplex}; collision timing is not yet modeled</small>
          </section>

          <section>
            <span>Physical model</span>
            {#each connection.physicalFacts as fact}
              <small>{fact}</small>
            {/each}
            <small>Calculated propagation delay: {connection.physicalDelayUs} us</small>
          </section>
        </aside>

        <YamlEditor
          sourceYaml={connection.sourceYaml}
          schemaVersion={SUPPORTED_CONNECTION_SCHEMA}
          writable={apiWritable}
          {saving}
          {saveError}
          onSave={saveConfiguration}
        />
      </div>
    {:else}
      <div class="config-missing">
        <Cable size={24} strokeWidth={1.8} />
        <strong>Unknown connection configuration</strong>
        <button type="button" onclick={onBack}>Return</button>
      </div>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>Rust-validated connection</span>
    <span>{connection?.sourcePath ?? "Unknown source"}</span>
    <span>{apiWritable ? "Editor ready" : "Read-only"}</span>
  </footer>
</div>

<script lang="ts">
  import { onMount } from "svelte";
  import { ArrowLeft, Cable, GitFork, Network, ShieldCheck } from "@lucide/svelte";
  import YamlEditor from "./YamlEditor.svelte";
  import {
    SUPPORTED_APPLIANCE_SCHEMA,
    findAppliance,
    findConnectionsForAppliance,
    installCatalog,
    type FrontendAppliance,
  } from "./appliance-config";
  import {
    configurationApiAvailable,
    saveAppliance,
  } from "./config-api";

  export let applianceId: string;
  export let onBack: () => void = () => {};
  export let onOpenConnection: (id: string) => void = () => {};

  let loadedId = "";
  let appliance: FrontendAppliance | null = null;
  let apiWritable = false;
  let saving = false;
  let saveError = "";

  $: if (loadedId !== applianceId) {
    loadedId = applianceId;
    appliance = findAppliance(applianceId);
    saveError = "";
  }
  $: connections = appliance
    ? findConnectionsForAppliance(appliance.id)
    : [];
  $: operationalPorts = appliance
    ? appliance.interfaces.filter(
        (port) =>
          port.administrativeState === "up" &&
          port.initialOperationalState === "up",
      ).length
    : 0;

  onMount(async () => {
    apiWritable = await configurationApiAvailable();
  });

  async function saveConfiguration(sourceYaml: string) {
    if (!appliance || !apiWritable) return false;
    saving = true;
    saveError = "";
    try {
      const catalog = await saveAppliance(
        appliance.id,
        sourceYaml,
        appliance.revision,
      );
      installCatalog(catalog);
      appliance =
        catalog.appliances.find((candidate) => candidate.id === applianceId) ??
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

  function peerLabel(connectionId: string) {
    const connection = connections.find((candidate) => candidate.id === connectionId);
    if (!connection || !appliance) return "";
    const peer =
      connection.endpointA.appliance === appliance.id
        ? connection.endpointB
        : connection.endpointA;
    return `${peer.appliance} / ${peer.interface}`;
  }
</script>

<svelte:head>
  <title>{appliance?.label ?? "Appliance configuration"} | Hearthline</title>
</svelte:head>

<div class="app-shell config-shell">
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
      <span class="brand-mark" aria-hidden="true"><Network size={20} strokeWidth={1.8} /></span>
      <div class="brand-copy">
        <strong>Hearthline</strong>
        <span>Configuration</span>
      </div>
    </div>

    <div class="view-context" aria-label="Current view">
      <span>Appliance</span>
      <strong>{appliance?.label ?? applianceId}</strong>
    </div>
  </header>

  <main class="config-workspace">
    {#if appliance}
      <header class="config-heading">
        <div>
          <span>{appliance.site} / {appliance.environment}</span>
          <h1>{appliance.label}</h1>
          <p>{appliance.summary}</p>
        </div>
        <span class="config-lifecycle">{appliance.lifecycle}</span>
      </header>

      <div class="config-layout">
        <aside class="config-metadata" aria-label="Configuration summary">
          <dl>
            <div>
              <dt>Stable ID</dt>
              <dd>{appliance.id}</dd>
            </div>
            <div>
              <dt>Kind</dt>
              <dd>{appliance.kind}</dd>
            </div>
            <div>
              <dt>Behavior family</dt>
              <dd>{appliance.behaviorFamily}</dd>
            </div>
            <div>
              <dt>Zone</dt>
              <dd>{appliance.zone}</dd>
            </div>
            <div>
              <dt>Role</dt>
              <dd>{appliance.role}</dd>
            </div>
            <div>
              <dt>Source</dt>
              <dd>{appliance.sourcePath}</dd>
            </div>
          </dl>

          <section>
            <span>Interfaces</span>
            <strong>{operationalPorts} / {appliance.interfaceCount} up</strong>
            {#if appliance.addresses.length > 0}
              {#each appliance.addresses as address}
                <small>{address}</small>
              {/each}
            {:else}
              <small>No Layer 3 address</small>
            {/if}
          </section>

          {#if appliance.spanningTree}
            <section>
              <span><GitFork size={13} strokeWidth={1.8} /> Spanning tree</span>
              <strong>{appliance.spanningTree.protocol.toUpperCase()}</strong>
              <small>Bridge priority {appliance.spanningTree.bridgePriority}</small>
              <small>Bridge MAC {appliance.spanningTree.bridgeMac}</small>
            </section>
          {/if}

          {#if appliance.linkAggregation}
            <section>
              <span><GitFork size={13} strokeWidth={1.8} /> Link aggregation</span>
              <strong>{appliance.linkAggregation.groups.length} LACP {appliance.linkAggregation.groups.length === 1 ? "bundle" : "bundles"}</strong>
              <small>System MAC {appliance.linkAggregation.systemMac}</small>
              {#each appliance.linkAggregation.groups as group}
                <small>{group.id} / {group.mode} / min {group.minimumActiveMembers} / {group.members.join(", ")}</small>
              {/each}
            </section>
          {/if}

          {#if appliance.multiChassis}
            <section>
              <span><Network size={13} strokeWidth={1.8} /> Multi-chassis</span>
              <strong>{appliance.multiChassis.domain}</strong>
              <small>{appliance.multiChassis.role} / peer {appliance.multiChassis.peer}</small>
              <small>Peer link {appliance.multiChassis.peerLink}</small>
            </section>
          {/if}

          {#if appliance.firewallHa}
            <section>
              <span><ShieldCheck size={13} strokeWidth={1.8} /> Firewall HA</span>
              <strong>{appliance.firewallHa.domain}</strong>
              <small>{appliance.firewallHa.role} / peer {appliance.firewallHa.peer}</small>
              <small>Sync {appliance.firewallHa.syncInterface} / sessions {appliance.firewallHa.sessionSync ? "enabled" : "disabled"}</small>
              <small>Monitors {appliance.firewallHa.monitoredInterfaces.join(", ")}</small>
            </section>
          {/if}

          <section class="config-port-list">
            <span>Port configuration</span>
            {#each appliance.interfaces as port}
              <div>
                <header>
                  <Cable size={14} strokeWidth={1.9} />
                  <b>{port.id}</b>
                  <i
                    class:down={port.initialOperationalState === "down"}
                    aria-label={`Operational state ${port.initialOperationalState}`}
                  >
                    {port.initialOperationalState}
                  </i>
                </header>
                <small>{port.hardware} / {port.mode}</small>
                <small>{port.speedMbps.toLocaleString()} Mbps / {port.duplex} / MTU {port.mtu}</small>
                {#if port.firstHop}
                  <small>{port.firstHop.protocol.toUpperCase()} {port.firstHop.group} / {port.firstHop.virtualIp} / priority {port.firstHop.priority} / {port.firstHop.initialRole}</small>
                {/if}
                <small>Supports {port.supportedMedia.join(", ")}</small>
              </div>
            {/each}
          </section>

          {#if appliance.services.length > 0}
            <section>
              <span>Accepted services</span>
              {#each appliance.services as service}
                <small>{service}</small>
              {/each}
            </section>
          {/if}

          <section>
            <span>Parsed behavior</span>
            {#each appliance.behaviorFacts as fact}
              <small>{fact}</small>
            {/each}
          </section>

          <section class="config-connection-list">
            <span>Connections</span>
            <strong>{connections.length}</strong>
            {#each connections as connection}
              <button
                type="button"
                onclick={() => onOpenConnection(connection.id)}
              >
                <Cable size={14} strokeWidth={1.9} />
                <span>
                  <b>{connection.label}</b>
                  <small>{peerLabel(connection.id)}</small>
                </span>
              </button>
            {/each}
          </section>
        </aside>

        <YamlEditor
          sourceYaml={appliance.sourceYaml}
          schemaVersion={SUPPORTED_APPLIANCE_SCHEMA}
          writable={apiWritable}
          {saving}
          {saveError}
          onSave={saveConfiguration}
        />
      </div>
    {:else}
      <div class="config-missing">
        <strong>Unknown appliance configuration</strong>
        <button type="button" onclick={onBack}>Return to architecture</button>
      </div>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-state"><i></i>Rust-validated configuration</span>
    <span>{appliance?.sourcePath ?? "Unknown source"}</span>
    <span>{apiWritable ? "Editor ready" : "Read-only"}</span>
  </footer>
</div>

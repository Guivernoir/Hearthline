<script lang="ts">
  import { FileText, LoaderCircle, Monitor, Network, Play, Router, Server, ShieldCheck } from "@lucide/svelte";
  import {
    runWorkstationAction,
    type RuntimeDeviceSnapshot,
    type WorkstationActionReport,
    type WorkstationNetworkState,
  } from "./workstation-api";

  type RuntimeTable = "status" | "mac" | "neighbors" | "pat" | "sessions";

  export let workstationId: string;
  export let networkState: WorkstationNetworkState | null = null;
  export let onResult: (report: WorkstationActionReport) => void = () => {};
  export let onOpenConfig: (id: string) => void = () => {};

  let selectedAppliance = "";
  let activeTable: RuntimeTable = "status";
  let consoleCommand = "show status";
  let consoleOutput = ["Runtime console ready."];
  let consoleStatus: WorkstationActionReport["status"] = "completed";
  let busy = false;

  $: devices = networkState?.devices ?? [];
  $: if (!selectedAppliance || !devices.some((device) => device.id === selectedAppliance)) {
    selectedAppliance = devices[0]?.id ?? "";
  }
  $: selectedDevice = devices.find((device) => device.id === selectedAppliance) ?? null;
  $: availableTables = tableOptions(selectedDevice);
  $: if (!availableTables.some((table) => table.id === activeTable)) activeTable = "status";

  function tableOptions(device: RuntimeDeviceSnapshot | null) {
    const tables: { id: RuntimeTable; label: string; count?: number }[] = [
      { id: "status", label: "Status" },
    ];
    if (device?.supportsMacTable) tables.push({ id: "mac", label: "CAM", count: device.macTable.length });
    if (device?.supportsNeighbors) tables.push({ id: "neighbors", label: "Neighbors", count: device.neighbors.length });
    if (device?.supportsPat) tables.push({ id: "pat", label: "PAT", count: device.patTranslations.length });
    if (device?.supportsFirewallSessions) tables.push({ id: "sessions", label: "Sessions", count: device.firewallSessions.length });
    return tables;
  }

  function selectTable(table: RuntimeTable) {
    activeTable = table;
    consoleCommand = commandFor(table);
  }

  function commandFor(table: RuntimeTable) {
    return {
      status: "show status",
      mac: "show mac address-table",
      neighbors: "show arp",
      pat: "show ip nat translations",
      sessions: "show sessions",
    }[table];
  }

  function endpoint(address: string, port: number | null) {
    return port === null ? address : `${address}:${port}`;
  }

  async function inspect() {
    const command = consoleCommand.trim();
    if (!selectedDevice || !command || busy) return;
    busy = true;
    try {
      const report = await runWorkstationAction(workstationId, {
        kind: "inspect",
        appliance: selectedDevice.id,
        command,
      });
      consoleOutput = report.output;
      consoleStatus = report.status;
      onResult(report);
    } catch (cause) {
      consoleOutput = [cause instanceof Error ? cause.message : "Runtime inspection failed"];
      consoleStatus = "failed";
    } finally {
      busy = false;
    }
  }
</script>

<section class="network-state-app" aria-label="Runtime network state">
  <header class="runtime-titlebar">
    <span><Network size={16} strokeWidth={1.9} />Network state</span>
    <small>Session context / {networkState?.simulatedTimeMs ?? 0} ms</small>
  </header>

  {#if !networkState?.active || devices.length === 0}
    <div class="runtime-empty">
      <Network size={30} />
      <strong>Runtime inactive</strong>
      <span>No network state has been learned in this endpoint session.</span>
    </div>
  {:else}
    <div class="runtime-workspace">
      <aside class="runtime-devices">
        <label for="runtime-device">Runtime appliance</label>
        <select id="runtime-device" bind:value={selectedAppliance}>
          {#each devices as device}<option value={device.id}>{device.id}</option>{/each}
        </select>
        <div class="runtime-device-list">
          {#each devices as device}
            <button class:active={device.id === selectedAppliance} type="button" onclick={() => (selectedAppliance = device.id)}>
              {#if device.supportsFirewallSessions}
                <ShieldCheck size={16} />
              {:else if device.kind.includes("switch")}
                <Network size={16} />
              {:else if device.kind === "workstation"}
                <Monitor size={16} />
              {:else if device.kind.includes("service")}
                <Server size={16} />
              {:else}
                <Router size={16} />
              {/if}
              <span><strong>{device.id}</strong><small>{device.kind}</small></span>
            </button>
          {/each}
        </div>
      </aside>

      <div class="runtime-detail">
        <header class="runtime-appliance-header">
          <div><small>Selected appliance</small><strong>{selectedDevice?.id}</strong><span>{selectedDevice?.kind}</span></div>
          <button type="button" aria-label="View appliance configuration" title="View appliance configuration" onclick={() => selectedDevice && onOpenConfig(selectedDevice.id)}>
            <FileText size={17} />
          </button>
        </header>

        <nav class="runtime-tabs" aria-label="Runtime tables">
          {#each availableTables as table}
            <button class:active={activeTable === table.id} type="button" onclick={() => selectTable(table.id)}>
              {table.label}{#if table.count !== undefined}<span>{table.count}</span>{/if}
            </button>
          {/each}
        </nav>

        <div class="runtime-table-viewport">
          {#if selectedDevice && activeTable === "status"}
            <dl class="runtime-summary">
              <div><dt>CAM entries</dt><dd>{selectedDevice.macTable.length}</dd></div>
              <div><dt>Neighbors</dt><dd>{selectedDevice.neighbors.length}</dd></div>
              <div><dt>PAT translations</dt><dd>{selectedDevice.patTranslations.length}</dd></div>
              <div><dt>Firewall sessions</dt><dd>{selectedDevice.firewallSessions.length}</dd></div>
            </dl>
          {:else if selectedDevice && activeTable === "mac"}
            <table><thead><tr><th>VLAN</th><th>MAC address</th><th>Interface</th><th>TTL</th></tr></thead><tbody>
              {#each selectedDevice.macTable as entry}<tr><td>{entry.vlan}</td><td>{entry.macAddress}</td><td>{entry.interface}</td><td>{entry.remainingTtlMs} ms</td></tr>{/each}
            </tbody></table>
            {#if selectedDevice.macTable.length === 0}<p class="runtime-no-entries">No active entries.</p>{/if}
          {:else if selectedDevice && activeTable === "neighbors"}
            <table><thead><tr><th>Address</th><th>MAC address</th><th>Interface</th><th>State</th><th>TTL</th></tr></thead><tbody>
              {#each selectedDevice.neighbors as entry}<tr><td>{entry.address}</td><td>{entry.macAddress}</td><td>{entry.interface}</td><td>{entry.state}</td><td>{entry.remainingTtlMs} ms</td></tr>{/each}
            </tbody></table>
            {#if selectedDevice.neighbors.length === 0}<p class="runtime-no-entries">No active entries.</p>{/if}
          {:else if selectedDevice && activeTable === "pat"}
            <table><thead><tr><th>Protocol</th><th>Inside local</th><th>Inside global</th><th>Remote</th><th>TTL</th></tr></thead><tbody>
              {#each selectedDevice.patTranslations as entry}<tr><td>{entry.protocol}</td><td>{endpoint(entry.internalAddress, entry.internalToken)}</td><td>{endpoint(entry.externalAddress, entry.externalToken)}</td><td>{endpoint(entry.remoteAddress, entry.remotePort)}</td><td>{entry.remainingTtlMs} ms</td></tr>{/each}
            </tbody></table>
            {#if selectedDevice.patTranslations.length === 0}<p class="runtime-no-entries">No active entries.</p>{/if}
          {:else if selectedDevice && activeTable === "sessions"}
            <table><thead><tr><th>Protocol</th><th>Source</th><th>Destination</th><th>TTL</th></tr></thead><tbody>
              {#each selectedDevice.firewallSessions as entry}<tr><td>{entry.protocol}</td><td>{endpoint(entry.sourceAddress, entry.sourcePort)}</td><td>{endpoint(entry.destinationAddress, entry.destinationPort)}</td><td>{entry.remainingTtlMs} ms</td></tr>{/each}
            </tbody></table>
            {#if selectedDevice.firewallSessions.length === 0}<p class="runtime-no-entries">No active entries.</p>{/if}
          {/if}
        </div>

        <section class="runtime-console" aria-label="Simulator console">
          <header><strong>Simulator console</strong><span class={consoleStatus}>{consoleStatus}</span></header>
          <pre>{consoleOutput.join("\n")}</pre>
          <form onsubmit={(event) => { event.preventDefault(); void inspect(); }}>
            <label for="runtime-command">{selectedDevice?.id ?? "runtime"}#</label>
            <input id="runtime-command" bind:value={consoleCommand} autocomplete="off" spellcheck="false" disabled={busy} />
            <button type="submit" aria-label="Run runtime command" title="Run runtime command" disabled={busy || !consoleCommand.trim()}>
              {#if busy}<LoaderCircle class="spin" size={16} />{:else}<Play size={16} />{/if}
            </button>
          </form>
        </section>
      </div>
    </div>
  {/if}
</section>

<script lang="ts">
  import { Cable, GitFork, Network, Server, ShieldCheck } from "@lucide/svelte";
  import type {
    FirstHopRole,
    FirewallHaRole,
    ScenarioConnectionState,
    ScenarioFirstHopState,
    ScenarioFirewallHaState,
    ScenarioLinkAggregationState,
    ScenarioSpanningTreeState,
  } from "./simulation-api";

  interface SpanningTreeStateGroup
    extends Omit<ScenarioSpanningTreeState, "vlan"> {
    vlans: number[];
  }

  export let participants: string[] = [];
  export let connections: ScenarioConnectionState[] = [];
  export let firstHopStates: ScenarioFirstHopState[] = [];
  export let firewallHaStates: ScenarioFirewallHaState[] = [];
  export let linkAggregationStates: ScenarioLinkAggregationState[] = [];
  export let spanningTreeStates: ScenarioSpanningTreeState[] = [];
  export let disabled = false;
  export let onOpenAppliance: (id: string) => void = () => {};
  export let onConnectionChange: (id: string, operational: boolean) => void =
    () => {};
  export let onFirstHopChange: (
    appliance: string,
    port: string,
    role: FirstHopRole,
  ) => void = () => {};
  export let onFirewallHaChange: (
    appliance: string,
    role: FirewallHaRole,
  ) => void = () => {};

  $: downCount = connections.filter((connection) => !connection.operational).length;
  $: activeGatewayCount = firstHopStates.filter(
    (state) => state.role === "active",
  ).length;
  $: activeFirewallCount = firewallHaStates.filter(
    (state) => state.role === "active",
  ).length;
  $: discardingPortCount = spanningTreeStates.filter(
    (state) => state.state === "discarding",
  ).length;
  $: distributingMemberCount = linkAggregationStates.filter(
    (state) => state.distributing,
  ).length;
  $: orderedLinkAggregationStates = [...linkAggregationStates].sort(
    (left, right) =>
      Number(left.distributing) - Number(right.distributing) ||
      left.logical_id.localeCompare(right.logical_id) ||
      left.appliance.localeCompare(right.appliance) ||
      left.interface.localeCompare(right.interface),
  );
  $: orderedSpanningTreeStates = groupSpanningTreeStates(spanningTreeStates).sort((left, right) => {
    const rank = { disabled: 0, alternate: 1, root: 2, designated: 3 };
    return rank[left.role] - rank[right.role] ||
      left.appliance.localeCompare(right.appliance) ||
      left.interface.localeCompare(right.interface);
  });

  function groupSpanningTreeStates(
    states: ScenarioSpanningTreeState[],
  ): SpanningTreeStateGroup[] {
    const groups = new Map<string, SpanningTreeStateGroup>();
    for (const state of states) {
      const { vlan, ...shared } = state;
      const key = [
        state.appliance,
        state.interface,
        state.connection,
        state.protocol,
        state.root_bridge,
        state.root_path_cost,
        state.port_path_cost,
        state.role,
        state.state,
      ].join(":");
      const group = groups.get(key);
      if (group) {
        group.vlans.push(vlan);
      } else {
        groups.set(key, { ...shared, vlans: [vlan] });
      }
    }
    return [...groups.values()].map((group) => ({
      ...group,
      vlans: group.vlans.sort((left, right) => left - right),
    }));
  }
</script>

<section class="participant-list">
  <header>
    <span>Execution topology</span>
    <strong>{participants.length} appliances</strong>
  </header>
  <div>
    {#each participants as participant, index}
      <button
        type="button"
        title={`Open ${participant} configuration`}
        onclick={() => onOpenAppliance(participant)}
      >
        <Server size={14} strokeWidth={1.8} />
        <span>{participant}</span>
      </button>
      {#if index < participants.length - 1}
        <i aria-hidden="true"></i>
      {/if}
    {/each}
  </div>
</section>

{#if firstHopStates.length > 0}
  <section class="first-hop-state-editor">
    <details open>
      <summary>
        <span><ShieldCheck size={14} strokeWidth={1.8} />First-hop gateway</span>
        <strong>{activeGatewayCount} active / {firstHopStates.length} members</strong>
      </summary>
      <div class="first-hop-state-list">
        {#each firstHopStates as state (`${state.appliance}:${state.interface}`)}
          <article class:active={state.role === "active"}>
            <span class="connection-state-copy">
              <strong>{state.appliance} / {state.interface}</strong>
              <small>{state.protocol.toUpperCase()} {state.group} / {state.virtual_ip} / priority {state.priority}</small>
            </span>
            <span class="first-hop-role" role="group" aria-label={`Set ${state.appliance} ${state.interface} role`}>
              <button
                type="button"
                class:active={state.role === "active"}
                aria-pressed={state.role === "active"}
                {disabled}
                onclick={() => onFirstHopChange(state.appliance, state.interface, "active")}
              >Active</button>
              <button
                type="button"
                class:active={state.role === "standby"}
                aria-pressed={state.role === "standby"}
                {disabled}
                onclick={() => onFirstHopChange(state.appliance, state.interface, "standby")}
              >Standby</button>
            </span>
          </article>
        {/each}
      </div>
    </details>
  </section>
{/if}

{#if firewallHaStates.length > 0}
  <section class="firewall-ha-state">
    <details open>
      <summary>
        <span><ShieldCheck size={14} strokeWidth={1.8} />Firewall HA</span>
        <strong>{activeFirewallCount} active / {firewallHaStates.length} members</strong>
      </summary>
      <div class="firewall-ha-state-list">
        {#each firewallHaStates as state (state.appliance)}
          <article class:active={state.role === "active"} class:sync-down={!state.sync_operational}>
            <span class="connection-state-copy">
              <strong>{state.appliance} / {state.domain}</strong>
              <small>peer {state.peer} / sync {state.sync_operational ? "up" : "down"} / sessions {state.session_sync ? "enabled" : "disabled"}</small>
            </span>
            <span class="first-hop-role" role="group" aria-label={`Set ${state.appliance} firewall HA role`}>
              <button
                type="button"
                class:active={state.role === "active"}
                aria-pressed={state.role === "active"}
                {disabled}
                onclick={() => onFirewallHaChange(state.appliance, "active")}
              >Active</button>
              <button
                type="button"
                class:active={state.role === "standby"}
                aria-pressed={state.role === "standby"}
                {disabled}
                onclick={() => onFirewallHaChange(state.appliance, "standby")}
              >Standby</button>
            </span>
          </article>
        {/each}
      </div>
    </details>
  </section>
{/if}

{#if linkAggregationStates.length > 0}
  <section class="link-aggregation-state">
    <details open>
      <summary>
        <span><Network size={14} strokeWidth={1.8} />Link aggregation</span>
        <strong>{distributingMemberCount} distributing / {linkAggregationStates.length} members</strong>
      </summary>
      <div class="link-aggregation-state-list">
        {#each orderedLinkAggregationStates as state (`${state.appliance}:${state.interface}:${state.connection}`)}
          <article class:inactive={!state.distributing}>
            <span class="connection-state-copy">
              <strong>{state.appliance} / {state.interface}</strong>
              <small>{state.logical_id} / {state.protocol.toUpperCase()} {state.mode} / {state.active_members} of {state.minimum_active_members} required</small>
            </span>
            <span class="link-aggregation-status">
              <strong>{state.distributing ? "distributing" : state.selected ? "standby" : "down"}</strong>
              <small>{state.multi_chassis_domain ? `${state.multi_chassis_domain} / peer ${state.peer_forwarding ? "active" : "inactive"}` : state.system_id}</small>
            </span>
          </article>
        {/each}
      </div>
    </details>
  </section>
{/if}

{#if spanningTreeStates.length > 0}
  <section class="spanning-tree-state">
    <details open>
      <summary>
        <span><GitFork size={14} strokeWidth={1.8} />Spanning tree</span>
        <strong>{discardingPortCount} discarding / {spanningTreeStates.length} states</strong>
      </summary>
      <div class="spanning-tree-state-list">
        {#each orderedSpanningTreeStates as state (`${state.appliance}:${state.interface}:${state.role}:${state.vlans.join("-")}`)}
          <article class:discarding={state.state === "discarding"}>
            <span class="connection-state-copy">
              <strong>{state.appliance} / {state.interface}</strong>
              <small>{state.vlans.length === 1 ? "VLAN" : "VLANs"} {state.vlans.join(", ")} / {state.role} / root {state.root_bridge}</small>
            </span>
            <span class="spanning-tree-status">
              <strong>{state.state}</strong>
              <small>cost {state.root_path_cost}</small>
            </span>
          </article>
        {/each}
      </div>
    </details>
  </section>
{/if}

<section class:has-fault={downCount > 0} class="connection-state-editor">
  <details open={downCount > 0}>
    <summary>
      <span><Cable size={14} strokeWidth={1.8} />Connection state</span>
      <strong>{downCount} down / {connections.length} links</strong>
    </summary>
    <div class="connection-state-list">
      {#each connections as connection (connection.id)}
        <label class:down={!connection.operational}>
          <span class="connection-state-copy">
            <strong>{connection.label}</strong>
            <small>{connection.endpoint_a} to {connection.endpoint_b}</small>
          </span>
          <span class="connection-state-toggle">
            <input
              type="checkbox"
              role="switch"
              aria-label={`Set ${connection.id} operational`}
              checked={connection.operational}
              {disabled}
              onchange={(event) =>
                onConnectionChange(connection.id, event.currentTarget.checked)}
            />
            <i aria-hidden="true"></i>
            <strong>{connection.operational ? "Up" : "Down"}</strong>
          </span>
        </label>
      {/each}
    </div>
  </details>
</section>

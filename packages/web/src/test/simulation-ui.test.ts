import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import ExecutionTopology from "../lib/simulation/ExecutionTopology.svelte";
import PacketComposer from "../lib/simulation/PacketComposer.svelte";
import SimulationTrace from "../lib/simulation/SimulationTrace.svelte";
import SimulationWorkspace from "../lib/simulation/SimulationWorkspace.svelte";
import {
  applyScenarioRecovery,
  isScenarioRecoveryApplied,
  transitionFirewallHaRole,
} from "../lib/simulation/simulation-state";
import type { ScenarioPacket } from "../lib/simulation/simulation-api";
import { scenarioReport, scenarioSummary } from "./fixtures";

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("simulation state contracts", () => {
  it("applies recovery and performs coherent firewall role transitions", () => {
    const scenario = scenarioSummary();
    const current = {
      connectionStates: scenario.connection_states.map((state) => ({ ...state, operational: false })),
      firstHopStates: scenario.first_hop_states.map((state) => ({ ...state, role: "standby" as const })),
      firewallHaStates: scenario.firewall_ha_states.map((state) => ({ ...state, role: "standby" as const, sync_operational: false })),
    };
    expect(isScenarioRecoveryApplied(null, current)).toBe(false);
    expect(isScenarioRecoveryApplied(scenario.recovery, current)).toBe(false);
    const recovered = applyScenarioRecovery(scenario.recovery!, current);
    expect(isScenarioRecoveryApplied(scenario.recovery, recovered)).toBe(true);
    expect(transitionFirewallHaRole(recovered.firewallHaStates, recovered.firstHopStates, "missing", "active")).toBeNull();
    const transitioned = transitionFirewallHaRole(recovered.firewallHaStates, recovered.firstHopStates, "firewall-b", "active");
    expect(transitioned?.firewallHaStates.find((state) => state.appliance === "firewall-b")?.role).toBe("active");
  });
});

describe("simulation controls", () => {
  it("renders every supported packet contract", async () => {
    const packets: ScenarioPacket[] = [
      scenarioSummary().packet,
      { source_ip: "1.1.1.1", destination_ip: "2.2.2.2", ttl: 32, wire_length_bytes: 80, transport: { protocol: "udp", source_port: 10, destination_port: 53 }, application: { kind: "dns-query", name: "shop.example" } },
      { source_ip: "1.1.1.1", destination_ip: "2.2.2.2", ttl: 32, wire_length_bytes: 80, transport: { protocol: "icmp-echo", identifier: 1, sequence: 2 }, application: { kind: "none" } },
      { source_ip: "1.1.1.1", destination_ip: "2.2.2.2", ttl: 32, wire_length_bytes: 80, transport: { protocol: "udp", source_port: 10, destination_port: 20 }, application: { kind: "telemetry", service: "historian", source: "plc", sequence: 1, payload: "value=4" } },
      { source_ip: "1.1.1.1", destination_ip: "2.2.2.2", ttl: 32, wire_length_bytes: 80, transport: { protocol: "tcp", source_port: 10, destination_port: 20, syn: true, ack: false, fin: false, rst: false }, application: { kind: "service", service: "https" } },
    ];
    for (const packet of packets) {
      const submitted = vi.fn();
      const view = render(PacketComposer, { packet, onSubmit: submitted });
      expect(view.container.querySelector("fieldset")).not.toBeNull();
      const body = screen.queryByText("Request body")?.closest("label")?.querySelector("textarea");
      if (body) await fireEvent.input(body, { target: { value: "payload" } });
      await fireEvent.submit(view.container.querySelector("form")!);
      expect(submitted).toHaveBeenCalledOnce();
      view.unmount();
    }
  });

  it("edits topology roles, connection state, and appliance navigation", async () => {
    const scenario = scenarioSummary();
    const callbacks = {
      onOpenAppliance: vi.fn(),
      onConnectionChange: vi.fn(),
      onFirstHopChange: vi.fn(),
      onFirewallHaChange: vi.fn(),
    };
    render(ExecutionTopology, {
      participants: scenario.participants,
      connections: scenario.connection_states,
      firstHopStates: scenario.first_hop_states,
      firewallHaStates: scenario.firewall_ha_states,
      linkAggregationStates: scenario.link_aggregation_states,
      spanningTreeStates: scenario.spanning_tree_states,
      disabled: false,
      ...callbacks,
    });
    await fireEvent.click(screen.getByRole("button", { name: "customer-pc-01" }));
    await fireEvent.click(screen.getByRole("group", { name: /edge-router outside role/i }).querySelectorAll("button")[1]);
    await fireEvent.click(screen.getByRole("group", { name: /firewall-b firewall ha role/i }).querySelectorAll("button")[0]);
    await fireEvent.click(screen.getByRole("switch", { name: /set wan operational/i }));
    expect(callbacks.onOpenAppliance).toHaveBeenCalledWith("customer-pc-01");
    expect(callbacks.onConnectionChange).toHaveBeenCalledWith("wan", false);
  });

  it("renders trace filters and every special report section", async () => {
    const base = scenarioReport();
    const report = {
      ...base,
      expectation_mode: "continuity" as const,
      continuity: { failed_appliance: "firewall-a", promoted_appliance: "firewall-b", failure_at_us: 500, last_heartbeat_us: 400, promotion_at_us: 1_000, interruption_us: 500, synchronized_sessions: 2, sessions_after_continuation: 2, replicated_updates: 3, sync_operational_at_failure: false, faults: [{ type: "sync-link-loss" as const, at_us: 450 }], continuation_expectation_met: true },
      ha_isolation: { active_appliance: "firewall-a", standby_appliance: "firewall-b", isolation_at_us: 500, last_heartbeat_us: 400, evaluation_at_us: 900, promotion_inhibited_at_us: 1_000, active_members: 1, standby_sessions: 2, sync_operational: false, peer_failure_confirmed: false, continuation_expectation_met: true },
      local_autonomy: { hmi: "hmi", controller: "plc", remote_io: "rio", safety_interface: "safety", actuator: "valve", command_tag: "valve-command", command_value: "open", expected_actuator_state: "open", actuator_state: "open", outage_connections: ["wan"], local_path_connections: ["local"], local_path_operational: true, safety_reset_applied: true, command_applied: true, northbound_expectation_met: true, autonomy_expectation_met: true, control_trace: [{ sequence: 0, component: "plc", stage: "scan", detail: "applied" }] },
    };
    const open = vi.fn();
    render(SimulationTrace, { report, expectation: report.expectation, onOpenAppliance: open });
    for (const filter of ["network", "media", "drops", "all"]) await fireEvent.click(screen.getByRole("button", { name: filter }));
    await fireEvent.click(screen.getAllByRole("button", { name: "plc" })[0]);
    expect(open).toHaveBeenCalledWith("plc");
  });

  it("loads, edits, recovers, executes, resets, and selects a scenario", async () => {
    const scenario = scenarioSummary();
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      const endpoint = String(input);
      if (endpoint.endsWith("/api/simulations")) return json({ schema_version: "0.15.0", scenarios: [scenario, { ...scenario, id: "second", label: "Second scenario", recovery: null }] });
      if (endpoint.includes("/run")) return json(scenarioReport());
      return json({ error: "missing" }, 404);
    }));
    const view = render(SimulationWorkspace);
    await waitFor(() => expect(screen.getAllByText("Customer web", { exact: true }).length).toBeGreaterThan(0));
    await fireEvent.click(screen.getByRole("switch", { name: /set wan operational/i }));
    await fireEvent.click(screen.getByRole("button", { name: /recover/i }));
    await fireEvent.click(screen.getByRole("button", { name: /run scenario/i }));
    await waitFor(() => expect(view.container.querySelector(".trace-list article")).not.toBeNull());
    await fireEvent.click(screen.getByRole("button", { name: /reset packet/i }));
    await fireEvent.click(screen.getByRole("button", { name: /second scenario/i }));
    expect(screen.getAllByText("Second scenario", { exact: true }).length).toBeGreaterThan(0);
  });
});

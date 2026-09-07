import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import HmiView from "../lib/process/hmi/HmiView.svelte";
import SecurityConsoleView from "../lib/office/SecurityConsoleView.svelte";
import WorkstationView from "../lib/workstation/WorkstationView.svelte";
import {
  hmiSnapshot,
  scenarioReport,
  securitySession,
  workstationProfile,
  workstationReport,
} from "./fixtures";

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function endpointId(endpoint: string, segment: string) {
  return decodeURIComponent(endpoint.split(segment)[1]?.split("/")[0] ?? "unknown");
}

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", class {
    observe() {}
    unobserve() {}
    disconnect() {}
  });
  vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const endpoint = String(input);
    if (endpoint.includes("/api/hmis/")) {
      const id = endpointId(endpoint, "/api/hmis/");
      const snapshot = hmiSnapshot(id);
      if (endpoint.endsWith("/program")) {
        return json({ schemaVersion: "0.1.0", controller: snapshot.controller, language: "structured-text", program: "FORMING", task: "cyclic", sourcePath: "forming.st", bindingPath: "forming.yaml", revision: "test", source: "PROGRAM FORMING\nEND_PROGRAM", bindingYaml: "schema_version: 0.1.0" });
      }
      if (endpoint.endsWith("/historian")) {
        const latest = { source: id, sequence: 1, capturedAtMs: 100, phase: "pressure", cycle: 1, payload: "pressure=4.5", wireLengthBytes: 64 };
        return json({ schemaVersion: "0.1.0", sampleIntervalMs: 1_000, local: { applianceId: id, storedRecords: 1, capacity: 64, latest }, replica: { applianceId: "historian-replica", storedRecords: 1, capacity: 64, latest }, pendingRecords: 0, droppedUnreplicated: 0, replicationAttempts: 1, lastError: null, lastCollection: scenarioReport(), lastReplication: scenarioReport(), lastPublication: null });
      }
      if (endpoint.endsWith("/telemetry")) return json(scenarioReport());
      if (endpoint.endsWith("/actions")) {
        return json({ schemaVersion: "0.1.0", status: "applied", message: "Command applied", trace: [{ sequence: 0, component: snapshot.controller, stage: "command", detail: "accepted" }], snapshot });
      }
      return json(snapshot);
    }
    if (endpoint.includes("/api/workstations/")) {
      const id = endpointId(endpoint, "/api/workstations/");
      return json(endpoint.endsWith("/actions") ? workstationReport(id) : workstationProfile(id));
    }
    if (endpoint.includes("/api/security/events/")) {
      return json({ ...securitySession().events[0], acknowledged: true });
    }
    if (endpoint.includes("/api/security/consoles/")) {
      const id = endpointId(endpoint, "/api/security/consoles/");
      return json(endpoint.endsWith("/clear")
        ? { ...securitySession(id), activeCount: 0, events: [] }
        : securitySession(id));
    }
    return json({ error: `unhandled ${init?.method ?? "GET"} ${endpoint}` }, 404);
  }));
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

async function openHmi(id: string) {
  const view = render(HmiView, { applianceId: id });
  await waitFor(() => expect(view.container.querySelector(".hmi-runtime-header")).not.toBeNull());
  return view;
}

async function clickButton(name: RegExp | string) {
  const candidates = screen.getAllByRole("button", { name });
  const button = candidates.find((candidate) => !(candidate as HTMLButtonElement).disabled) ?? candidates[0];
  await fireEvent.click(button);
}

describe("operator interfaces", () => {
  it("exercises the independent body-preparation workspaces and handoff controls", async () => {
    for (const id of [
      "area-01-hmi-01",
      "area-01-gl-hmi-01",
      "area-01-wt-hmi-01",
      "area-01-wd-hmi-01",
      "area-01-rw-hmi-01",
      "area-01-rc-hmi-01",
    ]) {
      const view = await openHmi(id);
      for (const page of [/process/i, /handoffs/i, /recipes/i, /diagnostics/i, /routes/i, /pumps/i]) {
        const button = screen.queryAllByRole("button", { name: page })[0];
        if (button) await fireEvent.click(button);
      }
      expect(view.container.querySelector(".body-workspace")).not.toBeNull();
      view.unmount();
    }
  }, 30_000);

  it("exercises mould station state, commands, alarms, and control-source inspection", async () => {
    const view = await openHmi("area-02-hmi-01");
    await clickButton(/manual/i);
    await clickButton(/open/i);
    await clickButton(/acknowledge cell-001/i);
    await clickButton(/view executing control source/i);
    await waitFor(() => expect(screen.getByText("PROGRAM FORMING", { exact: false })).not.toBeNull());
    await clickButton(/close/i);
    expect(view.container.querySelector(".hmi-cycle-section")).not.toBeNull();
  });

  it("exercises robot status, jog, and authoritative program controls", async () => {
    const view = await openHmi("area-02-joystick-01");
    expect(view.container.querySelector(".robot-pendant")).not.toBeNull();
    await clickButton(/^jog$/i);
    await clickButton(/^program$/i);
    await clickButton(/step robot program/i);
    await clickButton(/reset robot program/i);
    expect(screen.getByLabelText("Robot G program source")).not.toBeNull();
  });

  it("exercises each machine-PC asset and supervisory page", async () => {
    const view = await openHmi("area-02-machine-pc-01");
    for (const page of [/mould 1/i, "Slip tank", "Production", "Cell safety", "System", "Trends", "Logs"]) {
      await clickButton(page);
    }
    await waitFor(() => expect(view.container.querySelector(".machine-pc-logs")).not.toBeNull());
    await clickButton(/acknowledge cell-001/i);
  });

  it("renders generic process HMIs without forming-only authority", async () => {
    const view = await openHmi("area-03-hmi-01");
    expect(view.container.querySelector(".hmi-signal-grid")).not.toBeNull();
    expect(screen.queryByText("Mould production")).toBeNull();
  });
});

describe("endpoint and security applications", () => {
  it("runs browser, terminal, and runtime network workflows", async () => {
    const view = render(WorkstationView, { applianceId: "customer-pc-01" });
    await waitFor(() => expect(view.container.querySelector(".desktop-dock")).not.toBeNull());
    await clickButton("Navigate");
    await waitFor(() => expect(screen.getByText("Catalog")).not.toBeNull());
    await clickButton(/open terminal/i);
    await fireEvent.input(screen.getByLabelText("Terminal command"), { target: { value: "show route" } });
    await clickButton(/run command/i);
    await clickButton(/open network state/i);
    for (const table of [/^CAM/i, /^Neighbors/i, /^PAT/i, /^Sessions/i]) await clickButton(table);
    await clickButton(/run runtime command/i);
    expect(view.container.querySelector(".runtime-workspace")).not.toBeNull();
  });

  it("filters, acknowledges, refreshes, and clears security evidence", async () => {
    const view = render(SecurityConsoleView, { applianceId: "operations-soc-console-01" });
    await waitFor(() => expect(view.container.querySelector(".security-investigation")).not.toBeNull());
    await clickButton(/^Active/i);
    await clickButton("Acknowledge");
    await clickButton(/refresh events/i);
    await waitFor(() => {
      const clear = screen.getByRole("button", { name: /clear event queue/i });
      expect((clear as HTMLButtonElement).disabled).toBe(false);
    });
    await clickButton(/clear event queue/i);
    await waitFor(() => expect(view.container.textContent).toContain("Queue clear"));
  });
});

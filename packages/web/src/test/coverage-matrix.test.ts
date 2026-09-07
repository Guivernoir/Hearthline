import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import GlazePanel from "../lib/process/hmi/body-preparation/panels/GlazePanel.svelte";
import HandoffPanel from "../lib/process/hmi/body-preparation/panels/HandoffPanel.svelte";
import QualityIoPanel from "../lib/process/hmi/body-preparation/panels/QualityIoPanel.svelte";
import RecipePanel from "../lib/process/hmi/body-preparation/panels/RecipePanel.svelte";
import SlipPanel from "../lib/process/hmi/body-preparation/panels/SlipPanel.svelte";
import WaterPanel from "../lib/process/hmi/body-preparation/panels/WaterPanel.svelte";
import HistorianPanel from "../lib/process/hmi/machine/supervisory/HistorianPanel.svelte";
import SupervisoryWorkspace from "../lib/process/hmi/machine/supervisory/SupervisoryWorkspace.svelte";
import { bodyPreparationFixture, hmiSnapshot, scenarioReport } from "./fixtures";

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("process presentation state matrix", () => {
  it("supports component defaults for embedded read-only process summaries", () => {
    const body = bodyPreparationFixture();
    const snapshot = hmiSnapshot("area-01-hmi-01");

    render(SlipPanel, { slip: body.slip }).unmount();
    render(GlazePanel, { glaze: body.glaze }).unmount();
    render(WaterPanel, { water: body.water, returns: body.returnWater }).unmount();
    render(HandoffPanel, { pipelines: body.pipelines, scope: "slip" }).unmount();
    render(QualityIoPanel, {
      snapshot,
      body,
      process: null,
      scope: "return-water-process",
    }).unmount();
    render(RecipePanel, { snapshot, body, scope: "return-water-process" }).unmount();
  });

  it("renders idle, held, pending, abnormal, and zero-target process states", async () => {
    const body = bodyPreparationFixture();
    const actions = vi.fn();
    body.slip.train.running = false;
    body.slip.train.held = true;
    body.slip.targetBatchMassKg = 0;
    body.slip.qualityReleased = false;
    body.slip.qualityChecks[0].withinLimit = false;
    body.glaze.train.running = false;
    body.glaze.targetPowderMassKg = 0;
    body.glaze.qualityReleased = false;
    body.glaze.qualityChecks[0].withinLimit = false;
    body.glaze.water.conductivityUsCm = 700;
    body.pipelines.slipToForming.leakDetected = true;
    body.pipelines.glazeToGlazing.leakDetected = true;

    const slipView = render(SlipPanel, {
      slip: body.slip,
      safetyTripped: false,
      busyTarget: "",
      onExecute: actions,
    });
    await fireEvent.click(screen.getByRole("button", { name: /resume/i }));
    expect(screen.getByText("Pending")).not.toBeNull();
    slipView.unmount();

    const glazeView = render(GlazePanel, {
      glaze: body.glaze,
      safetyTripped: true,
      busyTarget: "glaze-hold",
      onExecute: actions,
    });
    expect(screen.getByText("Pending")).not.toBeNull();
    glazeView.unmount();

    const waterView = render(WaterPanel, {
      water: body.water,
      returns: body.returnWater,
      safetyTripped: false,
      busyTarget: "",
      onExecute: actions,
    });
    expect(screen.getByText("Water quality")).not.toBeNull();
    waterView.unmount();
  });

  it("renders every handoff and diagnostics scope with active faults", async () => {
    const body = bodyPreparationFixture();
    const snapshot = hmiSnapshot("area-01-hmi-01");
    const actions = vi.fn();
    body.pipelines.waterToSlip.leakDetected = true;
    body.pipelines.waterToGlaze.leakDetected = true;
    body.pipelines.glazeToGlazing.leakDetected = true;
    body.pipelines.slipToForming.leakDetected = true;
    body.water.product.turbidityNtu = 4;
    body.returnWater.bodyReuseQuality.glazeContaminationPercent = 0.2;
    body.slip.qualityReleased = false;
    body.glaze.qualityReleased = false;

    for (const scope of ["slip", "water-pipeline", "glaze", "return-water-pipeline"] as const) {
      const view = render(HandoffPanel, { pipelines: body.pipelines, scope, compact: true });
      expect(view.container.querySelector(".body-handoff-panel")).not.toBeNull();
      view.unmount();
    }

    for (const scope of ["slip", "water-process", "glaze", "return-water-process"] as const) {
      const process = { ...snapshot.process!, fault: scope === "slip" ? "ingredient-shortage" as const : null };
      const view = render(QualityIoPanel, {
        snapshot,
        body,
        process,
        scope,
        busyTarget: scope === "slip" ? "fault-ingredient-shortage" : "",
        onExecute: actions,
      });
      const enabledFault = screen.queryAllByRole("button").find((button) => !(button as HTMLButtonElement).disabled);
      if (enabledFault) await fireEvent.click(enabledFault);
      view.unmount();
    }
  });

  it("unlocks recipe controls only after the local train is idle", async () => {
    const body = bodyPreparationFixture();
    const snapshot = hmiSnapshot("area-01-hmi-01");
    const actions = vi.fn();
    body.slip.train.running = false;
    body.slip.train.held = false;
    snapshot.activeRecipe = "alternate";

    const view = render(RecipePanel, {
      snapshot,
      body,
      scope: "slip",
      busyTarget: "",
      onExecute: actions,
    });
    await fireEvent.click(screen.getByRole("button", { name: /standard/i }));
    const input = screen.getByRole("spinbutton");
    await fireEvent.input(input, { target: { value: "5.2" } });
    await fireEvent.click(screen.getByRole("button", { name: /apply casting pressure/i }));
    expect(actions).toHaveBeenCalledTimes(2);
    view.unmount();
  });
});

describe("historian and supervisory alternate states", () => {
  it("publishes a failed transfer and exposes route-drop evidence", async () => {
    const snapshot = hmiSnapshot("area-02-machine-pc-01");
    const report = { ...scenarioReport(), expectation_met: false, status: "failed" as const };
    const latest = { source: "forming", sequence: 2, capturedAtMs: 200, phase: "pressure", cycle: 1, payload: "pressure=4.7", wireLengthBytes: 64 };
    vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
      const endpoint = String(input);
      if (endpoint.endsWith("/telemetry")) return json(report);
      return json({
        schemaVersion: "0.1.0",
        sampleIntervalMs: 1_000,
        local: { applianceId: "local", storedRecords: 4, capacity: 64, latest },
        replica: { applianceId: "replica", storedRecords: 3, capacity: 64, latest },
        pendingRecords: 1,
        droppedUnreplicated: 1,
        replicationAttempts: 3,
        lastError: null,
        lastCollection: report,
        lastReplication: null,
        lastPublication: null,
      });
    }));

    const view = render(HistorianPanel, { applianceId: "area-02-machine-pc-01" });
    await waitFor(() => expect((screen.getByRole("button", { name: /publish/i }) as HTMLButtonElement).disabled).toBe(false));
    await fireEvent.click(screen.getByRole("button", { name: /publish/i }));
    await waitFor(() => expect(screen.getByText("Publication failed")).not.toBeNull());
    view.unmount();

    const supervisory = structuredClone(snapshot.supervisory!);
    supervisory.repository.synchronized = true;
    supervisory.events = [];
    supervisory.tags[0].quality = "bad";
    supervisory.tags[0].samples = [];
    const supervisoryView = render(SupervisoryWorkspace, { supervisory });
    expect(screen.getByText("No event records")).not.toBeNull();
    supervisoryView.unmount();
  });

  it("reports malformed historian responses without hiding the status surface", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("bad gateway", { status: 503 })));
    const view = render(HistorianPanel, { applianceId: "offline-historian" });
    await waitFor(() => expect(view.container.textContent).toContain("HMI request failed (503)"));
    view.unmount();
  });
});

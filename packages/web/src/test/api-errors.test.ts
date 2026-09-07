import { afterEach, describe, expect, it, vi } from "vitest";
import { configurationApiAvailable, discardModelDraft, saveAppliance } from "../lib/config/config-api";
import { loadHmiSnapshot } from "../lib/process/hmi/hmi-api";
import { loadSecurityConsole } from "../lib/office/security-api";
import { loadSimulationCatalog } from "../lib/simulation/simulation-api";
import { loadWorkstationProfile } from "../lib/workstation/workstation-api";

function json(body: unknown, status: number) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

afterEach(() => vi.unstubAllGlobals());

describe("frontend API failure contracts", () => {
  it("preserves service errors and status fallbacks at every API boundary", async () => {
    for (const [operation, prefix] of [
      [() => loadHmiSnapshot("missing"), "HMI"],
      [() => loadSimulationCatalog(), "Simulation"],
      [() => loadWorkstationProfile("missing"), "Workstation"],
      [() => loadSecurityConsole("missing"), "Security console"],
    ] as const) {
      vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(json({ error: "specific failure" }, 422)));
      await expect(operation()).rejects.toThrow("specific failure");

      vi.stubGlobal("fetch", vi.fn().mockResolvedValueOnce(new Response("not json", { status: 503 })));
      await expect(operation()).rejects.toThrow(`${prefix} request failed (503)`);
    }
  });

  it("treats health failures as unavailable and rejects failed discard operations", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    await expect(configurationApiAvailable()).resolves.toBe(false);

    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(json({ status: "degraded", writeAccess: false }, 200)));
    await expect(configurationApiAvailable()).resolves.toBe(false);

    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("proxy failure", { status: 500 })));
    await expect(discardModelDraft(7)).rejects.toThrow("Model operation failed (500)");
  });

  it("rolls back a draft when preview diagnostics reject a direct save", async () => {
    const fetch = vi.fn()
      .mockResolvedValueOnce(json({ id: 12, baseRevision: "base" }, 200))
      .mockResolvedValueOnce(new Response(null, { status: 204 }))
      .mockResolvedValueOnce(json({
        candidateRevision: null,
        diagnostics: [{ severity: "error", message: "capacity evidence missing" }],
      }, 200))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetch);

    await expect(saveAppliance("config/appliances/test.yaml", "id: test\n"))
      .rejects.toThrow("capacity evidence missing");
    expect(fetch).toHaveBeenLastCalledWith("/api/model/drafts/12", {
      method: "DELETE",
      headers: { Accept: "application/json" },
    });
  });
});

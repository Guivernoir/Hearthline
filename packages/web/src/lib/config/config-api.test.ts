import { afterEach, describe, expect, it, vi } from "vitest";
import {
  commitModelDraft,
  configurationApiAvailable,
  createModelDraft,
  discardModelDraft,
  loadModelDocuments,
  previewModelDraft,
  updateModelDraft,
} from "./config-api";

function response(body: unknown, status = 200) {
  return new Response(status === 204 ? null : JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

afterEach(() => vi.unstubAllGlobals());

describe("model transaction API", () => {
  it("normalizes draft paths and preserves revision preconditions", async () => {
    const fetch = vi.fn()
      .mockResolvedValueOnce(response({ id: 7, baseRevision: "base" }))
      .mockResolvedValueOnce(response(undefined, 204))
      .mockResolvedValueOnce(response({ candidateRevision: "candidate", diagnostics: [] }))
      .mockResolvedValueOnce(response(undefined, 204));
    vi.stubGlobal("fetch", fetch);

    const draft = await createModelDraft();
    await updateModelDraft(draft.id, "/config/blueprints/cell.yaml", "id: cell\n");
    await previewModelDraft(draft.id);
    await commitModelDraft(draft, "Reviewed capacity evidence");

    expect(JSON.parse(String(fetch.mock.calls[1][1]?.body))).toEqual({
      path: "project/config/blueprints/cell.yaml",
      source: "id: cell\n",
    });
    expect(JSON.parse(String(fetch.mock.calls[3][1]?.body))).toEqual({
      expectedRevision: "base",
      reason: "Reviewed capacity evidence",
    });
  });

  it("reports health availability and structured API errors", async () => {
    const fetch = vi.fn()
      .mockResolvedValueOnce(response({ status: "ok", writeAccess: true }))
      .mockResolvedValueOnce(response({ error: "stale model revision" }, 409));
    vi.stubGlobal("fetch", fetch);
    expect(await configurationApiAvailable()).toBe(true);
    await expect(loadModelDocuments()).rejects.toThrow("stale model revision");
  });

  it("treats missing draft discard as idempotent", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(response({}, 404)));
    await expect(discardModelDraft(99)).resolves.toBeUndefined();
  });
});

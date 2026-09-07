import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import { tick } from "svelte";
import App from "../App.svelte";
import { getApplianceCatalog } from "../lib/config/appliance-config";
import { processView } from "../lib/process/process-model";

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function installApiStub() {
  Object.defineProperty(HTMLElement.prototype, "scrollTo", {
    configurable: true,
    value: vi.fn(),
  });
  Object.defineProperties(HTMLElement.prototype, {
    setPointerCapture: { configurable: true, value: vi.fn() },
    releasePointerCapture: { configurable: true, value: vi.fn() },
    hasPointerCapture: { configurable: true, value: vi.fn(() => true) },
  });
  vi.stubGlobal("ResizeObserver", class {
    observe() {}
    unobserve() {}
    disconnect() {}
  });
  vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
    const endpoint = String(input);
    if (endpoint.endsWith("/api/health")) {
      return json({ status: "ok", writeAccess: true });
    }
    if (endpoint.endsWith("/api/model/documents")) {
      return json({
        modelRevision: "test-revision",
        documents: [
          {
            kind: "blueprint",
            path: "project/config/blueprints/test.yaml",
            revision: "source-revision",
            source: [
              "schema_version: 0.1.0",
              "id: test-cell",
              "parameters: []",
              "nodes: []",
              "connections: []",
              "nested: []",
              "exports: []",
            ].join("\n"),
          },
        ],
      });
    }
    if (endpoint.endsWith("/api/model/drafts")) {
      return json({ id: 1, baseRevision: "test-revision" });
    }
    if (endpoint.endsWith("/api/simulations")) {
      return json({ schema_version: "0.15.0", scenarios: [] });
    }
    return json({ error: `unhandled test endpoint ${endpoint}` }, 404);
  }));
}

async function renderRoute(route: string) {
  window.history.replaceState(null, "", route ? `/#${route}` : "/");
  const view = render(App);
  await tick();
  await waitFor(() => expect(view.container.querySelector("main, .architecture-shell")).not.toBeNull());
  return view;
}

async function clickIfAvailable(name: RegExp) {
  const button = screen.queryAllByRole("button", { name })
    .find((candidate) => !(candidate as HTMLButtonElement).disabled);
  if (button) {
    await fireEvent.click(button);
    await tick();
  }
}

async function exerciseArchitectureSurface(container: HTMLElement) {
  for (const control of [
    /^zoom out$/i,
    /^zoom in$/i,
    /reset zoom/i,
    /^fit to view$/i,
    /^reset view$/i,
    /^toggle reference grid$/i,
    /^architecture minimap$/i,
  ]) {
    await clickIfAvailable(control);
  }

  const markers = Array.from(container.querySelectorAll<HTMLButtonElement>([
    'button[aria-label^="Inspect "]',
    'button[aria-label^="Select "]',
    'button[title^="Inspect "]',
    "button.environment-node",
    "button.site-environment-node",
  ].join(",")));
  const samples = markers.length > 1 ? [markers[0], markers.at(-1)!] : markers;
  for (const marker of samples) {
    await fireEvent.click(marker);
    await tick();
    await clickIfAvailable(/^close (device |location )?details$|^close inspector$/i);
  }

  const viewport = container.querySelector<HTMLElement>(".viewport");
  if (viewport) {
    await fireEvent.wheel(viewport, { deltaY: 100 });
    await fireEvent.wheel(viewport, { ctrlKey: true, deltaY: 100, clientX: 40, clientY: 50 });
    await fireEvent.wheel(viewport, { metaKey: true, deltaY: -100, clientX: 60, clientY: 70 });
    await fireEvent.pointerDown(viewport, { button: 1, pointerId: 7, clientX: 80, clientY: 90 });
    await fireEvent.pointerMove(viewport, { pointerId: 7, clientX: 100, clientY: 110 });
    await fireEvent.pointerUp(viewport, { pointerId: 7, clientX: 100, clientY: 110 });
    await fireEvent.pointerDown(viewport, { button: 0, pointerId: 8, clientX: 20, clientY: 20 });
  }

  for (const key of ["Space", "+", "-", "0", "f", "Escape"]) {
    await fireEvent.keyDown(window, { code: key === "Space" ? "Space" : undefined, key });
  }
  await fireEvent.keyUp(window, { code: "Space", key: " " });
  await tick();
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  window.history.replaceState(null, "", "/");
});

describe("catalog-driven application routes", () => {
  it("renders every architecture environment in logical and physical modes", async () => {
    installApiStub();
    const routes = [
      "",
      "customer",
      "customer/customer-lan",
      "customer/customer-edge",
      "customer/public-web-path",
      "office",
      "office/it-dmz",
      "office/business-it",
      "office/operations-intelligence",
      "factory",
      "factory/ot-dmz",
      "factory/process",
      "factory/process/body-preparation",
      "factory/process/body-preparation/slip",
      "factory/process/body-preparation/water",
      "factory/process/body-preparation/glaze",
      ...processView.areas
        .filter((area) => area.routeKey !== "body-preparation")
        .map((area) => `factory/process/${area.routeKey}`),
    ];

    for (const route of routes) {
      const view = await renderRoute(route);
      expect(view.container.textContent?.trim().length).toBeGreaterThan(20);
      const physical = screen.queryByRole("button", { name: /physical/i });
      if (physical) {
        await fireEvent.click(physical);
        await tick();
        expect(view.container.textContent?.trim().length).toBeGreaterThan(20);
      }
      view.unmount();
    }
  }, 60_000);

  it("renders the simulation and transactional model workspaces", async () => {
    installApiStub();
    let view = await renderRoute("simulations");
    await waitFor(() => expect(screen.getByText("No scenario selected")).not.toBeNull());
    view.unmount();

    view = await renderRoute("model/editor");
    await waitFor(() => expect(screen.getByLabelText("Model documents")).not.toBeNull());
    await waitFor(() => expect(screen.getByDisplayValue("test-cell")).not.toBeNull());
    view.unmount();
  });

  it("renders generated appliance and connection configuration details", async () => {
    installApiStub();
    const catalog = getApplianceCatalog();
    const appliance = catalog.appliances.find((item) => item.interfaces.length > 1)
      ?? catalog.appliances[0];
    const connection = catalog.connections[0];
    if (!appliance || !connection) throw new Error("generated catalog must not be empty");

    let view = await renderRoute(`config/appliances/${appliance.id}`);
    expect(screen.getByRole("heading", { level: 1, name: appliance.label })).not.toBeNull();
    view.unmount();

    view = await renderRoute(`config/connections/${connection.id}`);
    expect(screen.getByRole("heading", { level: 1, name: connection.label })).not.toBeNull();
    view.unmount();
  });

  it("operates representative architecture canvases and their inspectors", async () => {
    installApiStub();
    for (const route of [
      "",
      "customer",
      "customer/customer-lan",
      "customer/customer-edge",
      "office",
      "office/it-dmz",
      "office/operations-intelligence",
      "factory",
      "factory/process",
      "factory/process/forming",
      "factory/process/body-preparation/water",
    ]) {
      const view = await renderRoute(route);
      await exerciseArchitectureSurface(view.container);
      view.unmount();
    }
  }, 90_000);

  it("rejects unknown routes and preserves the regional entry point", async () => {
    installApiStub();
    const view = await renderRoute("unknown/route");
    expect(screen.getByText("Hearthline", { exact: true })).not.toBeNull();
    view.unmount();
  });
});

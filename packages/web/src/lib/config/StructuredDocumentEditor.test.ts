import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import StructuredDocumentEditor from "./StructuredDocumentEditor.svelte";

describe("structured blueprint editing", () => {
  it("adds and removes schema-owned node entries", async () => {
    const onChange = vi.fn();
    render(StructuredDocumentEditor, {
      kind: "blueprint",
      document: {
        schema_version: "0.1.0",
        id: "cell",
        parameters: [],
        nodes: [{ local_id: "controller", family: "virtual-controller", ports: ["control"] }],
        connections: [],
        nested: [],
        exports: [],
      },
      onChange,
    });

    await fireEvent.click(screen.getByRole("button", { name: "Add nodes entry" }));
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({
      nodes: [
        expect.objectContaining({ local_id: "controller" }),
        expect.objectContaining({ local_id: "component", family: "field-sensor" }),
      ],
    }));

    await fireEvent.click(screen.getByRole("button", { name: "Remove nodes entry" }));
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ nodes: [] }));
  });
});

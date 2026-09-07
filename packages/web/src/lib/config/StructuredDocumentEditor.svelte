<script lang="ts">
  import { Plus, Trash2 } from "@lucide/svelte";
  import type { ModelDocumentKind } from "../../generated/model-api";

  type DocumentValue = Record<string, unknown>;

  export let kind: ModelDocumentKind;
  export let document: DocumentValue;
  export let onChange: (document: DocumentValue) => void = () => {};

  $: entries = Object.entries(document);

  function replace(key: string, value: unknown) {
    onChange({ ...document, [key]: value });
  }

  function replaceArray(key: string, index: number, value: Record<string, unknown>) {
    const items = Array.isArray(document[key]) ? [...document[key] as unknown[]] : [];
    items[index] = value;
    replace(key, items);
  }

  function removeArrayItem(key: string, index: number) {
    const items = Array.isArray(document[key]) ? [...document[key] as unknown[]] : [];
    items.splice(index, 1);
    replace(key, items);
  }

  function addArrayItem(key: string) {
    const defaults: Record<string, Record<string, unknown>> = {
      imports: { blueprint: "", schema_version: "0.1.0", sha256: "" },
      parameters: { id: "parameter", kind: { type: "integer", minimum: 1, maximum: 8 }, required: true },
      nodes: { local_id: "component", family: "field-sensor", ports: [] },
      connections: { local_id: "connection", from_node: "", from_port: "", to_node: "", to_port: "", mode: "single" },
      nested: { local_id: "cell", blueprint: "", values: {} },
      exports: { id: "export", kind: "signal", node: "", port: "", contract: "" },
    };
    const items = Array.isArray(document[key]) ? [...document[key] as unknown[]] : [];
    items.push(defaults[key] ?? {});
    replace(key, items);
  }

  function updateObjectField(
    key: string,
    index: number,
    item: Record<string, unknown>,
    field: string,
    value: unknown,
  ) {
    replaceArray(key, index, { ...item, [field]: value });
  }

  function parseJson(value: string, fallback: unknown) {
    try {
      return JSON.parse(value) as unknown;
    } catch {
      return fallback;
    }
  }

  function scalarType(value: unknown) {
    if (typeof value === "boolean") return "boolean";
    if (typeof value === "number") return "number";
    return "text";
  }

  function label(value: string) {
    return value.replaceAll("_", " ").replaceAll("-", " ");
  }

  function objectValue(value: unknown): Record<string, unknown> {
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? value as Record<string, unknown>
      : {};
  }
</script>

<div class="structured-document" aria-label="Structured model document editor">
  {#each entries as [key, value] (key)}
    {#if kind === "blueprint" && Array.isArray(value) && ["imports", "parameters", "nodes", "connections", "nested", "exports"].includes(key)}
      <section class="structure-section">
        <header>
          <div><strong>{label(key)}</strong><span>{value.length}</span></div>
          <button type="button" aria-label={`Add ${label(key)} entry`} title={`Add ${label(key)} entry`} onclick={() => addArrayItem(key)}>
            <Plus size={15} />
          </button>
        </header>
        <div class="structure-rows">
          {#each value as rawItem, index (`${key}-${index}`)}
            {@const item = objectValue(rawItem)}
            <div class="structure-row">
              <div class="structure-row-fields">
                {#each Object.entries(item) as [field, fieldValue] (field)}
                  <label class:wide={typeof fieldValue === "object"}>
                    <span>{label(field)}</span>
                    {#if typeof fieldValue === "boolean"}
                      <input
                        type="checkbox"
                        checked={fieldValue}
                        onchange={(event) => updateObjectField(key, index, item, field, event.currentTarget.checked)}
                      />
                    {:else if field === "mode"}
                      <select value={String(fieldValue)} onchange={(event) => updateObjectField(key, index, item, field, event.currentTarget.value)}>
                        <option value="single">Single</option>
                        <option value="pairwise">Pairwise</option>
                        <option value="fan-out">Fan out</option>
                      </select>
                    {:else if field === "kind" && typeof fieldValue === "string"}
                      <select value={fieldValue} onchange={(event) => updateObjectField(key, index, item, field, event.currentTarget.value)}>
                        <option value="port">Port</option>
                        <option value="signal">Signal</option>
                        <option value="material-handoff">Material handoff</option>
                        <option value="network-conduit">Network conduit</option>
                      </select>
                    {:else if Array.isArray(fieldValue)}
                      <input
                        value={fieldValue.join(", ")}
                        oninput={(event) => updateObjectField(key, index, item, field, event.currentTarget.value.split(",").map((part) => part.trim()).filter(Boolean))}
                      />
                    {:else if fieldValue !== null && typeof fieldValue === "object"}
                      <textarea
                        rows="3"
                        value={JSON.stringify(fieldValue, null, 2)}
                        onblur={(event) => updateObjectField(key, index, item, field, parseJson(event.currentTarget.value, fieldValue))}
                      ></textarea>
                    {:else}
                      <input
                        type={scalarType(fieldValue)}
                        value={String(fieldValue ?? "")}
                        oninput={(event) => updateObjectField(key, index, item, field, typeof fieldValue === "number" ? Number(event.currentTarget.value) : event.currentTarget.value)}
                      />
                    {/if}
                  </label>
                {/each}
              </div>
              <button type="button" class="row-remove" aria-label={`Remove ${label(key)} entry`} title="Remove" onclick={() => removeArrayItem(key, index)}>
                <Trash2 size={14} />
              </button>
            </div>
          {/each}
        </div>
      </section>
    {:else if kind === "instance" && key === "values" && value !== null && typeof value === "object" && !Array.isArray(value)}
      <section class="structure-section">
        <header><div><strong>Parameter values</strong><span>{Object.keys(value).length}</span></div></header>
        <div class="instance-values">
          {#each Object.entries(value) as [parameter, parameterValue] (parameter)}
            <label>
              <span>{label(parameter)}</span>
              <input
                type={scalarType(parameterValue)}
                value={String(parameterValue)}
                oninput={(event) => replace("values", { ...objectValue(value), [parameter]: typeof parameterValue === "number" ? Number(event.currentTarget.value) : event.currentTarget.value })}
              />
            </label>
          {/each}
        </div>
      </section>
    {:else if value !== null && typeof value === "object"}
      <label class="document-field document-field-complex">
        <span>{label(key)}</span>
        <textarea
          rows={Math.min(14, Math.max(4, JSON.stringify(value, null, 2).split("\n").length))}
          value={JSON.stringify(value, null, 2)}
          onblur={(event) => replace(key, parseJson(event.currentTarget.value, value))}
        ></textarea>
      </label>
    {:else}
      <label class="document-field">
        <span>{label(key)}</span>
        {#if typeof value === "boolean"}
          <input type="checkbox" checked={value} onchange={(event) => replace(key, event.currentTarget.checked)} />
        {:else}
          <input
            type={scalarType(value)}
            value={String(value ?? "")}
            oninput={(event) => replace(key, typeof value === "number" ? Number(event.currentTarget.value) : event.currentTarget.value)}
          />
        {/if}
      </label>
    {/if}
  {/each}
</div>

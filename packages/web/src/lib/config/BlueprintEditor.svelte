<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    AlertTriangle,
    ArrowLeft,
    Braces,
    CheckCircle2,
    FileCode2,
    FilePlus2,
    GitCompareArrows,
    Network,
    PanelRightOpen,
    RefreshCw,
    Save,
    Trash2,
    X,
  } from "@lucide/svelte";
  import { parse, stringify } from "yaml";
  import type {
    DraftCreated,
    DraftPreview,
    ModelDocument,
    ModelDocumentKind,
  } from "../../generated/model-api";
  import {
    commitModelDraft,
    createModelDraft,
    discardModelDraft,
    loadModelDocuments,
    previewModelDraft,
    updateModelDraft,
  } from "./config-api";
  import StructuredDocumentEditor from "./StructuredDocumentEditor.svelte";

  export let onBack: () => void = () => {};

  const kinds: ModelDocumentKind[] = ["blueprint", "instance", "appliance", "connection", "scenario"];

  let documents: ModelDocument[] = [];
  let modelRevision = "";
  let draft: DraftCreated | null = null;
  let selectedPath = "";
  let selectedKind: ModelDocumentKind | "all" = "all";
  let tab: "structured" | "yaml" = "structured";
  let sourceOverrides: Record<string, string> = {};
  let removedPaths: string[] = [];
  let parsedDocument: Record<string, unknown> | null = null;
  let parseError = "";
  let preview: DraftPreview | null = null;
  let loading = true;
  let working = false;
  let error = "";
  let reason = "Update model through blueprint editor";
  let creating = false;
  let reviewOpen = false;
  let newKind: ModelDocumentKind = "blueprint";
  let newId = "";
  let disposed = false;

  $: visibleDocuments = documents.filter((document) => selectedKind === "all" || document.kind === selectedKind);
  $: selectedDocument = documents.find((document) => document.path === selectedPath) ?? null;
  $: activeSource = selectedDocument ? (sourceOverrides[selectedDocument.path] ?? selectedDocument.source) : "";
  $: hasChanges = Object.keys(sourceOverrides).length > 0 || removedPaths.length > 0;
  $: validationErrors = preview?.diagnostics.filter((diagnostic) => diagnostic.severity === "error") ?? [];
  $: canCommit = Boolean(preview?.candidateRevision) && validationErrors.length === 0 && hasChanges && reason.trim().length >= 8;

  onMount(async () => {
    await reload();
  });

  onDestroy(() => {
    disposed = true;
    if (draft) void discardModelDraft(draft.id).catch(() => undefined);
  });

  async function reload() {
    loading = true;
    error = "";
    try {
      const [catalog, created] = await Promise.all([loadModelDocuments(), createModelDraft()]);
      documents = catalog.documents;
      modelRevision = catalog.modelRevision;
      draft = created;
      selectedPath = documents[0]?.path ?? "";
      sourceOverrides = {};
      removedPaths = [];
      preview = null;
      parseSelected();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Unable to open model editor";
    } finally {
      loading = false;
    }
  }

  function selectDocument(path: string) {
    selectedPath = path;
    parseSelected();
  }

  function parseSelected() {
    parseError = "";
    const document = documents.find((candidate) => candidate.path === selectedPath);
    if (!document) {
      parsedDocument = null;
      return;
    }
    try {
      const value = parse(sourceOverrides[document.path] ?? document.source, { maxAliasCount: 50 });
      if (value === null || typeof value !== "object" || Array.isArray(value)) throw new Error("Document root must be a mapping");
      parsedDocument = value as Record<string, unknown>;
    } catch (cause) {
      parsedDocument = null;
      parseError = cause instanceof Error ? cause.message : "Invalid YAML";
    }
  }

  function updateRaw(source: string) {
    if (!selectedDocument) return;
    sourceOverrides = { ...sourceOverrides, [selectedDocument.path]: source };
    preview = null;
    parseSelected();
  }

  function updateStructured(value: Record<string, unknown>) {
    parsedDocument = value;
    updateRaw(stringify(value, { indent: 2, lineWidth: 0 }));
  }

  function restoreSelected() {
    if (!selectedDocument) return;
    const next = { ...sourceOverrides };
    delete next[selectedDocument.path];
    sourceOverrides = next;
    removedPaths = removedPaths.filter((path) => path !== selectedDocument?.path);
    preview = null;
    parseSelected();
  }

  function removeSelected() {
    if (!selectedDocument) return;
    removedPaths = [...new Set([...removedPaths, selectedDocument.path])];
    const next = { ...sourceOverrides };
    delete next[selectedDocument.path];
    sourceOverrides = next;
    preview = null;
  }

  function createDocument() {
    const id = newId.trim().toLowerCase();
    if (!/^[a-z0-9][a-z0-9-]{1,62}[a-z0-9]$/.test(id)) {
      error = "Document ID must use 3-64 lowercase letters, numbers, and hyphens";
      return;
    }
    const folder: Record<ModelDocumentKind, string> = {
      blueprint: "blueprints",
      instance: "instances",
      appliance: "appliances/editor",
      connection: "connections/editor",
      scenario: "scenarios/editor",
    };
    const path = `project/config/${folder[newKind]}/${id}.yaml`;
    if (documents.some((document) => document.path === path)) {
      error = `Document ${path} already exists`;
      return;
    }
    const skeletons: Record<ModelDocumentKind, Record<string, unknown>> = {
      blueprint: { schema_version: "0.1.0", id, parameters: [], nodes: [], connections: [], nested: [], exports: [] },
      instance: { schema_version: "0.1.0", id, blueprint: "", site: "factory", environment: "", values: {} },
      appliance: { schema_version: "1.0.0", id, kind: "sensor", label: id, site: "factory", environment: "", ports: [] },
      connection: { schema_version: "1.0.0", id, media: "copper-ethernet", endpoints: [] },
      scenario: { schema_version: "1.0.0", id, description: "", steps: [], expected: [] },
    };
    const source = stringify(skeletons[newKind], { indent: 2, lineWidth: 0 });
    const document: ModelDocument = { kind: newKind, path, revision: "new", source };
    documents = [...documents, document].sort((left, right) => left.path.localeCompare(right.path));
    sourceOverrides = { ...sourceOverrides, [path]: source };
    selectedPath = path;
    selectedKind = newKind;
    newId = "";
    creating = false;
    preview = null;
    parseSelected();
  }

  async function stageAndPreview() {
    if (!draft) return;
    working = true;
    error = "";
    try {
      for (const [path, source] of Object.entries(sourceOverrides)) {
        await updateModelDraft(draft.id, path, source);
      }
      for (const path of removedPaths) await updateModelDraft(draft.id, path, null);
      preview = await previewModelDraft(draft.id);
      reviewOpen = true;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Draft compilation failed";
    } finally {
      working = false;
    }
  }

  async function commit() {
    if (!draft || !canCommit) return;
    working = true;
    error = "";
    const committedDraft = draft;
    try {
      await commitModelDraft(committedDraft, reason.trim());
      draft = null;
      await reload();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Model commit failed";
    } finally {
      working = false;
    }
  }

  async function closeEditor() {
    if (draft) {
      const current = draft;
      draft = null;
      await discardModelDraft(current.id).catch(() => undefined);
    }
    if (!disposed) onBack();
  }
</script>

<svelte:head><title>Hearthline Model Editor</title></svelte:head>

<div class="app-shell model-editor-shell">
  <header class="topbar">
    <div class="brand-block">
      <button type="button" class="brand-back" aria-label="Return to architecture" title="Back" onclick={closeEditor}><ArrowLeft size={17} /></button>
      <span class="brand-mark" aria-hidden="true"><Braces size={19} /></span>
      <div class="brand-copy"><strong>Hearthline</strong><span>Model editor</span></div>
    </div>
    <div class="view-context"><span>Model</span><strong>{selectedDocument?.path ?? "Source documents"}</strong></div>
    <div class="toolbar">
      <button type="button" aria-label="Refresh model documents" title="Refresh" disabled={working} onclick={reload}><RefreshCw size={17} /></button>
      <button type="button" aria-label="Compile draft preview" title="Compile preview" disabled={!hasChanges || working || Boolean(parseError)} onclick={stageAndPreview}><GitCompareArrows size={17} /></button>
      <button type="button" class:active={canCommit} aria-label="Commit validated model draft" title="Commit" disabled={!canCommit || working} onclick={commit}><Save size={17} /></button>
      <button type="button" class="review-toggle" class:active={reviewOpen} aria-label="Toggle draft review" aria-pressed={reviewOpen} title="Draft review" onclick={() => (reviewOpen = !reviewOpen)}><PanelRightOpen size={17} /></button>
    </div>
  </header>

  <main class="model-editor-workspace">
    <aside class="model-document-browser" aria-label="Model documents">
      <div class="document-browser-head">
        <strong>Documents</strong>
        <button type="button" aria-label="Create model document" title="New document" onclick={() => (creating = !creating)}><FilePlus2 size={16} /></button>
      </div>
      <div class="document-kind-tabs" aria-label="Document types">
        <button class:active={selectedKind === "all"} onclick={() => (selectedKind = "all")}>All</button>
        {#each kinds as kind}
          <button class:active={selectedKind === kind} onclick={() => (selectedKind = kind)}>{kind}</button>
        {/each}
      </div>
      {#if creating}
        <div class="new-document-form">
          <select aria-label="New document type" bind:value={newKind}>{#each kinds as kind}<option value={kind}>{kind}</option>{/each}</select>
          <input aria-label="New document ID" placeholder="document-id" bind:value={newId} />
          <button type="button" onclick={createDocument}>Create</button>
        </div>
      {/if}
      <div class="document-list">
        {#each visibleDocuments as document (document.path)}
          <button
            type="button"
            class:active={document.path === selectedPath}
            class:modified={document.path in sourceOverrides}
            class:removed={removedPaths.includes(document.path)}
            onclick={() => selectDocument(document.path)}
          >
            <FileCode2 size={15} />
            <span><strong>{document.path.split("/").at(-1)}</strong><small>{document.path}</small></span>
          </button>
        {/each}
      </div>
    </aside>

    <section class="model-document-editor" aria-label="Document editor">
      {#if loading}
        <div class="editor-empty">Loading locked model revision</div>
      {:else if selectedDocument}
        <header class="document-editor-head">
          <div><span>{selectedDocument.kind}</span><strong>{selectedDocument.path.split("/").at(-1)}</strong><small>{selectedDocument.revision.slice(0, 12)}</small></div>
          <div class="document-editor-actions">
            <button type="button" aria-label="Restore source document" title="Restore" disabled={!(selectedDocument.path in sourceOverrides) && !removedPaths.includes(selectedDocument.path)} onclick={restoreSelected}><RefreshCw size={15} /></button>
            <button type="button" aria-label="Delete source document" title="Delete" onclick={removeSelected}><Trash2 size={15} /></button>
          </div>
        </header>
        <div class="editor-tabs" role="tablist">
          <button role="tab" aria-selected={tab === "structured"} class:active={tab === "structured"} onclick={() => (tab = "structured")}>Structured</button>
          <button role="tab" aria-selected={tab === "yaml"} class:active={tab === "yaml"} onclick={() => (tab = "yaml")}>YAML</button>
        </div>
        {#if removedPaths.includes(selectedDocument.path)}
          <div class="document-removed"><AlertTriangle size={17} />Document will be removed at commit.</div>
        {:else if tab === "structured"}
          {#if parsedDocument}
            <div class="editor-scroll"><StructuredDocumentEditor kind={selectedDocument.kind} document={parsedDocument} onChange={updateStructured} /></div>
          {:else}
            <div class="editor-empty error">{parseError}</div>
          {/if}
        {:else}
          <textarea class="raw-yaml-editor" aria-label="Raw YAML model document" spellcheck="false" value={activeSource} oninput={(event) => updateRaw(event.currentTarget.value)}></textarea>
        {/if}
      {:else}
        <div class="editor-empty">No editable model document selected</div>
      {/if}
    </section>

    <aside class:open={reviewOpen} class="model-draft-review" aria-label="Draft review">
      <header>
        <div><span>Revision</span><strong>{modelRevision.slice(0, 12) || "Unavailable"}</strong><small>{draft ? `Draft ${draft.id}` : "No draft"}</small></div>
        <button type="button" class="review-close" aria-label="Close draft review" title="Close draft review" onclick={() => (reviewOpen = false)}><X size={16} /></button>
      </header>
      {#if error}<div class="draft-message error" role="alert"><AlertTriangle size={15} />{error}</div>{/if}
      {#if parseError}<div class="draft-message error"><AlertTriangle size={15} />{parseError}</div>{/if}
      {#if preview}
        <section class="review-section">
          <h2>Topology</h2>
          <dl><div><dt>Added</dt><dd>{preview.addedObjects.length}</dd></div><div><dt>Modified</dt><dd>{preview.modifiedObjects.length}</dd></div><div><dt>Removed</dt><dd>{preview.removedObjects.length}</dd></div></dl>
          <div class="review-object-list">{#each [...preview.addedObjects, ...preview.modifiedObjects, ...preview.removedObjects].slice(0, 12) as object}<span>{object}</span>{/each}</div>
        </section>
        <section class="review-section">
          <h2>Capacity</h2>
          {#each preview.capacityDeltas as delta}
            <div class:rejected={delta.candidateStatus === "rejected"} class="capacity-delta"><span>{delta.resource}</span><strong>{delta.previousDemand ?? 0} -&gt; {delta.candidateDemand ?? 0}</strong></div>
          {:else}<div class="review-clear"><CheckCircle2 size={15} />No capacity delta</div>{/each}
        </section>
        <section class="review-section">
          <h2>Diagnostics</h2>
          {#each preview.diagnostics as diagnostic}
            <div class:error={diagnostic.severity === "error"} class="diagnostic"><strong>{diagnostic.severity}</strong><span>{diagnostic.path}{diagnostic.line ? `:${diagnostic.line}` : ""}</span><p>{diagnostic.message}</p></div>
          {:else}<div class="review-clear"><CheckCircle2 size={15} />Compilation passed</div>{/each}
        </section>
      {:else}
        <div class="review-pending"><Network size={24} /><span>{hasChanges ? "Compile the draft to review topology and capacity." : "Edit a source document to begin a draft."}</span></div>
      {/if}
      <div class="commit-reason"><label for="commit-reason">Review reason</label><textarea id="commit-reason" rows="3" bind:value={reason}></textarea></div>
    </aside>
  </main>

  <footer class="statusbar"><span class="status-state"><i></i>{working ? "Working" : "Transactional editor"}</span><span>{documents.length} documents / {Object.keys(sourceOverrides).length + removedPaths.length} changes</span><span>{preview?.candidateRevision ? "Validated" : "Draft"}</span></footer>
</div>

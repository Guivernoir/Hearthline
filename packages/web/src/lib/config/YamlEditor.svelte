<script lang="ts">
  import {
    Check,
    Clipboard,
    Pencil,
    Save,
    X,
  } from "@lucide/svelte";

  export let sourceYaml: string;
  export let schemaVersion: string;
  export let writable = false;
  export let saving = false;
  export let saveError = "";
  export let onSave: (sourceYaml: string) => Promise<boolean> = async () => false;

  let editing = false;
  let copied = false;
  let draft = sourceYaml;

  $: if (!editing) draft = sourceYaml;

  async function copyConfiguration() {
    try {
      await navigator.clipboard.writeText(editing ? draft : sourceYaml);
      copied = true;
      window.setTimeout(() => (copied = false), 1600);
    } catch {
      copied = false;
    }
  }

  function beginEditing() {
    draft = sourceYaml;
    editing = true;
  }

  function cancelEditing() {
    draft = sourceYaml;
    editing = false;
  }

  async function commit() {
    if (await onSave(draft)) editing = false;
  }
</script>

<article class:editing class="config-document">
  <div class="config-document-toolbar">
    <span>Complete source document</span>
    <span class="config-document-actions">
      <strong>YAML / schema {schemaVersion}</strong>
      <button
        type="button"
        aria-label="Copy complete YAML configuration"
        title="Copy complete YAML configuration"
        onclick={copyConfiguration}
      >
        {#if copied}
          <Check size={16} strokeWidth={1.9} />
        {:else}
          <Clipboard size={16} strokeWidth={1.9} />
        {/if}
      </button>
      {#if editing}
        <button
          type="button"
          aria-label="Discard YAML changes"
          title="Discard YAML changes"
          disabled={saving}
          onclick={cancelEditing}
        >
          <X size={16} strokeWidth={1.9} />
        </button>
        <button
          type="button"
          class="primary-action"
          aria-label="Validate and save YAML"
          title="Validate and save YAML"
          disabled={saving || draft === sourceYaml}
          onclick={commit}
        >
          <Save size={16} strokeWidth={1.9} />
        </button>
      {:else}
        <button
          type="button"
          aria-label="Edit YAML configuration"
          title={writable ? "Edit YAML configuration" : "Configuration API is offline"}
          disabled={!writable}
          onclick={beginEditing}
        >
          <Pencil size={16} strokeWidth={1.9} />
        </button>
      {/if}
    </span>
  </div>
  {#if saveError}
    <div class="config-save-error" role="alert">{saveError}</div>
  {/if}
  {#if editing}
    <textarea
      aria-label="YAML configuration editor"
      spellcheck="false"
      bind:value={draft}
    ></textarea>
  {:else}
    <pre><code>{sourceYaml}</code></pre>
  {/if}
</article>

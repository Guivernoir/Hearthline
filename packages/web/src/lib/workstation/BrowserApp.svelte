<script lang="ts">
  import {
    ArrowLeft,
    ArrowRight,
    Globe2,
    LoaderCircle,
    LockKeyhole,
    RefreshCw,
    Send,
    ShieldCheck,
    ShieldX,
  } from "@lucide/svelte";
  import {
    runWorkstationAction,
    type WorkstationActionReport,
  } from "./workstation-api";

  export let workstationId: string;
  export let initialUrl: string | null = null;
  export let onResult: (report: WorkstationActionReport) => void = () => {};

  let address = initialUrl ?? "";
  let history: string[] = [];
  let historyIndex = -1;
  let report: WorkstationActionReport | null = null;
  let busy = false;
  let error = "";

  async function navigate(target = address, recordHistory = true) {
    if (!target.trim() || busy) return;
    busy = true;
    error = "";
    try {
      report = await runWorkstationAction(workstationId, {
        kind: "browser",
        url: target.trim(),
      });
      address = report.browser?.url ?? target.trim();
      if (recordHistory) {
        history = [...history.slice(0, historyIndex + 1), address];
        historyIndex = history.length - 1;
      }
      onResult(report);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Browser request failed";
      report = null;
    } finally {
      busy = false;
    }
  }

  function moveHistory(offset: number) {
    const next = historyIndex + offset;
    if (next < 0 || next >= history.length) return;
    historyIndex = next;
    address = history[next];
    void navigate(address, false);
  }
</script>

<section class="browser-app" aria-label="Workstation browser">
  <header class="browser-toolbar">
    <div class="browser-navigation">
      <button type="button" aria-label="Back" title="Back" disabled={historyIndex === 0 || busy} onclick={() => moveHistory(-1)}>
        <ArrowLeft size={16} />
      </button>
      <button type="button" aria-label="Forward" title="Forward" disabled={historyIndex >= history.length - 1 || busy} onclick={() => moveHistory(1)}>
        <ArrowRight size={16} />
      </button>
      <button type="button" aria-label="Reload" title="Reload" disabled={busy} onclick={() => void navigate(address, false)}>
        <RefreshCw class={busy ? "spin" : ""} size={15} />
      </button>
    </div>
    <form class="browser-address" onsubmit={(event) => { event.preventDefault(); void navigate(); }}>
      <LockKeyhole size={14} strokeWidth={1.9} />
      <input bind:value={address} aria-label="Browser address" spellcheck="false" />
      <button type="submit" aria-label="Navigate" title="Navigate" disabled={busy || !address.trim()}>
        {#if busy}<LoaderCircle class="spin" size={16} />{:else}<Send size={16} />{/if}
      </button>
    </form>
  </header>

  <div class="browser-document" aria-live="polite">
    {#if busy}
      <div class="browser-state">
        <LoaderCircle class="spin" size={28} />
        <strong>Sending request</strong>
      </div>
    {:else if error}
      <div class="browser-state error">
        <ShieldX size={28} />
        <strong>Request failed</strong>
        <span>{error}</span>
      </div>
    {:else if report?.browser?.response?.document}
      <article class="browser-site">
        <header>
          <strong>{report.browser.response.document.title}</strong>
          <span><LockKeyhole size={13} />{report.browser.host}</span>
        </header>
        <main>
          <small>HTTP {report.browser.response.status}</small>
          <h1>{report.browser.response.document.heading}</h1>
          <p>{report.browser.response.document.body}</p>
        </main>
        <details>
          <summary>Connection details</summary>
          <dl>
            <div><dt>Resolved address</dt><dd>{report.browser.resolvedAddress ?? "No answer"}</dd></div>
            <div><dt>Resolution</dt><dd>{report.browser.resolutionSource}</dd></div>
            <div><dt>Application gateway</dt><dd>{report.browser.gateway ?? "Not reached"}</dd></div>
            <div><dt>Application service</dt><dd>{report.browser.forwardedTo ?? "Not reached"}</dd></div>
            <div><dt>Request body</dt><dd>{report.browser.requestBodyBytes} bytes</dd></div>
            <div><dt>Simulation runs</dt><dd>{report.simulations.length}</dd></div>
            <div><dt>Session ARP</dt><dd>{report.networkState.arpEntries.length} entries</dd></div>
            <div><dt>PAT translations</dt><dd>{report.networkState.patTranslations}</dd></div>
          </dl>
        </details>
      </article>
    {:else if report?.browser}
      <div class:denied={report.browser.outcome !== "responded"} class="browser-result">
        <span class="browser-result-icon">
          {#if report.browser.outcome === "responded"}
            <ShieldCheck size={28} />
          {:else}
            <ShieldX size={28} />
          {/if}
        </span>
        <div>
          <small>{report.browser.host}</small>
          <h1>{report.title}</h1>
          <p>{report.output[1] ?? report.output[0]}</p>
        </div>
        <dl>
          <div><dt>Resolved address</dt><dd>{report.browser.resolvedAddress ?? "No answer"}</dd></div>
          <div><dt>Resolution</dt><dd>{report.browser.resolutionSource}</dd></div>
          <div><dt>Application gateway</dt><dd>{report.browser.gateway ?? "Not reached"}</dd></div>
          <div><dt>Application service</dt><dd>{report.browser.forwardedTo ?? "Not reached"}</dd></div>
          <div><dt>Request body</dt><dd>{report.browser.requestBodyBytes} bytes</dd></div>
          <div><dt>Simulation runs</dt><dd>{report.simulations.length}</dd></div>
          <div><dt>Session ARP</dt><dd>{report.networkState.arpEntries.length} entries</dd></div>
          <div><dt>PAT translations</dt><dd>{report.networkState.patTranslations}</dd></div>
        </dl>
      </div>
    {:else}
      <div class="browser-state">
        <Globe2 size={30} />
        <strong>New tab</strong>
        {#if initialUrl}
          <button type="button" onclick={() => void navigate(initialUrl)}>
            <LockKeyhole size={15} />Open {initialUrl}
          </button>
        {/if}
      </div>
    {/if}
  </div>
</section>

<script lang="ts">
  import { onMount, tick } from "svelte";
  import { LoaderCircle, TerminalSquare } from "@lucide/svelte";
  import {
    runWorkstationAction,
    type WorkstationActionReport,
  } from "./workstation-api";

  interface TranscriptEntry {
    command: string;
    output: string[];
    status: WorkstationActionReport["status"];
  }

  export let workstationId: string;
  export let hostname: string;
  export let onResult: (report: WorkstationActionReport) => void = () => {};

  let command = "";
  let busy = false;
  let error = "";
  let history: string[] = [];
  let historyIndex = 0;
  let transcript: TranscriptEntry[] = [
    {
      command: "",
      output: ["Hearthline endpoint shell", "Run 'help' for available commands."],
      status: "completed",
    },
  ];
  let outputViewport: HTMLDivElement;
  let commandInput: HTMLInputElement;

  onMount(() => commandInput?.focus());

  async function execute() {
    const submitted = command.trim();
    if (!submitted || busy) return;
    command = "";
    error = "";
    busy = true;
    history = [...history, submitted];
    historyIndex = history.length;
    try {
      const report = await runWorkstationAction(workstationId, {
        kind: "terminal",
        command: submitted,
      });
      transcript = report.clearOutput
        ? []
        : [
            ...transcript,
            {
              command: submitted,
              output: report.output,
              status: report.status,
            },
          ];
      onResult(report);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Terminal action failed";
      transcript = [
        ...transcript,
        { command: submitted, output: [error], status: "failed" },
      ];
    } finally {
      busy = false;
      await tick();
      if (outputViewport) outputViewport.scrollTop = outputViewport.scrollHeight;
      commandInput?.focus();
    }
  }

  function handleHistory(event: KeyboardEvent) {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    event.preventDefault();
    if (event.key === "ArrowUp") {
      historyIndex = Math.max(0, historyIndex - 1);
    } else {
      historyIndex = Math.min(history.length, historyIndex + 1);
    }
    command = history[historyIndex] ?? "";
  }
</script>

<section class="terminal-app" aria-label="Workstation terminal">
  <header class="window-titlebar">
    <span><TerminalSquare size={15} strokeWidth={1.9} />Terminal</span>
    <small>{hostname}</small>
  </header>
  <div class="terminal-output" bind:this={outputViewport} aria-live="polite">
    {#each transcript as entry}
      <div class:terminal-error={entry.status === "failed" || entry.status === "denied"}>
        {#if entry.command}
          <div class="terminal-command">
            <span>{hostname}&gt;</span>{entry.command}
          </div>
        {/if}
        {#each entry.output as line}
          <pre>{line || " "}</pre>
        {/each}
      </div>
    {/each}
  </div>
  <form class="terminal-prompt" onsubmit={(event) => { event.preventDefault(); void execute(); }}>
    <label for="workstation-command">{hostname}&gt;</label>
    <input
      id="workstation-command"
      bind:this={commandInput}
      bind:value={command}
      onkeydown={handleHistory}
      autocomplete="off"
      spellcheck="false"
      aria-label="Terminal command"
      disabled={busy}
    />
    <button type="submit" aria-label="Run command" title="Run command" disabled={busy || !command.trim()}>
      {#if busy}
        <LoaderCircle class="spin" size={16} />
      {:else}
        <TerminalSquare size={16} />
      {/if}
    </button>
  </form>
</section>

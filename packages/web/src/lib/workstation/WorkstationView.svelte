<script lang="ts">
  import { onMount } from "svelte";
  import {
    ArrowLeft,
    FileText,
    Globe2,
    LoaderCircle,
    Monitor,
    Network,
    TerminalSquare,
    Wifi,
  } from "@lucide/svelte";
  import BrowserApp from "./BrowserApp.svelte";
  import TerminalApp from "./TerminalApp.svelte";
  import {
    loadWorkstationProfile,
    type WorkstationActionReport,
    type WorkstationProfile,
  } from "./workstation-api";

  type ActiveApplication = "browser" | "terminal";

  export let applianceId: string;
  export let onBack: () => void = () => {};
  export let onOpenConfig: (id: string) => void = () => {};

  let profile: WorkstationProfile | null = null;
  let activeApplication: ActiveApplication = "browser";
  let lastReport: WorkstationActionReport | null = null;
  let activityOpen = false;
  let loading = true;
  let error = "";

  onMount(async () => {
    try {
      profile = await loadWorkstationProfile(applianceId);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Cannot load workstation";
    } finally {
      loading = false;
    }
  });

  function receiveResult(report: WorkstationActionReport) {
    lastReport = report;
  }
</script>

<svelte:head>
  <title>{profile?.label ?? "Workstation"} | Hearthline</title>
</svelte:head>

<div class="app-shell workstation-shell">
  <header class="topbar">
    <div class="brand-block">
      <button type="button" class="brand-back" aria-label="Back to architecture" title="Back to architecture" onclick={onBack}>
        <ArrowLeft size={18} strokeWidth={1.9} />
      </button>
      <span class="brand-mark workstation-mark" aria-hidden="true"><Monitor size={20} /></span>
      <div class="brand-copy"><strong>{profile?.label ?? applianceId}</strong><span>Endpoint session</span></div>
    </div>
    <div class="view-context" aria-label="Current appliance">
      <span>{profile?.environment ?? "Endpoint"}</span><Network size={14} /><strong>{profile?.hostname ?? applianceId}</strong>
    </div>
    <div class="toolbar" aria-label="Workstation tools">
      <button type="button" aria-label="View configuration" title="View configuration" onclick={() => onOpenConfig(applianceId)}>
        <FileText size={17} />
      </button>
    </div>
  </header>

  <main class="workstation-desktop">
    {#if loading}
      <div class="workstation-loading"><LoaderCircle class="spin" size={28} /><span>Starting endpoint session</span></div>
    {:else if error || !profile}
      <div class="workstation-loading error"><Monitor size={28} /><strong>Endpoint unavailable</strong><span>{error}</span></div>
    {:else}
      <nav class="desktop-dock" aria-label="Workstation applications">
        <button class:active={activeApplication === "browser"} type="button" aria-label="Open browser" title="Browser" onclick={() => (activeApplication = "browser")}>
          <Globe2 size={22} /><span>Browser</span>
        </button>
        <button class:active={activeApplication === "terminal"} type="button" aria-label="Open terminal" title="Terminal" onclick={() => (activeApplication = "terminal")}>
          <TerminalSquare size={22} /><span>Terminal</span>
        </button>
        <button type="button" aria-label="Open appliance configuration" title="Configuration" onclick={() => onOpenConfig(applianceId)}>
          <FileText size={22} /><span>Config</span>
        </button>
      </nav>

      <section class="desktop-window">
        {#if activeApplication === "browser"}
          <BrowserApp workstationId={applianceId} initialUrl={profile.browserHome} onResult={receiveResult} />
        {:else}
          <TerminalApp workstationId={applianceId} hostname={profile.hostname} onResult={receiveResult} />
        {/if}
      </section>

      {#if lastReport}
        <aside class:open={activityOpen} class="network-activity">
          <button type="button" aria-expanded={activityOpen} onclick={() => (activityOpen = !activityOpen)}>
            <Wifi size={15} />
            <span>{lastReport.title}</span>
            <strong class={lastReport.status}>{lastReport.status}</strong>
          </button>
          {#if activityOpen}
            <div>
              {#each lastReport.simulations as simulation}
                <section>
                  <header><strong>{simulation.scenario_label}</strong><span>{simulation.duration_us} us</span></header>
                  <p>{simulation.statistics.events} events / {simulation.link_count} links / {simulation.statistics.drops} drops</p>
                  {#if simulation.packet.application.kind === "http-request"}
                    <p class="activity-request">
                      <strong>{simulation.packet.application.method.toUpperCase()}</strong>
                      <span>{simulation.packet.application.host}{simulation.packet.application.path}</span>
                      <small>{simulation.packet.application.body_bytes} body bytes</small>
                    </p>
                  {/if}
                  {#if simulation.security}
                    <div class={`workstation-security-evidence ${simulation.security.disposition}`}>
                      <strong>{simulation.security.severity} / {simulation.security.technique}</strong>
                      <span>{simulation.security.evidence}</span>
                    </div>
                  {/if}
                  <ol>
                    {#each simulation.trace as entry}
                      <li><time>{entry.time_us} us</time><strong>{entry.component}</strong><span>{entry.summary}</span></li>
                    {/each}
                  </ol>
                </section>
              {/each}
            </div>
          {/if}
        </aside>
      {/if}
    {/if}
  </main>

  <footer class="statusbar workstation-statusbar">
    <span class="status-state"><i></i>{profile?.hostname ?? "Endpoint session"}</span>
    <span>{profile?.interfaces[0]?.addresses[0] ?? "No address"}</span>
    <span>DNS {profile?.dnsServers[0] ?? "not configured"}</span>
  </footer>
</div>

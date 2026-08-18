<script lang="ts">
  import {
    Blocks,
    CheckCircle2,
    Database,
    GitCompareArrows,
    ServerCog,
    ShieldCheck,
  } from "@lucide/svelte";
  import type { HmiSupervisoryState, HmiSupervisoryTag } from "../../hmi-api";

  export let supervisory: HmiSupervisoryState;

  function display(value: string) {
    return value.replaceAll("-", " ");
  }

  function trendPoints(tag: HmiSupervisoryTag) {
    const samples = tag.samples.slice(-24);
    if (samples.length < 2) return "0,28 180,28";
    const values = samples.map((sample) => sample.value);
    const minimum = Math.min(...values);
    const maximum = Math.max(...values);
    const span = maximum - minimum || 1;
    return samples.map((sample, index) => {
      const x = index / (samples.length - 1) * 180;
      const y = 50 - (sample.value - minimum) / span * 42;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    }).join(" ");
  }
</script>

<div class="supervisory-workspace">
  <header class="supervisory-summary">
    <div><Blocks size={18} /><span><small>Object namespace</small><strong>{supervisory.namespace}</strong></span></div>
    <dl>
      <div><dt>Model</dt><dd>{supervisory.modelId}</dd></div>
      <div><dt>Revision</dt><dd>{supervisory.repository.deployedRevision}</dd></div>
      <div><dt>Session</dt><dd>{supervisory.identity.user}</dd></div>
      <div><dt>Role</dt><dd>{display(supervisory.identity.role)}</dd></div>
    </dl>
    <span class:synchronized={supervisory.repository.synchronized} class="deployment-state">
      <GitCompareArrows size={15} />{supervisory.repository.synchronized ? "Deployed" : "Revision pending"}
    </span>
  </header>

  <div class="supervisory-grid">
    <section class="supervisory-assets">
      <header><span><Blocks size={16} />Asset instances</span><strong>{supervisory.assets.length}</strong></header>
      {#each supervisory.assets as asset}
        <article class:child={asset.parent}>
          <i></i><div><strong>{asset.label}</strong><small>{asset.id}</small></div>
          <span>{display(asset.template)}</span><em>{asset.components.length} objects / {asset.historizedTags.length} tags</em>
        </article>
      {/each}
      <footer>{supervisory.templates.length} reusable templates / {supervisory.templates.filter((item) => item.alarmCapable).length} alarm capable</footer>
    </section>

    <section class="supervisory-deployment">
      <header><span><ServerCog size={16} />Deployment</span><strong>{supervisory.deploymentNodes.length} nodes</strong></header>
      {#each supervisory.deploymentNodes as node}
        <article>
          <span class:active={node.state === "active"} class:standby={node.state === "standby"}><ServerCog size={15} /></span>
          <div><strong>{node.label}</strong><small>{node.host}</small></div>
          <em>{display(node.role)}</em><b>{node.state}</b>
        </article>
      {/each}
      <footer><ShieldCheck size={14} />{display(supervisory.identity.authentication)} / {supervisory.identity.permissions.length} effective permissions</footer>
    </section>

    <section class="supervisory-tags">
      <header><span><Database size={16} />Live history tags</span><strong>{supervisory.tags.length}</strong></header>
      <div>
        {#each supervisory.tags as tag}
          <article>
            <header><span><i class:bad={tag.quality !== "good"}></i>{tag.tag}</span><strong>{tag.value.toFixed(1)} {display(tag.unit)}</strong></header>
            <svg viewBox="0 0 180 58" preserveAspectRatio="none" aria-label={`${tag.tag} recent trend`}><polyline points={trendPoints(tag)}></polyline></svg>
            <footer><span>{tag.quality.toUpperCase()}</span><em>{tag.samples.length} samples</em></footer>
          </article>
        {/each}
      </div>
    </section>

    <section class="supervisory-events">
      <header><span><CheckCircle2 size={16} />Alarms and events</span><strong>{supervisory.events.length}</strong></header>
      {#if supervisory.events.length}
        {#each [...supervisory.events].reverse().slice(0, 12) as event}
          <article><b>{event.sequence}</b><div><strong>{display(event.category)}</strong><span>{event.message}</span><small>{event.source}</small></div><em>{event.state}</em></article>
        {/each}
      {:else}
        <div class="supervisory-empty"><CheckCircle2 size={20} />No event records</div>
      {/if}
    </section>
  </div>
</div>

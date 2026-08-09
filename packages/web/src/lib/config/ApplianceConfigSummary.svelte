<script lang="ts">
  import { FileText, MonitorPlay } from "@lucide/svelte";
  import type { FrontendAppliance } from "./appliance-config";

  export let appliances: FrontendAppliance[] = [];
  export let onOpen: (id: string) => void = () => {};
  export let onOperate: ((id: string) => void) | null = null;

  function operationLabel(kind: string) {
    if (kind === "hmi") return "Open HMI";
    if (kind === "scada-workstation") return "Open SCADA";
    if (kind === "operations-console") return "Open SOC console";
    return "Open workstation";
  }
</script>

{#if appliances.length > 0}
  <section class="appliance-summary" aria-label="Parsed appliance configuration">
    <div class="appliance-summary-heading">
      <span>Parsed configuration</span>
      <strong>{appliances.length === 1 ? appliances[0].lifecycle : `${appliances.length} members`}</strong>
    </div>

    {#if appliances.length === 1}
      {@const appliance = appliances[0]}
      <p>{appliance.summary}</p>
      <dl>
        <div>
          <dt>Appliance kind</dt>
          <dd>{appliance.kind}</dd>
        </div>
        <div>
          <dt>Behavior</dt>
          <dd>{appliance.behaviorFamily}</dd>
        </div>
        <div>
          <dt>Interfaces</dt>
          <dd>{appliance.interfaceCount}</dd>
        </div>
      </dl>
      <div class="appliance-actions">
        {#if onOperate}
          <button type="button" class="appliance-open appliance-operate" onclick={() => onOperate?.(appliance.id)}>
            <MonitorPlay size={15} strokeWidth={1.9} />
            <span>{operationLabel(appliance.kind)}</span>
          </button>
        {/if}
        <button type="button" class="appliance-open" onclick={() => onOpen(appliance.id)}>
          <FileText size={15} strokeWidth={1.9} />
          <span>View full configuration</span>
        </button>
      </div>
    {:else}
      <p>Individually configured members represented by this diagram node.</p>
      <div class="appliance-member-list">
        {#each appliances as appliance}
          <div>
            <span>
              <strong>{appliance.label}</strong>
              <small>{appliance.kind} / {appliance.lifecycle}</small>
            </span>
            {#if onOperate && appliance.tags.includes("interactive")}
              <button
                type="button"
                aria-label={`${operationLabel(appliance.kind)} ${appliance.label}`}
                title={operationLabel(appliance.kind)}
                onclick={() => onOperate?.(appliance.id)}
              >
                <MonitorPlay size={15} strokeWidth={1.9} />
              </button>
            {/if}
            <button
              type="button"
              aria-label={`View ${appliance.label} configuration`}
              title={`View ${appliance.label} configuration`}
              onclick={() => onOpen(appliance.id)}
            >
              <FileText size={15} strokeWidth={1.9} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </section>
{/if}

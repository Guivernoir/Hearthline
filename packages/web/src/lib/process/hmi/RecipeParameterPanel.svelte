<script lang="ts">
  import { Check, ClipboardList, Gauge, LoaderCircle } from "@lucide/svelte";
  import type { HmiParameter, HmiRecipe, HmiStationStatus } from "./hmi-api";

  export let parameters: HmiParameter[] = [];
  export let recipes: HmiRecipe[] = [];
  export let activeRecipe: string | null = null;
  export let stations: HmiStationStatus[] = [];
  export let targetFilter = "";
  export let busyTarget = "";
  export let onParameter: (id: string, value: number) => void = () => {};
  export let onRecipe: (id: string) => void = () => {};

  let tab: "stations" | "parameters" | "recipes" = "stations";
  let draft: Record<string, number> = {};

  $: for (const parameter of parameters) {
    if (draft[parameter.id] === undefined) draft[parameter.id] = parameter.value;
  }
  $: targets = [...new Set(parameters
    .filter((parameter) => !targetFilter || parameter.target === targetFilter)
    .map((parameter) => parameter.target))];
  $: visibleStations = stations.filter((station) => targetFilter
    ? station.target === targetFilter
    : station.stationType !== "machine-pc");
</script>

<section class="hmi-engineering-panel" aria-label="Machine configuration">
  <header>
    <span><ClipboardList size={16} />Machine configuration</span>
    <nav aria-label="Configuration sections">
      <button type="button" class:active={tab === "stations"} onclick={() => (tab = "stations")}>Stations</button>
      <button type="button" class:active={tab === "parameters"} onclick={() => (tab = "parameters")}>Parameters</button>
      <button type="button" class:active={tab === "recipes"} onclick={() => (tab = "recipes")}>Recipes</button>
    </nav>
  </header>

  {#if tab === "stations"}
    <div class="hmi-station-grid">
      {#each visibleStations as station}
        <article class:bypass={station.sensorBypassActive}>
          <i></i>
          <span><strong>{station.label}</strong><small>{station.target}</small></span>
          <em>{station.selectedMode}</em>
        </article>
      {/each}
    </div>
  {:else if tab === "parameters"}
    <div class="hmi-parameter-groups">
      {#each targets as target}
        <section>
          <h3>{target.replaceAll("-", " ")}</h3>
          <div>
            {#each parameters.filter((parameter) => parameter.target === target) as parameter}
              <label>
                <span>{parameter.label}<small>{parameter.minimum}–{parameter.maximum} {parameter.unit}</small></span>
                <input bind:value={draft[parameter.id]} type="number" min={parameter.minimum} max={parameter.maximum} step={parameter.step} />
                <button
                  type="button"
                  aria-label={`Apply ${parameter.label}`}
                  title="Apply parameter"
                  disabled={busyTarget !== "" || draft[parameter.id] === parameter.value}
                  onclick={() => onParameter(parameter.id, Number(draft[parameter.id]))}
                >
                  {#if busyTarget === parameter.id}<LoaderCircle class="spin" size={14} />{:else}<Check size={14} />{/if}
                </button>
              </label>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  {:else}
    <div class="hmi-recipe-list">
      {#each recipes as recipe}
        <button type="button" class:active={activeRecipe === recipe.id} disabled={busyTarget !== ""} onclick={() => onRecipe(recipe.id)}>
          <Gauge size={17} />
          <span><strong>{recipe.label}</strong><small>{recipe.description}</small></span>
          {#if activeRecipe === recipe.id}<Check size={15} />{/if}
        </button>
      {/each}
    </div>
  {/if}
</section>

<script lang="ts">
  import { Check, ClipboardList, FlaskConical, LoaderCircle, Settings2 } from "@lucide/svelte";
  import type {
    HmiAction,
    HmiBodyPreparationState,
    HmiSnapshot,
    BodyPreparationHmiScope,
  } from "../../hmi-api";

  export let snapshot: HmiSnapshot;
  export let body: HmiBodyPreparationState;
  export let scope: BodyPreparationHmiScope;
  export let busyTarget = "";
  export let onExecute: (action: HmiAction, target: string) => void = () => {};

  let draft: Record<string, number> = {};
  $: localTrains = scope === "slip" ? [body.slip.train]
    : scope === "water-process" ? [body.water.train]
    : scope === "return-water-process" ? [body.returnWater.train]
    : [body.glaze.train];
  $: locked = localTrains
    .some((train) => train.running || train.held);
  $: for (const parameter of snapshot.parameters) {
    if (draft[parameter.id] === undefined) draft[parameter.id] = parameter.value;
  }

  function group(parameterId: string) {
    if (parameterId.startsWith("water-") || parameterId.includes("reuse")) return "Water and reuse";
    if (parameterId.startsWith("glaze-")) return "Glaze";
    return "Slip";
  }
</script>

<div class="body-recipe-page">
  <section class="body-recipe-list">
    <header><span><ClipboardList size={16} />Development recipes</span><small>Public engineering basis</small></header>
    {#each snapshot.recipes as recipe}
      <button
        type="button"
        class:active={snapshot.activeRecipe === recipe.id}
        disabled={locked || Boolean(busyTarget)}
        onclick={() => onExecute({ kind: "select-recipe", recipeId: recipe.id }, recipe.id)}
      ><FlaskConical size={18} /><span><strong>{recipe.label}</strong><small>{recipe.description}</small></span>{#if snapshot.activeRecipe === recipe.id}<Check size={15} />{/if}</button>
    {/each}
  </section>
  <section class="body-parameter-list">
    <header><span><Settings2 size={16} />Engineering setpoints</span><small>{locked ? "Hold active: changes locked" : "Idle-only changes"}</small></header>
    <div>
      {#each snapshot.parameters as parameter}
        <label>
          <span><em>{group(parameter.id)}</em><strong>{parameter.label}</strong><small>{parameter.minimum}-{parameter.maximum} {parameter.unit}</small></span>
          <input bind:value={draft[parameter.id]} type="number" min={parameter.minimum} max={parameter.maximum} step={parameter.step} disabled={locked} />
          <button
            type="button"
            aria-label={`Apply ${parameter.label}`}
            title="Apply parameter"
            disabled={locked || Boolean(busyTarget) || draft[parameter.id] === parameter.value}
            onclick={() => onExecute({ kind: "set-parameter", parameterId: parameter.id, value: Number(draft[parameter.id]) }, parameter.id)}
          >{#if busyTarget === parameter.id}<LoaderCircle class="spin" size={13} />{:else}<Check size={13} />{/if}</button>
        </label>
      {/each}
    </div>
  </section>
</div>

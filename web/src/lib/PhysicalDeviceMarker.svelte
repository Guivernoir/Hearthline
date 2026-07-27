<script lang="ts">
  import type { Component } from "svelte";

  export let icon: Component<any>;
  export let label: string;
</script>

<span class="physical-marker-symbol" aria-hidden="true">
  <svelte:component this={icon} size={40} strokeWidth={1.7} />
</span>
<strong class="physical-marker-label">{label}</strong>

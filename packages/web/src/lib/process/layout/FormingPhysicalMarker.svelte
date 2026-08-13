<script lang="ts">
  export let id: string;
  export let kind: string;
  export let label: string;

  $: marker = markerKind(id, kind);

  function markerKind(componentId: string, componentKind: string) {
    if (componentId === "area-02-robot-01") return "robot";
    if (componentId === "area-02-joystick-01") return "pendant";
    if (componentId.includes("handoff")) return "handoff";
    if (componentId === "area-02-cell-guard-safe-01") return "gate";
    if (componentId.includes("-hmi-") || componentId.includes("machine-pc")) return "screen";
    if (componentId.includes("-rio-") || componentId.includes("robot-controller")) return "control-cabinet";
    if (componentId.includes("manifold")) return "utility-cabinet";
    if (componentKind.includes("cabinet")) return "control-cabinet";
    return "mould";
  }
</script>

<span class={`forming-physical-equipment ${marker}`} aria-hidden="true">
  {#if marker === "mould"}
    <svg viewBox="0 0 150 88">
      <path class="machine-frame" d="M10 13 H140 V76 H126 V27 H24 V76 H10 Z"></path>
      <path class="mould-body" d="M48 31 H102 V67 H88 L75 77 L62 67 H48 Z"></path>
      <path class="mould-axis" d="M75 13 V43 M63 43 H87"></path>
      <rect class="integrated-utility-shell" x="108" y="35" width="29" height="35" rx="1"></rect>
      <path class="integrated-utility-lines" d="M114 44 H131 M114 53 H131 M114 62 H131"></path>
      <circle class="integrated-utility-status" cx="132" cy="30" r="3"></circle>
    </svg>
  {:else if marker === "robot"}
    <svg viewBox="0 0 150 110">
      <path class="robot-base" d="M31 98 H91 L82 74 H42 Z"></path>
      <circle cx="62" cy="72" r="14"></circle>
      <path class="robot-arm" d="M62 60 L68 25 L105 40 L127 22"></path>
      <circle cx="68" cy="25" r="10"></circle>
      <circle cx="105" cy="40" r="9"></circle>
      <path class="robot-tool" d="M126 13 V32 M126 17 L141 9 M126 28 L141 36"></path>
    </svg>
  {:else if marker === "screen"}
    <svg viewBox="0 0 108 82">
      <rect x="8" y="7" width="92" height="58" rx="3"></rect>
      <path d="M39 75 H69 M54 65 V75"></path>
      <path class="screen-status" d="M18 18 H90 V50 H18 Z"></path>
    </svg>
  {:else if marker === "pendant"}
    <svg viewBox="0 0 90 96">
      <rect x="13" y="5" width="64" height="86" rx="9"></rect>
      <rect class="screen-status" x="23" y="15" width="44" height="35" rx="2"></rect>
      <circle cx="45" cy="67" r="9"></circle>
      <path d="M45 58 V76 M36 67 H54"></path>
    </svg>
  {:else if marker === "handoff"}
    <svg viewBox="0 0 150 82">
      <path class="handoff-rail" d="M10 63 H140 M18 54 H132"></path>
      <rect class="handoff-platform" x="46" y="24" width="58" height="30" rx="2"></rect>
      <circle cx="28" cy="63" r="7"></circle>
      <circle cx="122" cy="63" r="7"></circle>
      <path class="handoff-piece" d="M62 24 V13 H88 V24"></path>
    </svg>
  {:else if marker === "gate"}
    <svg viewBox="0 0 130 96">
      <path class="gate-post" d="M17 8 V91 M113 8 V91"></path>
      <path class="gate-panel" d="M20 15 H104 V84 H20 Z M20 15 L104 84 M104 15 L20 84"></path>
      <rect class="gate-sensor" x="105" y="27" width="13" height="27" rx="2"></rect>
    </svg>
  {:else}
    <svg viewBox="0 0 105 95">
      <rect x="8" y="5" width="89" height="86" rx="2"></rect>
      <path d="M16 23 H89 M16 66 H89"></path>
      {#if marker === "control-cabinet"}
        <path class="cabinet-slots" d="M22 34 H49 V55 H22 Z M57 34 H83 V55 H57 Z"></path>
      {:else}
        <circle cx="29" cy="44" r="8"></circle>
        <circle cx="53" cy="44" r="8"></circle>
        <circle cx="77" cy="44" r="8"></circle>
      {/if}
      <circle class="cabinet-light" cx="81" cy="78" r="4"></circle>
    </svg>
  {/if}
</span>
<strong class="physical-marker-label">{label}</strong>

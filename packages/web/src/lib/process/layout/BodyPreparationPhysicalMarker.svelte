<script lang="ts">
  export let id: string;
  export let kind: string;
  export let label: string;

  $: marker = markerKind(id, kind);

  function markerKind(componentId: string, componentKind: string) {
    if (componentId.includes("rw-clar")) return "clarifier";
    if (componentId.includes("rw-fp")) return "filter-press";
    if (componentId.includes("gl-mill")) return "mill";
    if (componentId.includes("wt-ro")) return "ro-skid";
    if (componentId.includes("wt-soft")) return "softener";
    if (componentId.includes("wt-fil") || componentId.includes("wt-carb")) return "filter";
    if (componentId.includes("feed-")) return "feeder";
    if (componentId.includes("wit-")) return "hopper";
    if (componentId === "area-01-ag-01") return "mixer";
    if (componentId.includes("-lt-") || componentId.includes("ag-02") || componentId.includes("gl-ag")) return "tank";
    if (componentId.includes("scr-")) return "screen";
    if (componentId.includes("mag-")) return "magnet";
    if (componentId.includes("pmp-") || componentId.includes("dp-")) return "pump";
    if (componentId.includes("xv-") || componentId.includes("ht-")) return "valve";
    if (componentId.includes("hmi-")) return "screen-panel";
    if (componentId.includes("rio-") || componentId.includes("intlk-")) return "cabinet";
    if (componentKind.includes("instrument")) return "instrument";
    return "instrument";
  }
</script>

<span class={`body-physical-equipment ${marker}`} aria-hidden="true">
  {#if marker === "feeder"}
    <svg viewBox="0 0 120 110"><path d="M18 12 H102 L88 62 H32 Z"></path><path d="M48 62 H72 V84 H90 V101 H30 V84 H48 Z"></path><path class="detail" d="M27 27 H93 M35 45 H85"></path></svg>
  {:else if marker === "hopper"}
    <svg viewBox="0 0 130 110"><path d="M18 12 H112 V58 L82 88 H48 L18 58 Z"></path><path d="M48 88 V101 M82 88 V101"></path><path class="load-cell" d="M38 96 H55 M75 96 H92"></path></svg>
  {:else if marker === "mixer"}
    <svg viewBox="0 0 160 120"><path d="M25 28 H135 V93 Q80 113 25 93 Z"></path><path class="detail" d="M80 6 V84 M55 62 L80 84 L105 62"></path><rect x="62" y="5" width="36" height="22"></rect></svg>
  {:else if marker === "tank"}
    <svg viewBox="0 0 150 125"><ellipse cx="75" cy="23" rx="50" ry="14"></ellipse><path d="M25 23 V99 Q75 119 125 99 V23"></path><path class="detail" d="M75 8 V92 M52 72 L75 92 L98 72"></path></svg>
  {:else if marker === "screen"}
    <svg viewBox="0 0 140 95"><path d="M14 24 H126 L111 66 H29 Z"></path><path class="mesh" d="M28 35 H112 M24 47 H116 M34 59 H106"></path><path d="M38 66 V87 M102 66 V87"></path></svg>
  {:else if marker === "magnet"}
    <svg viewBox="0 0 130 100"><rect x="15" y="18" width="100" height="65"></rect><path class="magnet-core" d="M42 35 V62 Q42 76 65 76 Q88 76 88 62 V35 M42 35 H55 V59 M88 35 H75 V59"></path></svg>
  {:else if marker === "pump"}
    <svg viewBox="0 0 120 90"><circle cx="55" cy="45" r="30"></circle><path class="detail" d="M55 25 L79 45 L55 65 Z M85 45 H113"></path><path d="M25 75 H88"></path></svg>
  {:else if marker === "valve"}
    <svg viewBox="0 0 110 85"><path d="M8 42 H102 M28 20 L55 42 L28 64 Z M82 20 L55 42 L82 64 Z"></path><circle cx="55" cy="16" r="11"></circle></svg>
  {:else if marker === "screen-panel"}
    <svg viewBox="0 0 115 92"><rect x="8" y="8" width="99" height="65"></rect><rect class="panel-display" x="20" y="18" width="75" height="40"></rect><path d="M43 84 H72 M57 73 V84"></path></svg>
  {:else if marker === "cabinet"}
    <svg viewBox="0 0 105 100"><rect x="9" y="5" width="87" height="90"></rect><path class="detail" d="M18 24 H87 M18 67 H87"></path><circle class="status" cx="80" cy="81" r="4"></circle></svg>
  {:else if marker === "filter"}
    <svg viewBox="0 0 130 120"><ellipse cx="65" cy="20" rx="34" ry="12"></ellipse><path d="M31 20 V96 Q65 112 99 96 V20"></path><path class="media-bed" d="M35 53 Q65 41 95 53 V91 Q65 103 35 91 Z"></path><path class="detail" d="M65 8 V105 M20 64 H31 M99 64 H112"></path></svg>
  {:else if marker === "softener"}
    <svg viewBox="0 0 150 120"><ellipse cx="48" cy="20" rx="25" ry="10"></ellipse><path d="M23 20 V94 Q48 108 73 94 V20"></path><ellipse cx="105" cy="20" rx="25" ry="10"></ellipse><path d="M80 20 V94 Q105 108 130 94 V20"></path><path class="detail" d="M48 8 V105 M105 8 V105 M73 64 H80"></path></svg>
  {:else if marker === "ro-skid"}
    <svg viewBox="0 0 170 110"><rect x="8" y="12" width="154" height="88"></rect><rect class="vessel" x="25" y="27" width="112" height="18" rx="9"></rect><rect class="vessel" x="25" y="55" width="112" height="18" rx="9"></rect><path class="detail" d="M21 84 H145 M145 36 H156 M145 64 H156 M18 36 H25 M18 64 H25"></path></svg>
  {:else if marker === "mill"}
    <svg viewBox="0 0 170 115"><circle cx="82" cy="55" r="43"></circle><path class="mill-band" d="M50 18 Q82 8 114 18 V92 Q82 102 50 92 Z"></path><path d="M16 55 H39 M125 55 H153 M32 98 H132"></path><circle class="detail" cx="82" cy="55" r="9"></circle></svg>
  {:else if marker === "clarifier"}
    <svg viewBox="0 0 170 120"><path d="M15 25 H155 L130 94 H40 Z"></path><path class="detail" d="M85 8 V91 M47 43 H123 M55 61 H115 M64 79 H106"></path><path d="M40 94 V108 M130 94 V108"></path></svg>
  {:else if marker === "filter-press"}
    <svg viewBox="0 0 170 110"><path d="M15 22 H155 M25 88 H145 M30 22 V92 M140 22 V92"></path><path class="press-plates" d="M45 30 V82 M58 30 V82 M71 30 V82 M84 30 V82 M97 30 V82 M110 30 V82 M123 30 V82"></path><path d="M8 55 H30 M140 55 H162"></path></svg>
  {:else}
    <svg viewBox="0 0 90 90"><circle cx="45" cy="45" r="34"></circle><path class="detail" d="M45 45 L66 31 M20 45 H27 M63 65 L70 72"></path></svg>
  {/if}
</span>
<strong class="physical-marker-label">{label}</strong>

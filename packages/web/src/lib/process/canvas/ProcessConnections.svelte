<script lang="ts">
  import { processView, type ProcessEdge, type ProcessPosition } from "../process-model";

  const positions = new Map<string, ProcessPosition>([
    ...processView.supportNodes.map((node) => [node.id, node.position] as const),
    ...processView.areas.map((area) => [area.id, area.position] as const),
  ]);
  const supportIds = new Set(processView.supportNodes.map((node) => node.id));
  const supportEdges = processView.networkEdges.filter(
    (edge) => supportIds.has(edge.source) && supportIds.has(edge.destination),
  );

  function edgePath(edge: ProcessEdge) {
    const source = positions.get(edge.source);
    const destination = positions.get(edge.destination);
    if (!source || !destination) return "";
    const sourceCenterX = source.x + source.width / 2;
    const sourceCenterY = source.y + source.height / 2;
    const destinationCenterX = destination.x + destination.width / 2;
    const destinationCenterY = destination.y + destination.height / 2;
    if (Math.abs(sourceCenterY - destinationCenterY) < 20) {
      const sourceX = sourceCenterX < destinationCenterX
        ? source.x + source.width
        : source.x;
      const destinationX = sourceCenterX < destinationCenterX
        ? destination.x
        : destination.x + destination.width;
      return `M${sourceX} ${sourceCenterY} H${destinationX}`;
    }
    const sourceY = sourceCenterY < destinationCenterY
      ? source.y + source.height
      : source.y;
    const destinationY = sourceCenterY < destinationCenterY
      ? destination.y
      : destination.y + destination.height;
    const middleY = (sourceY + destinationY) / 2;
    return `M${sourceCenterX} ${sourceY} V${middleY} H${destinationCenterX} V${destinationY}`;
  }
</script>

<svg
  class="connections"
  viewBox="0 0 1640 1080"
  aria-hidden="true"
>
  <defs>
    <marker
      id="network-arrow"
      markerWidth="8"
      markerHeight="8"
      refX="7"
      refY="4"
      orient="auto"
    >
      <path d="M0,0 L8,4 L0,8 Z" fill="#4d7379"></path>
    </marker>
    <marker
      id="material-arrow"
      markerWidth="9"
      markerHeight="9"
      refX="8"
      refY="4.5"
      orient="auto"
    >
      <path d="M0,0 L9,4.5 L0,9 Z" fill="#c76432"></path>
    </marker>
  </defs>

  <g class="process-physical-background">
    <rect class="process-site-shell" x="25" y="90" width="1590" height="850"></rect>
    <rect class="process-support-gallery" x="25" y="90" width="1590" height="175"></rect>
    <rect class="process-production-hall" x="25" y="280" width="1590" height="660"></rect>
    <rect class="process-utility-corridor" x="45" y="615" width="1550" height="82"></rect>

    <rect class="process-cell process-cell-wet" x="40" y="370" width="285" height="230"></rect>
    <rect class="process-cell process-cell-forming" x="350" y="370" width="285" height="230"></rect>
    <rect class="process-cell process-cell-drying" x="660" y="370" width="285" height="230"></rect>
    <rect class="process-cell process-cell-thermal" x="970" y="370" width="285" height="230"></rect>
    <rect class="process-cell process-cell-finish" x="1280" y="370" width="285" height="230"></rect>

    <rect class="process-cell process-cell-logistics" x="40" y="710" width="285" height="210"></rect>
    <rect class="process-cell process-cell-quality" x="350" y="710" width="285" height="210"></rect>
    <rect class="process-cell process-cell-thermal" x="660" y="710" width="285" height="210"></rect>
    <rect class="process-cell process-cell-quality" x="970" y="710" width="285" height="210"></rect>
    <rect class="process-cell process-cell-thermal" x="1280" y="710" width="285" height="210"></rect>

    <path class="process-overhead-utility" d="M80 640 H1560"></path>
    <path class="process-floor-rail" d="M75 585 H1565 M75 902 H1565"></path>
    <path class="process-loading-bay" d="M55 750 H100 V885 H55 M265 750 H310 V885 H265"></path>
    <path class="process-exhaust-stack" d="M1120 365 V305 H1150 V365 M1430 705 V645 H1460 V705"></path>

    <text class="process-physical-label" x="50" y="112">CONTROL AND SITE SERVICES GALLERY</text>
    <text class="process-physical-label" x="50" y="310">CERAMICS PRODUCTION HALL</text>
    <text class="process-physical-label" x="70" y="666">UTILITY, MATERIAL, AND MAINTENANCE CORRIDOR</text>
    <text class="process-detail-label" x="70" y="736">PACKING / DISPATCH BAY</text>
    <text class="process-detail-label" x="1320" y="736">PRIMARY FIRING BAY</text>
  </g>

  <g class="network-connections">
    {#each processView.networkEdges as edge (`${edge.source}:${edge.destination}`)}
      <path d={edgePath(edge)} marker-end="url(#network-arrow)"></path>
    {/each}
  </g>

  <g class="material-connections">
    {#each processView.materialFlow as edge (`${edge.source}:${edge.destination}`)}
      <path d={edgePath(edge)} marker-end="url(#material-arrow)"></path>
    {/each}
  </g>

  <g class="physical-support-connections">
    {#each supportEdges as edge (`${edge.source}:${edge.destination}`)}
      <path class:site-support-link={edge.source === "operations-intelligence" || edge.source === "site-conduit"} d={edgePath(edge)}></path>
    {/each}
  </g>

  <text x="74" y="335" class="line-label network-label">CONTROL NETWORK</text>
  <text x="72" y="405" class="line-label material-label">MATERIAL FLOW</text>
</svg>

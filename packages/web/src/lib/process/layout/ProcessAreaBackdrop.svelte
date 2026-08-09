<svelte:options namespace="svg" />

<script lang="ts">
  export let routeKey: string;
</script>

<defs>
  <marker id="process-data-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
    <path d="M0 0 L8 4 L0 8 Z"></path>
  </marker>
  <marker id="process-command-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
    <path d="M0 0 L8 4 L0 8 Z"></path>
  </marker>
</defs>

{#if routeKey === "forming"}
  <g class="process-area-physical-layer">
    <rect class="area-facility-shell" x="20" y="105" width="1860" height="1205"></rect>
    <rect class="area-control-room" x="40" y="130" width="1000" height="300"></rect>
    <rect class="area-compute-bay" x="60" y="165" width="440" height="225"></rect>
    <rect class="area-operator-bay" x="520" y="165" width="500" height="225"></rect>
    <rect class="area-field-room forming-machine-bay" x="1060" y="130" width="800" height="300"></rect>
    <path class="forming-machine-shape" d="M1105 215 H1235 V335 H1105 Z M1120 195 H1220 V215 M1130 255 H1210"></path>
    <path class="forming-machine-shape forming-press-frame" d="M1320 190 H1485 V350 H1455 V220 H1350 V350 H1320 Z M1370 240 H1435 V300 H1370 Z"></path>
    <path class="forming-machine-shape" d="M1550 330 H1785 V350 H1550 Z M1580 330 V285 L1635 250 L1680 285 M1635 250 V215 M1618 215 H1652"></path>
    <circle class="forming-utility-vessel" cx="1575" cy="205" r="24"></circle>
    <circle class="forming-utility-vessel" cx="1710" cy="205" r="24"></circle>
    <circle class="forming-utility-vessel" cx="1780" cy="205" r="24"></circle>
    <path class="forming-machine-flow" d="M1060 290 H1105 M1235 290 H1320 M1485 290 H1550"></path>
    <path class="forming-machine-arrow" d="M1095 290 l-13 -8 v16 Z M1310 290 l-13 -8 v16 Z M1540 290 l-13 -8 v16 Z"></path>
    <rect class="forming-module forming-feed-module" x="35" y="520" width="435" height="690"></rect>
    <rect class="forming-module forming-press-module" x="495" y="520" width="435" height="690"></rect>
    <rect class="forming-module forming-transfer-module" x="955" y="520" width="435" height="690"></rect>
    <rect class="forming-module forming-utilities-module" x="1415" y="520" width="435" height="690"></rect>
    <path class="area-cable-tray" d="M160 350 H910 V470 H250 V565 M910 470 H1630 V565"></path>
    <path class="area-material-line forming-material-flow" d="M120 1260 H1370"></path>
    <path class="forming-flow-arrow" d="M450 1260 l-18 -10 v20 Z M910 1260 l-18 -10 v20 Z M1370 1260 l-18 -10 v20 Z"></path>
    <path class="forming-utility-line" d="M1630 1175 H710 V1210"></path>
    <text class="area-zone-label" x="75" y="155">CELL NETWORK / HOSTED CONTROL RUNTIME</text>
    <text class="area-zone-label" x="540" y="155">SCADA AND DISTRIBUTED I/O CABINETS</text>
    <text class="area-zone-label" x="1080" y="155">FORMING MACHINE ENVELOPE</text>
    <text class="area-detail-label" x="1068" y="275">FROM BODY PREPARATION</text>
    <text class="area-detail-label" x="1115" y="390">SLIP TANK / 40 C</text>
    <text class="area-detail-label" x="1350" y="390">PRESSURE-CASTING MOULD</text>
    <text class="area-detail-label" x="1560" y="390">ROBOT / OPERATOR HANDOFF</text>
    <text class="area-detail-label" x="1545" y="170">WATER</text>
    <text class="area-detail-label" x="1682" y="170">AIR</text>
    <text class="area-detail-label" x="1748" y="170">VACUUM</text>
    <text class="forming-module-label" x="55" y="550">01 / CERAMIC-SLIP SUPPLY</text>
    <text class="forming-module-label" x="515" y="550">02 / PRESSURE CASTING AND MOULD</text>
    <text class="forming-module-label" x="975" y="550">03 / ROBOTIC DEMOULD AND HANDOFF</text>
    <text class="forming-module-label" x="1435" y="550">SUPPORT / WATER, AIR, VACUUM</text>
    <text class="area-detail-label" x="85" y="1250">SLIP CASTING TO GREEN-PIECE HANDOFF</text>
    <text class="area-detail-label" x="1435" y="1205">RELEASE, CLEANING, AND DRYING UTILITIES</text>
  </g>

  <g class="process-area-logical-layer">
    <rect class="area-logical-zone area-control-zone" x="35" y="115" width="1010" height="330"></rect>
    <rect class="area-logical-zone area-network-zone" x="1065" y="115" width="800" height="330"></rect>
    <rect class="forming-module forming-feed-module" x="35" y="520" width="435" height="690"></rect>
    <rect class="forming-module forming-press-module" x="495" y="520" width="435" height="690"></rect>
    <rect class="forming-module forming-transfer-module" x="955" y="520" width="435" height="690"></rect>
    <rect class="forming-module forming-utilities-module" x="1415" y="520" width="435" height="690"></rect>
    <text class="area-zone-label" x="55" y="145">CONTROL AUTHORITY, SUPERVISION, AND I/O EXCHANGE</text>
    <text class="area-zone-label" x="1085" y="145">CELL ACCESS NETWORK / LEVEL 3 UPLINK BOUNDARY</text>
    <text class="forming-module-label" x="55" y="550">CERAMIC-SLIP SUPPLY MODULE</text>
    <text class="forming-module-label" x="515" y="550">PRESSURE-CASTING MOULD MODULE</text>
    <text class="forming-module-label" x="975" y="550">ROBOTIC DEMOULDING MODULE</text>
    <text class="forming-module-label" x="1435" y="550">WATER / AIR / VACUUM MODULE</text>
    <text class="forming-flow-note" x="55" y="1290">Inputs: sensor -> remote I/O -> PLC -> operator interfaces</text>
    <text class="forming-flow-note" x="1000" y="1290">Commands: operator interface -> PLC -> remote I/O -> actuator</text>
  </g>
{:else}
  <g class="process-area-physical-layer">
    <rect class="area-facility-shell" x="25" y="100" width="1230" height="720"></rect>
    <rect class="area-control-room" x="45" y="125" width="485" height="650"></rect>
    <rect class="area-field-room" x="555" y="125" width="680" height="650"></rect>
    <rect class="area-compute-bay" x="55" y="135" width="220" height="180"></rect>
    <rect class="area-operator-bay" x="290" y="135" width="235" height="180"></rect>
    <rect class="area-cabinet-row" x="55" y="340" width="470" height="245"></rect>
    <rect class="area-safety-bay" x="900" y="670" width="300" height="125"></rect>
    <path class="area-cable-tray" d="M165 270 V355 H410 V452 H695 V165 H1150"></path>
    <path class="area-material-line" d="M595 485 H1165"></path>
    <path class="area-floor-marking" d="M575 160 H1210 V650 H575 Z M850 160 V650"></path>
    <text class="area-zone-label" x="70" y="150">LEVEL 3 CONTROL COMPUTE</text>
    <text class="area-zone-label" x="305" y="150">LOCAL OPERATOR STATION</text>
    <text class="area-zone-label" x="70" y="365">CELL NETWORK AND DISTRIBUTED I/O</text>
    <text class="area-zone-label" x="585" y="155">FIELD EQUIPMENT BAY</text>
    <text class="area-zone-label" x="925" y="700">SAFETY / INTERLOCK BOUNDARY</text>
    <text class="area-detail-label" x="640" y="475">PROCESS MATERIAL PATH</text>
  </g>

  <g class="process-area-logical-layer">
    <rect class="area-logical-zone area-network-zone" x="35" y="315" width="245" height="270"></rect>
    <rect class="area-logical-zone area-control-zone" x="295" y="90" width="260" height="700"></rect>
    <rect class="area-logical-zone area-input-zone" x="575" y="90" width="280" height="700"></rect>
    <rect class="area-logical-zone area-output-zone" x="875" y="90" width="350" height="550"></rect>
    <rect class="area-logical-zone area-safety-zone" x="875" y="655" width="350" height="160"></rect>
    <text class="area-zone-label" x="55" y="345">CELL NETWORK</text>
    <text class="area-zone-label" x="315" y="120">CONTROL AND OPERATOR LAYER</text>
    <text class="area-zone-label" x="595" y="120">PROCESS INPUTS</text>
    <text class="area-zone-label" x="895" y="120">PROCESS OUTPUTS</text>
    <text class="area-zone-label" x="895" y="685">SAFETY AND PERMISSIVES</text>
  </g>
{/if}

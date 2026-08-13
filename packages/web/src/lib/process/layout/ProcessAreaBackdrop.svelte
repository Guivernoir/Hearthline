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
    <rect class="forming-floor" x="20" y="105" width="2270" height="1030"></rect>
    <rect class="forming-cabinet-zone" x="40" y="145" width="250" height="965"></rect>
    <rect class="forming-operator-zone" x="1790" y="125" width="480" height="985"></rect>
    <path class="forming-cell-fence" d="M390 1100 V155 H1755 V1100 H1680 M1510 1100 H390"></path>
    <path class="forming-guard-aisle" d="M390 370 H1755 M390 590 H1755 M390 810 H1755 M390 1030 H1540"></path>
    <path class="forming-robot-envelope" d="M985 245 Q1580 585 985 985 Q690 585 985 245 Z"></path>
    <path class="forming-utility-trench" d="M470 985 V250 M470 250 H515 M470 470 H515 M470 690 H515 M470 910 H515"></path>
    <path class="forming-handoff-line" d="M1530 250 H1885 M1530 470 H1885 M1530 690 H1885 M1530 910 H1885"></path>
    <path class="forming-gate-swing" d="M1680 1100 A120 120 0 0 0 1560 980"></path>
    <path class="forming-transfer-boundary" d="M1755 175 V960"></path>
    <text class="area-zone-label" x="55" y="175">EXTERNAL MOULD CONTROL CABINETS</text>
    <text class="area-zone-label" x="415" y="180">INTERLOCKED FORMING AND ROBOT CELL / TOP VIEW</text>
    <text class="area-zone-label forming-boundary-label" x="1495" y="180">GUARDED TRANSFER BOUNDARY</text>
    <text class="area-zone-label" x="1815" y="155">OPERATOR AND ROBOT CONTROL AREA</text>
  </g>

  <g class="process-area-logical-layer">
    <rect class="area-logical-zone area-control-zone" x="40" y="130" width="1030" height="310"></rect>
    <rect class="area-logical-zone area-network-zone" x="1090" y="130" width="1180" height="310"></rect>
    <rect class="forming-station-bay forming-shared-bay" x="40" y="520" width="410" height="1035"></rect>
    <rect class="forming-station-bay forming-mould-bay" x="480" y="520" width="410" height="1035"></rect>
    <rect class="forming-station-bay forming-mould-bay" x="940" y="520" width="410" height="1035"></rect>
    <rect class="forming-station-bay forming-mould-bay" x="1400" y="520" width="410" height="1035"></rect>
    <rect class="forming-station-bay forming-mould-bay" x="1860" y="520" width="410" height="1035"></rect>
    <rect class="forming-station-bay forming-robot-bay" x="480" y="1625" width="1790" height="500"></rect>

    <path class="forming-authority-line" d="M625 470 H2210"></path>
    <path class="forming-authority-line forming-safety-authority" d="M680 1600 H2210"></path>

    <text class="area-zone-label" x="60" y="160">PLC CONTROL AUTHORITY / EMBEDDED SCADA SUPERVISION</text>
    <text class="area-zone-label" x="1110" y="160">CELL ETHERNET / FACTORY LEVEL 3 UPLINK BOUNDARY</text>
    <text class="forming-flow-note" x="60" y="415">Machine PC: logs, live values, mould parameters, recipes, valve service, and controller-source view</text>
    <text class="forming-flow-note" x="1110" y="415">Automatic commands: PLC to station I/O / Manual movement: local keyed selector</text>
    <text class="forming-module-label" x="60" y="552">SHARED MATERIAL CONDITIONING INPUTS</text>
    <text class="forming-module-label" x="500" y="552">MOULD 1 / HMI 1 / RIO 1</text>
    <text class="forming-module-label" x="960" y="552">MOULD 2 / HMI 2 / RIO 2</text>
    <text class="forming-module-label" x="1420" y="552">MOULD 3 / HMI 3 / RIO 3</text>
    <text class="forming-module-label" x="1880" y="552">MOULD 4 / HMI 4 / RIO 4</text>
    <text class="forming-module-label" x="500" y="1657">ROBOT, GUARDED-CELL SAFETY, AND FOUR OPERATOR HANDOFF TRANSFER STATIONS</text>
    <text class="forming-flow-note" x="60" y="1588">Sensor data and actuator feedback return through the owning remote-I/O station</text>
    <text class="forming-flow-note" x="1510" y="1588">Setup bypasses process sensors, never E-stops or hardwired travel limits</text>
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

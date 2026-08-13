<script lang="ts">
  import {
    Activity,
    CircleGauge,
    Clock3,
    Gauge,
    Move3D,
    Rotate3D,
    Thermometer,
    Waves,
  } from "@lucide/svelte";
  import type {
    HmiActuator,
    HmiControlMode,
    HmiMouldProcessState,
    HmiSignal,
    HmiStationStatus,
  } from "../hmi-api";

  export let signals: HmiSignal[] = [];
  export let actuators: HmiActuator[] = [];
  export let stations: HmiStationStatus[] = [];
  export let mould: HmiMouldProcessState;

  const tags: Record<string, Record<string, string>> = {
    "mould-01": {
      pressure: "area-02-pt-02",
      temperature: "area-02-tt-02",
      fillHead: "area-02-pos-01",
      position: "area-02-pos-02",
      moisture: "area-02-mt-02",
      inclination: "area-02-m01-inc-01",
      movement: "area-02-mould-01-command",
      manifold: "area-02-m01-manifold-01-command",
    },
    "mould-02": stationTags("m02"),
    "mould-03": stationTags("m03"),
    "mould-04": stationTags("m04"),
  };

  $: currentTags = tags[mould.target] ?? tags["mould-01"];
  $: pressure = value(currentTags.pressure);
  $: temperature = value(currentTags.temperature);
  $: fillHead = value(currentTags.fillHead);
  $: position = value(currentTags.position);
  $: moisture = value(currentTags.moisture);
  $: inclination = value(currentTags.inclination);
  $: selector = stations.find((station) => station.target === mould.target);
  $: mode = selector?.selectedMode ?? "auto";
  $: movement = actuatorState(currentTags.movement, "stopped");
  $: manifold = actuatorState(currentTags.manifold, "isolated");
  $: opening = Math.min(1, Math.max(0, position / 600));
  $: fillProbe = Math.min(1, Math.max(0, fillHead / 800));
  $: mouldGap = opening * 32;
  $: upperLift = opening * 30;
  $: fillProbeY = 90 + fillProbe * 45;
  $: pieceVisible = ["mould-opening", "robot-pickup", "operator-delivery"].includes(mould.phase);
  $: service = serviceState(manifold);

  function stationTags(prefix: string) {
    return {
      pressure: `area-02-${prefix}-pt-01`,
      temperature: `area-02-${prefix}-tt-01`,
      fillHead: `area-02-${prefix}-pos-01`,
      position: `area-02-${prefix}-pos-02`,
      moisture: `area-02-${prefix}-mt-01`,
      inclination: `area-02-${prefix}-inc-01`,
      movement: `area-02-${prefix}-mould-01-command`,
      manifold: `area-02-${prefix}-manifold-01-command`,
    };
  }

  function value(tag: string) {
    return signals.find((signal) => signal.tag === tag)?.value ?? 0;
  }

  function actuatorState(tag: string, fallback: string) {
    return actuators.find((actuator) => actuator.commandTag === tag)?.currentState ?? fallback;
  }

  function modeLabel(controlMode: HmiControlMode) {
    return controlMode.toUpperCase();
  }

  function serviceState(state: string) {
    if (state.includes("slip") || state.includes("drain")) return "slip";
    if (state.includes("water") || state.includes("wash")) return "water";
    if (state.includes("air") || state.includes("pressure")) return "air";
    if (state.includes("vacuum")) return "vacuum";
    return "isolated";
  }

  function display(value: string) {
    return value.replaceAll("-", " ");
  }
</script>

<section class="hmi-mould-visual" aria-label={`${mould.target} live machine view`}>
  <header>
    <span><Activity size={16} />Live mould view</span>
    <strong class:attention={mould.stopRequest}>{display(mould.operatingState)}</strong>
  </header>

  <div class="hmi-mould-status-strip">
    <span><i class={`mode-${mode}`}></i>{modeLabel(mode)}</span>
    <span><Clock3 size={13} />{display(mould.phase)}</span>
    <span><CircleGauge size={13} />Cycle {mould.cycleCount}</span>
    <span class:attention={mould.stopRequest}>Request {mould.stopRequest ? display(mould.stopRequest) : "none"}</span>
  </div>

  <div class="hmi-mould-visual-body">
    <svg viewBox="0 0 720 340" role="img" aria-label={`${mould.label} at ${inclination.toFixed(1)} degrees, ${display(mould.phase)}`}>
      <defs>
        <linearGradient id={`steel-${mould.target}`} x1="0" x2="1">
          <stop offset="0" stop-color="#dbe3e1"></stop>
          <stop offset="0.5" stop-color="#f6f8f7"></stop>
          <stop offset="1" stop-color="#c9d4d1"></stop>
        </linearGradient>
      </defs>

      <path class="machine-floor" d="M36 298 H682"></path>
      <path class="machine-bed" d="M62 273 H650 L626 299 H83 Z"></path>
      <path class="machine-column" d="M92 273 V66 H126 V273 M590 273 V66 H624 V273"></path>
      <path class="machine-top-beam" d="M84 66 H632 V91 H84 Z"></path>

      <g class="main-axis">
        <rect class="hydraulic-body" x="500" y="174" width="94" height="48" rx="3"></rect>
        <path class="hydraulic-rod" d={`M500 198 H${432 + mouldGap}`}></path>
        <rect class="moving-crossbar" x={402 + mouldGap} y="116" width="30" height="151" rx="2"></rect>
        <rect class="fixed-crossbar" x={184 - mouldGap} y="116" width="30" height="151" rx="2"></rect>
      </g>

      <g class="vertical-axis">
        <rect class="vertical-cylinder" x="288" y="76" width="60" height="39" rx="3"></rect>
        <path class="hydraulic-rod" d={`M318 115 V${137 - upperLift}`}></path>
        <rect class="upper-platen" x="244" y={137 - upperLift} width="148" height="20" rx="2"></rect>
      </g>

      <g class="service-manifold">
        <rect x="105" y="105" width="61" height="87" rx="3"></rect>
        <circle class:active={service === "slip"} class="service-slip" cx="119" cy="124" r="6"></circle>
        <circle class:active={service === "air"} class="service-air" cx="119" cy="143" r="6"></circle>
        <circle class:active={service === "water"} class="service-water" cx="119" cy="162" r="6"></circle>
        <circle class:active={service === "vacuum"} class="service-vacuum" cx="119" cy="181" r="6"></circle>
        <path class:active={service !== "isolated"} class={`service-line service-${service}`} d="M166 148 H230 V184 H253"></path>
      </g>

      <g class="mould-tilt-frame" transform={`rotate(${inclination} 318 231)`}>
        <path class="mould-carrier" d="M228 265 V151 H408 V265"></path>
        <g class="mould-package">
          <path class="mould-part mould-part-left" transform={`translate(${-mouldGap} 0)`} d="M250 171 H313 V258 H267 Q250 242 250 218 Z"></path>
          <path class="mould-part mould-part-right" transform={`translate(${mouldGap} 0)`} d="M323 171 H386 V218 Q386 242 369 258 H323 Z"></path>
          <path class="mould-part mould-part-top" transform={`translate(0 ${-upperLift})`} d="M274 147 H362 L350 183 H286 Z"></path>
          <path class:visible={pieceVisible} class="cast-piece" d="M299 188 Q318 174 337 188 V241 Q318 252 299 241 Z"></path>
          <path class:active={pressure > 0.1} class="mould-cavity-fill" d="M301 238 V191 Q318 179 335 191 V238 Q318 247 301 238 Z"></path>
        </g>
      </g>

      <g class="fill-probe" transform={`translate(0 ${fillProbeY - 90})`}>
        <path d="M307 91 H329 V136 H322 V153 H314 V136 H307 Z"></path>
      </g>

      <g class="machine-labels">
        <text x="92" y="51">FIXED MACHINE FRAME</text>
        <text x="497" y="238">MAIN HYDRAULIC AXIS</text>
        <text x="274" y="52">VERTICAL CLAMP</text>
        <text x="102" y="211">SERVICE MANIFOLD</text>
        <text x="260" y="324">MULTI-PART MOULD ASSEMBLY</text>
      </g>
    </svg>

    <dl class="hmi-mould-metrics">
      <div><dt><Rotate3D size={14} />Inclination</dt><dd>{inclination.toFixed(1)}<small>degree</small></dd></div>
      <div><dt><Move3D size={14} />Opening stroke</dt><dd>{position.toFixed(0)}<small>millimetre</small></dd></div>
      <div><dt><Move3D size={14} />Fill probe</dt><dd>{fillHead.toFixed(0)}<small>millimetre</small></dd></div>
      <div><dt><Gauge size={14} />Casting pressure</dt><dd>{pressure.toFixed(1)}<small>bar</small></dd></div>
      <div><dt><Thermometer size={14} />Mould temperature</dt><dd>{temperature.toFixed(1)}<small>degree Celsius</small></dd></div>
      <div><dt><Waves size={14} />Residual moisture</dt><dd>{moisture.toFixed(1)}<small>percent</small></dd></div>
    </dl>
  </div>

  <footer>
    <span>Movement <strong>{display(movement)}</strong></span>
    <span>Manifold <strong>{display(manifold)}</strong></span>
    <span>Phase elapsed <strong>{(mould.phaseElapsedMs / 1000).toFixed(1)} s</strong></span>
  </footer>
</section>

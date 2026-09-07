import type {
  HmiBodyPreparationState,
  HmiMouldProcessState,
  HmiRobotPose,
  HmiSnapshot,
} from "../lib/process/hmi/hmi-api";
import type {
  ScenarioPacket,
  ScenarioReport,
  ScenarioSummary,
} from "../lib/simulation/simulation-api";
import type {
  WorkstationActionReport,
  WorkstationProfile,
} from "../lib/workstation/workstation-api";
import type { SecurityConsoleSession } from "../lib/office/security-api";

const pose: HmiRobotPose = { x: 100, y: 200, z: 300, w: 0, p: 90, r: 0 };
const phases = [
  { key: "fill", label: "Fill" },
  { key: "pressure", label: "Pressure" },
  { key: "drain", label: "Drain" },
];

function train(id: "slip" | "water" | "return-water" | "glaze") {
  return {
    id,
    label: `${id} train`,
    running: true,
    held: false,
    phase: "mixing",
    phaseProgressPercent: 42,
    phaseElapsedProcessMinutes: 6,
    phaseTargetProcessMinutes: 15,
    cycleCount: 3,
    phases,
  };
}

const quality = {
  temperatureC: 24,
  ph: 7.1,
  turbidityNtu: 0.8,
  conductivityUsCm: 280,
  hardnessMgLCaco3: 65,
  suspendedSolidsMgL: 4,
  glazeContaminationPercent: 0.01,
  recoveredFractionPercent: 20,
};

const pipeline = {
  inletFlowLMin: 100,
  outletFlowLMin: 98,
  inletPressureBar: 4.2,
  outletPressureBar: 3.9,
  lineLossPercent: 2,
  entrainedAirPercent: 0.1,
  deliveredQualityPercent: 98,
  leakDetected: false,
};

export function bodyPreparationFixture(): HmiBodyPreparationState {
  return {
    recipeBasis: "public reference process",
    simulatedMsPerProcessMinute: 100,
    slip: {
      train: train("slip"),
      batchMassKg: 980,
      targetBatchMassKg: 1_000,
      solidsPercent: 68,
      densityKgL: 1.78,
      highShearViscosityMpaS: 420,
      lowShearViscosityMpaS: 920,
      thixotropicIndex: 2.2,
      structureParameter: 0.72,
      temperatureC: 40,
      mixerLevelPercent: 74,
      conditioningTankLevelPercent: 62,
      transferFlowLMin: 95,
      specificEnergyKwhT: 12,
      residue44umPercent: 1.2,
      medianParticleUm: 8,
      castingRateGCm2Min: 0.31,
      qualityIndex: 94,
      qualityReleased: true,
      ingredients: [{ id: "clay", label: "Clay", targetKg: 600, actualKg: 598 }],
      qualityChecks: [{ id: "density", label: "Density", value: 1.78, unit: "kg/L", minimum: 1.7, maximum: 1.85, withinLimit: true }],
      water: quality,
      downstream: {
        fillingFlowFactor: 0.98,
        castingRateGCm2Min: 0.31,
        predictedGreenMoisturePercent: 18,
        predictedDryingShrinkagePercent: 4.2,
        dryingEnergyFactor: 1.01,
        greenStrengthIndex: 92,
        firedDefectRiskPercent: 2.1,
      },
    },
    water: {
      train: train("water"),
      rawTankL: 8_000,
      treatedTankL: 6_500,
      feedFlowLMin: 120,
      permeateFlowLMin: 90,
      rejectFlowLMin: 30,
      mediaFilterDpBar: 0.4,
      roRecoveryPercent: 75,
      raw: { ...quality, turbidityNtu: 3.2 },
      product: quality,
    },
    returnWater: {
      train: train("return-water"),
      activeStream: "body-return",
      bodyEqualizationL: 4_000,
      glazeEqualizationL: 2_000,
      bodyReuseTankL: 3_400,
      glazeReuseTankL: 1_500,
      feedFlowLMin: 80,
      clarifiedFlowLMin: 72,
      sludgeCakeKg: 120,
      influentTurbidityNtu: 35,
      effluentTurbidityNtu: 1.5,
      bodyReuseQuality: quality,
      glazeReuseQuality: { ...quality, glazeContaminationPercent: 0.02 },
    },
    glaze: {
      train: train("glaze"),
      powderMassKg: 490,
      targetPowderMassKg: 500,
      batchMassKg: 750,
      solidsPercent: 66,
      densityKgL: 1.72,
      fordCupSeconds: 28,
      medianParticleUm: 7,
      residue63umPercent: 0.8,
      millEnergyKwhT: 18,
      storageLevelPercent: 55,
      transferFlowLMin: 42,
      settlingRiskPercent: 4,
      qualityIndex: 96,
      qualityReleased: true,
      ingredients: [{ id: "frit", label: "Frit", targetKg: 400, actualKg: 399 }],
      qualityChecks: [{ id: "flow", label: "Cup flow", value: 28, unit: "s", minimum: 25, maximum: 31, withinLimit: true }],
      water: quality,
    },
    pipelines: {
      waterToSlip: pipeline,
      waterToGlaze: { ...pipeline, outletFlowLMin: 97 },
      slipToForming: { ...pipeline, entrainedAirPercent: 0.2 },
      glazeToGlazing: { ...pipeline, outletFlowLMin: 96 },
    },
    waterNetworks: {
      heartbeatIntervalMs: 1_000,
      heartbeatTimeoutMs: 3_000,
      pumps: [
        { id: "area-01-wd-pmp-01", label: "Industrial duty pump", groupId: "industrial-a", service: "industrial", preferredDuty: true, commanded: true, runningFeedback: true, heartbeatSequence: 12, heartbeatAgeMs: 80, heartbeatOk: true, maintenance: "normal" },
        { id: "area-01-wd-pmp-02", label: "Industrial standby pump", groupId: "industrial-a", service: "industrial", preferredDuty: false, commanded: false, runningFeedback: false, heartbeatSequence: 11, heartbeatAgeMs: 3_500, heartbeatOk: false, maintenance: "required" },
        { id: "area-01-rc-pmp-01", label: "Return duty pump", groupId: "return-a", service: "return", preferredDuty: true, commanded: true, runningFeedback: true, heartbeatSequence: 8, heartbeatAgeMs: 50, heartbeatOk: true, maintenance: "normal" },
        { id: "area-01-rc-pmp-02", label: "Return standby pump", groupId: "return-a", service: "return", preferredDuty: false, commanded: false, runningFeedback: false, heartbeatSequence: 8, heartbeatAgeMs: 60, heartbeatOk: true, maintenance: "dispatched" },
      ],
      routes: [
        { id: "industrial-slip", label: "Slip branch", network: "industrial", source: "Treated tank", destination: "Slip", pumpGroup: "industrial-a", demanded: true, available: true, inletFlowLMin: 100, outletFlowLMin: 98, inletPressureBar: 4, outletPressureBar: 3.7, leakDetected: false, quality },
        { id: "industrial-glaze", label: "Glaze branch", network: "industrial", source: "Treated tank", destination: "Glaze", pumpGroup: "industrial-a", demanded: false, available: true, inletFlowLMin: 50, outletFlowLMin: 48, inletPressureBar: 4, outletPressureBar: 3.6, leakDetected: true, quality },
        { id: "return-body", label: "Body return", network: "return", source: "Forming", destination: "Recovery", pumpGroup: "return-a", demanded: true, available: true, inletFlowLMin: 70, outletFlowLMin: 68, inletPressureBar: 2.8, outletPressureBar: 2.5, leakDetected: false, quality },
      ],
    },
  };
}

function mould(index: number): HmiMouldProcessState {
  const id = `mould-${String(index).padStart(2, "0")}`;
  return {
    target: id,
    label: `Mould ${index}`,
    phase: index === 2 ? "faulted" : "pressure",
    operatingState: index === 2 ? "faulted" : "producing",
    running: index !== 2,
    productionEnabled: true,
    paused: false,
    stopRequest: index === 3 ? "after-phase" : null,
    phaseElapsedMs: 2_000,
    scanCount: 100,
    cycleCount: index * 4,
    fault: index === 2 ? "mould-overpressure" : null,
    targetDurationMs: 10_000,
    castingPressureBar: 4.5,
    setpointsBound: true,
    controlCabinet: { remoteIo: `rio-${index}`, enclosureRating: "IP54", controlVoltageVdc: 24, safetyRelay: `relay-${index}`, modules: ["DI", "DO"] },
    utilityCabinet: { actuator: `manifold-${index}`, enclosureRating: "IP54", controlVoltageVdc: 24, isolationState: "isolated", activeState: "pressurized", circuits: [{ id: "air", label: "Compressed air", medium: "air", source: "header", nominalPressure: 6, state: "open" }] },
    phases,
  };
}

export function hmiSnapshot(id: string): HmiSnapshot {
  const controllers: Record<string, string> = {
    "area-01-wt-hmi-01": "area-01-wt-vplc-01",
    "area-01-wd-hmi-01": "area-01-wd-vplc-01",
    "area-01-rw-hmi-01": "area-01-rw-vplc-01",
    "area-01-rc-hmi-01": "area-01-rc-vplc-01",
    "area-01-gl-hmi-01": "area-01-gl-vplc-01",
  };
  const body = id.startsWith("area-01-");
  const robot = id === "area-02-joystick-01";
  const machine = id === "area-02-machine-pc-01";
  const mouldPanel = /^area-02-hmi-0[1-4]$/u.test(id);
  const stationType = robot ? "robot-joystick" : machine ? "machine-pc" : mouldPanel ? "mould-panel" : null;
  const selectedMode = robot ? "setup" : "manual";
  return {
    schemaVersion: "0.15.0",
    id,
    label: id,
    environment: body ? "Body Preparation" : "Forming",
    zone: "OT",
    role: "operator interface",
    interfaceKind: machine ? "scada-workstation" : "hmi",
    controller: controllers[id] ?? (body ? "area-01-vplc-01" : "area-02-plc-01"),
    remoteIo: "rio-01",
    remoteIoStations: ["rio-01", "rio-02"],
    permissions: ["start-mould", "reset-safety", "inject-faults"],
    sequence: 42,
    controlProgram: { language: "structured-text", program: "FORMING", task: "cyclic", sourcePath: "forming.st", bindingPath: "forming.yaml", revision: "test", currentStep: 4, scanIntervalMs: 20, watchdogMs: 100 },
    controlStation: stationType ? { stationType, target: mouldPanel ? `mould-0${id.at(-1)}` : robot ? "robot-01" : "forming-cell", positions: ["manual", "auto", "setup"], selectedMode, setupAuthenticated: robot, sensorBypassActive: robot, bypassedPermissives: ["pickup-clear"], retainedProtections: ["hard-limit"] } : null,
    stationStatus: [1, 2, 3, 4].map((index) => ({ stationId: `hmi-${index}`, label: `Mould ${index}`, stationType: "mould-panel", target: `mould-0${index}`, selectedMode: index === 1 ? "manual" : "auto", setupAuthenticated: false, sensorBypassActive: false })),
    parameters: [{ id: "pressure", label: "Casting pressure", target: "mould-01", unit: "bar", minimum: 1, maximum: 8, step: 0.1, value: 4.5 }],
    recipes: [{ id: "standard", label: "Standard", description: "Reference recipe" }],
    activeRecipe: "standard",
    process: { model: body ? "body-preparation" : "forming", phase: "pressure", running: true, phaseElapsedMs: 2_000, scanCount: 100, cycleCount: 4, fault: null, phases },
    bodyPreparation: body ? bodyPreparationFixture() : null,
    moulds: [1, 2, 3, 4].map(mould),
    robot: robot ? robotFixture() : null,
    guardedCell: {
      guard: { safetyComponent: "safety-01", positionSensor: "gate-closed", position: "closed", closedPermissive: true, resetRequired: true },
      handoffStations: [1, 2, 3, 4].map((index) => ({ mould: `mould-0${index}`, actuator: `handoff-${index}`, state: index === 1 ? "moving-to-operator" : "operator-side", progressPercent: index * 20, inCellSensor: `in-${index}`, operatorSideSensor: `out-${index}`, inCellConfirmed: false, operatorSideConfirmed: true, piecePresent: true })),
    },
    supervisory: machine ? supervisoryFixture() : null,
    signals: [
      { componentId: "pt-01", label: "Pressure", tag: "area-02-pt-02", unit: "bar", minimum: 0, maximum: 10, value: 4.5, qualityGood: true, timestampMs: 100 },
      { componentId: "gate", label: "Gate closed", tag: "gate-closed", unit: "state", minimum: 0, maximum: 1, value: 1, qualityGood: false, timestampMs: 100 },
      { componentId: "level", label: "Tank level", tag: "area-02-lt-01", unit: "percent", minimum: 0, maximum: 100, value: 72, qualityGood: true, timestampMs: 100 },
      { componentId: "temperature", label: "Tank temperature", tag: "area-02-tt-01", unit: "degC", minimum: 0, maximum: 80, value: 40, qualityGood: true, timestampMs: 100 },
    ],
    actuators: [{ componentId: "valve-01", label: "Air valve", commandTag: "area-02-m01-manifold-01-command", feedbackTag: "air-open", safeState: "closed", states: ["closed", "open"], currentState: "open" }, { componentId: "robot-01", label: "Robot", commandTag: "area-02-robot-01-command", feedbackTag: null, safeState: "stopped", states: ["stopped", "moving"], currentState: "stopped" }],
    safety: [{ componentId: "safety-01", label: "Cell safety", permissives: [{ tag: "gate-closed", satisfied: true }], tripLatched: true }],
    alarms: [{ id: "alarm-01", code: "CELL-001", source: "safety-01", message: "Fence reset required", severity: "trip", active: true, acknowledged: false, sequence: 1 }],
    audit: [{ sequence: 1, action: "set-mode", target: "mould-01", result: "applied" }],
  };
}

function robotFixture(): NonNullable<HmiSnapshot["robot"]> {
  return {
    coordinateSystem: "world",
    motionEnabled: true,
    pose,
    joints: [0, 10, 20, 30, 40, 50],
    gripperClosed: false,
    automaticCommand: "approaching",
    controllerState: "ready",
    activeUserFrame: "cell",
    activeTool: "gripper",
    activePayload: "green-piece",
    architecture: { controller: "robot-controller", manipulator: "six-axis-arm", pendant: "joystick", safetyInterface: "cell-safety", cellController: "forming-plc", servoAxes: 6, motionGroup: "group-1", interpolationCycleMs: 4 },
    frames: [{ id: "cell", label: "Cell", parent: null, pose }],
    payloads: [{ id: "green-piece", label: "Green piece", massKg: 12, centerOfMassMm: [0, 0, 120] }],
    tools: [{ id: "gripper", label: "Piece gripper", tcp: pose, payload: "green-piece" }],
    handoffs: [{ mould: "mould-01", program: "pick-01", userFrame: "cell", approachPosition: "m1-approach", pickupPosition: "m1-pick", handoffPosition: "m1-handoff", retreatPosition: "m1-retreat", pickupToleranceMm: 2, handoffToleranceMm: 3, orientationToleranceDeg: 1 }],
    cell: { activeMould: "mould-01", queuedMoulds: ["mould-02"], stage: "approaching", completedHandoffs: 8, activeProgram: "pick-01", faultCode: null, faultMessage: null },
    motion: { active: true, kind: "linear", progressPercent: 45, elapsedMs: 900, durationMs: 2_000, speedPercent: 25, targetPose: pose, targetJoints: [1, 2, 3, 4, 5, 6] },
    program: { name: "forming-pick", sourcePath: "forming.g", revision: "test", running: false, paused: true, activeLine: 20, cycleCount: 4, source: "N10 G1 X100\nN20 M30", lines: [{ number: 10, source: "G1 X100", operation: "move", active: false }, { number: 20, source: "M30", operation: "end", active: true }] },
    taughtPositions: [{ id: "m1-pick", label: "Mould 1 pickup", pose }],
    workspace: { minimum: { x: -1_000, y: -1_000, z: 0, w: -180, p: -180, r: -180 }, maximum: { x: 1_000, y: 1_000, z: 2_000, w: 180, p: 180, r: 180 }, jointMinimum: [-180, -90, -180, -180, -180, -360], jointMaximum: [180, 90, 180, 180, 180, 360] },
  };
}

function supervisoryFixture(): NonNullable<HmiSnapshot["supervisory"]> {
  return {
    namespace: "forming",
    modelId: "forming-cell",
    repository: { id: "repo", revision: "2", deployedRevision: "1", synchronized: false },
    templates: [{ id: "mould", label: "Mould", parent: null, attributes: ["pressure"], alarmCapable: true, historyCapable: true }],
    assets: [{ id: "mould-01", label: "Mould 1", template: "mould", parent: "forming", components: ["pt-01"], historizedTags: ["pressure"] }],
    deploymentNodes: [{ id: "primary", label: "Primary", host: "machine-pc", role: "runtime", state: "active", redundancyGroup: null }],
    identity: { user: "operator", role: "process-operator", authentication: "local", permissions: ["operate"] },
    tags: [{ tag: "pressure", value: 4.5, unit: "bar", quality: "good", timestampMs: 100, samples: [{ timestampMs: 0, value: 4, qualityGood: true }, { timestampMs: 100, value: 4.5, qualityGood: true }] }],
    events: [{ sequence: 1, category: "alarm", source: "mould-01", message: "Pressure warning", state: "active" }],
  };
}

export function workstationProfile(id = "customer-pc-01"): WorkstationProfile {
  return { schemaVersion: "0.4.0", id, label: id, kind: "workstation", site: "customer", environment: "Customer LAN", zone: "inside", role: "customer", hostname: id, browserHome: "https://shop.example/", defaultGateway: "192.168.0.1", dnsServers: ["203.0.113.53"], interfaces: [{ id: "eth0", hardware: "ethernet", macAddress: "02:00:00:00:00:01", addresses: ["192.168.0.10/24"], administrativeState: "up", operationalState: "up", speedMbps: 1_000, mtu: 1_500 }], applications: ["browser", "terminal"] };
}

export function workstationReport(id = "customer-pc-01"): WorkstationActionReport {
  return { schemaVersion: "0.4.0", workstationId: id, action: "browser", status: "succeeded", title: "Request completed", output: ["resolved shop.example", "HTTP 200"], clearOutput: false, browser: { url: "https://shop.example/", method: "GET", requestBodyBytes: 0, host: "shop.example", path: "/", resolvedAddress: "203.0.113.10", resolutionSource: "dns-query", gateway: "192.168.0.1", forwardedTo: "203.0.113.10", response: { status: 200, document: { title: "Shop", heading: "Catalog", body: "Available products" } }, outcome: "responded" }, simulations: [scenarioReport()], networkState: { active: true, simulatedTimeMs: 50, arpEntries: [{ address: "192.168.0.1", macAddress: "02:00:00:00:00:fe", interface: "eth0", remainingTtlMs: 30_000 }], patTranslations: 1, devices: [{ id: "router", kind: "router", supportsMacTable: true, supportsNeighbors: true, supportsPat: true, supportsFirewallSessions: true, macTable: [{ vlan: 10, macAddress: "02:00:00:00:00:01", interface: "g0/0", remainingTtlMs: 20_000 }], neighbors: [{ address: "192.168.0.10", macAddress: "02:00:00:00:00:01", interface: "g0/0", state: "reachable", remainingTtlMs: 20_000 }], patTranslations: [{ protocol: "tcp", internalAddress: "192.168.0.10", internalToken: 50_000, externalAddress: "203.0.113.2", externalToken: 40_000, remoteAddress: "203.0.113.10", remotePort: 443, remainingTtlMs: 20_000 }], firewallSessions: [{ protocol: "tcp", sourceAddress: "192.168.0.10", sourcePort: 50_000, destinationAddress: "203.0.113.10", destinationPort: 443, remainingTtlMs: 20_000 }] }] } };
}

const packet: ScenarioPacket = { source_ip: "192.168.0.10", destination_ip: "203.0.113.10", ttl: 64, wire_length_bytes: 128, transport: { protocol: "tcp", source_port: 50_000, destination_port: 443, syn: true, ack: false, fin: false, rst: false }, application: { kind: "http-request", method: "get", host: "shop.example", path: "/", body: null, body_bytes: 0 } };
const expectation = { component: "web-server", outcome: "delivered" as const, service: "https", target: "web-server", reason_contains: null };

export function scenarioSummary(): ScenarioSummary {
  return { schema_version: "0.15.0", id: "customer-web", label: "Customer web", summary: "Customer reaches the public service", category: "network", participants: ["customer-pc-01", "edge-router", "web-server"], source: "customer-pc-01", packet, connection_states: [{ id: "wan", label: "WAN", endpoint_a: "edge-router", endpoint_b: "web-server", configured_operational: true, operational: true }], first_hop_states: [{ appliance: "edge-router", interface: "outside", role: "active", protocol: "vrrp", group: 1, virtual_ip: "203.0.113.1", virtual_mac: "00:00:5e:00:01:01", priority: 110, preempt: true, configured_role: "active" }], firewall_ha_states: [{ appliance: "firewall-a", role: "active", peer: "firewall-b", domain: "edge", configured_role: "active", sync_interface: "sync", sync_connection: "wan", sync_operational: true, session_sync: true, heartbeat_interval_ms: 500, failure_hold_ms: 1_500, monitored_interfaces: ["outside"] }, { appliance: "firewall-b", role: "standby", peer: "firewall-a", domain: "edge", configured_role: "standby", sync_interface: "sync", sync_connection: "wan", sync_operational: true, session_sync: true, heartbeat_interval_ms: 500, failure_hold_ms: 1_500, monitored_interfaces: ["outside"] }], link_aggregation_states: [{ appliance: "edge-router", interface: "port-channel1", connection: "wan", group: "po1", logical_id: "po1", protocol: "lacp", mode: "active", system_id: "edge", partner_system_id: "web", multi_chassis_domain: null, selected: true, collecting: true, distributing: true, bundle_operational: true, active_members: 2, minimum_active_members: 1, peer_forwarding: false }], spanning_tree_states: [{ appliance: "edge-router", interface: "outside", connection: "wan", protocol: "rapid-pvst", vlan: 10, root_bridge: "edge-router", root_path_cost: 0, port_path_cost: 4, role: "designated", state: "forwarding" }], recovery: { label: "Restore WAN", summary: "Return the WAN to service", connection_overrides: [{ connection: "wan", operational: true }], first_hop_overrides: [{ appliance: "edge-router", interface: "outside", role: "active" }], firewall_ha_overrides: [{ appliance: "firewall-a", role: "active" }], expectation }, continuity: null, ha_isolation: null, local_autonomy: null, expectation, security: { tactic: "initial-access", technique: "public-service", severity: "medium", detector: "ids", defender: "firewall-a", control: "allowlist" } };
}

export function scenarioReport(): ScenarioReport {
  const summary = scenarioSummary();
  return { schema_version: "0.15.0", scenario_id: summary.id, scenario_label: summary.label, status: "passed", expectation_mode: "recovery", expectation_met: true, duration_us: 2_500, appliance_count: 3, link_count: 2, packet, connection_states: summary.connection_states, first_hop_states: summary.first_hop_states, firewall_ha_states: summary.firewall_ha_states, link_aggregation_states: summary.link_aggregation_states, spanning_tree_states: summary.spanning_tree_states, expectation, statistics: { events: 4, transmissions: 1, media_transits: 1, deliveries: 1, drops: 1, observations: 0 }, http_response: { status: 200, document: { title: "Shop", heading: "Catalog", body: "Available products" } }, security: { ...summary.security!, schema_version: "0.1.0", scenario_id: summary.id, disposition: "prevented", source_ip: packet.source_ip, destination_ip: packet.destination_ip, observed_at_us: 2_000, evidence: "policy matched" }, continuity: null, ha_isolation: null, local_autonomy: null, trace: ["transmission", "media", "delivery", "drop"].map((kind, sequence) => ({ sequence, time_us: sequence * 800, component: sequence === 3 ? "firewall-a" : "edge-router", kind: kind as "transmission" | "media" | "delivery" | "drop", summary: `${kind} event`, egress: "outside", connection: "wan", peer: "web-server", source_ip: packet.source_ip, destination_ip: packet.destination_ip, protocol: "tcp" })) };
}

export function securitySession(id = "operations-soc-console-01"): SecurityConsoleSession {
  const event = scenarioReport().security!;
  return { schemaVersion: "0.1.0", consoleId: id, sequence: 1, activeCount: 1, acknowledgedCount: 0, events: [{ id: 1, receivedSequence: 1, acknowledged: false, event }] };
}

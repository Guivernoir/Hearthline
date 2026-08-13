import {
  findAppliancesForEnvironment,
  type FrontendAppliance,
} from "../../config/appliance-config";
import type { ViewMode } from "../../shared/types";
import type { ProcessEquipment, ProcessIconKey } from "../process-model";
import type { EquipmentPosition, EquipmentView } from "./process-area-layout";

export const FORMING_WORLD_WIDTH = 2310;
export const FORMING_LOGICAL_WORLD_HEIGHT = 2240;
export const FORMING_PHYSICAL_WORLD_HEIGHT = 1180;

const PHYSICAL_POSITIONS: Record<string, EquipmentPosition> = {
  "area-02-machine-pc-01": { x: 2070, y: 155 },
  "area-02-hmi-01": { x: 1770, y: 285 },
  "area-02-hmi-02": { x: 1770, y: 505 },
  "area-02-hmi-03": { x: 1770, y: 725 },
  "area-02-hmi-04": { x: 1770, y: 945 },
  "area-02-m01-rio-01": { x: 65, y: 195 },
  "area-02-m02-rio-01": { x: 65, y: 405 },
  "area-02-m03-rio-01": { x: 65, y: 615 },
  "area-02-m04-rio-01": { x: 65, y: 825 },
  "area-02-mould-01": { x: 530, y: 205 },
  "area-02-m02-mould-01": { x: 530, y: 425 },
  "area-02-m03-mould-01": { x: 530, y: 645 },
  "area-02-m04-mould-01": { x: 530, y: 865 },
  "area-02-robot-01": { x: 1110, y: 515 },
  "area-02-m01-handoff-01": { x: 1585, y: 205 },
  "area-02-m02-handoff-01": { x: 1585, y: 425 },
  "area-02-m03-handoff-01": { x: 1585, y: 645 },
  "area-02-m04-handoff-01": { x: 1585, y: 865 },
  "area-02-cell-guard-safe-01": { x: 1500, y: 1015 },
  "area-02-robot-controller-01": { x: 2070, y: 845 },
  "area-02-joystick-01": { x: 2070, y: 650 },
};

const CONTROL_POSITIONS: Record<ViewMode, Record<string, EquipmentPosition>> = {
  physical: {
    "area-02-vplc-01": { x: 45, y: 215 },
    "area-02-sw-01": { x: 245, y: 215 },
    "area-02-machine-pc-01": { x: 445, y: 215 },
    "area-02-supervisory-node-02": { x: 645, y: 215 },
    "area-02-rio-01": { x: 845, y: 215 },
  },
  logical: {
    "area-02-machine-pc-01": { x: 70, y: 215 },
    "area-02-vplc-01": { x: 300, y: 215 },
    "area-02-rio-01": { x: 530, y: 215 },
    "area-02-supervisory-node-02": { x: 760, y: 215 },
    "area-02-sw-01": { x: 1120, y: 215 },
  },
};

const SHARED_SUPPLY = [
  "area-02-lt-01",
  "area-02-dt-01",
  "area-02-vis-01",
  "area-02-tt-01",
  "area-02-pt-01",
  "area-02-ft-01",
  "area-02-slip-01",
  "area-02-ft-02",
  "area-02-ft-03",
  "area-02-pt-04",
  "area-02-vt-01",
  "area-02-water-01",
  "area-02-air-01",
  "area-02-vac-01",
];

const MOULD_ONE = [
  "area-02-hmi-01",
  "area-02-m01-rio-01",
  "area-02-m01-inc-01",
  "area-02-mt-02",
  "area-02-pos-01",
  "area-02-pos-02",
  "area-02-pt-02",
  "area-02-tt-02",
  "area-02-m01-manifold-01",
  "area-02-mould-01",
  "area-02-safe-01",
];

const ROBOT_STATION = [
  "area-02-cell-rio-01",
  "area-02-cell-gate-pos-01",
  "area-02-cell-guard-safe-01",
  "area-02-m01-handoff-01",
  "area-02-m01-handoff-in-01",
  "area-02-m01-handoff-out-01",
  "area-02-m02-handoff-01",
  "area-02-m02-handoff-in-01",
  "area-02-m02-handoff-out-01",
  "area-02-m03-handoff-01",
  "area-02-m03-handoff-in-01",
  "area-02-m03-handoff-out-01",
  "area-02-m04-handoff-01",
  "area-02-m04-handoff-in-01",
  "area-02-m04-handoff-out-01",
  "area-02-joystick-01",
  "area-02-robot-controller-01",
  "area-02-pos-03",
  "area-02-pe-01",
  "area-02-robot-01",
  "area-02-robot-safe-01",
];

const STATION_LEFT: Record<string, number> = {
  "station-mould-01": 480,
  "station-mould-02": 940,
  "station-mould-03": 1400,
  "station-mould-04": 1860,
};

export function buildFormingEquipment(viewMode: ViewMode): EquipmentView[] {
  return findAppliancesForEnvironment("Forming")
    .map((appliance) => {
      const position = positionFor(appliance, viewMode);
      return position
        ? { ...toProcessEquipment(appliance, viewMode), ...position }
        : null;
    })
    .filter((item): item is EquipmentView => item !== null);
}

function positionFor(
  appliance: FrontendAppliance,
  viewMode: ViewMode,
): EquipmentPosition | null {
  if (viewMode === "physical") {
    return PHYSICAL_POSITIONS[appliance.id] ?? null;
  }
  if (CONTROL_POSITIONS[viewMode][appliance.id]) {
    return CONTROL_POSITIONS[viewMode][appliance.id];
  }

  const supplyIndex = SHARED_SUPPLY.indexOf(appliance.id);
  if (supplyIndex >= 0) return stationGrid(50, 590, supplyIndex);

  const mouldOneIndex = MOULD_ONE.indexOf(appliance.id);
  if (mouldOneIndex >= 0) return stationGrid(STATION_LEFT["station-mould-01"], 590, mouldOneIndex);

  const robotIndex = ROBOT_STATION.indexOf(appliance.id);
  if (robotIndex >= 0) {
    return {
      x: 500 + (robotIndex % 8) * 220,
      y: 1690 + Math.floor(robotIndex / 8) * 135,
    };
  }

  for (let index = 2; index <= 4; index += 1) {
    const station = `station-mould-0${index}`;
    if (!appliance.tags.includes(station)) continue;
    const members = stationMembers(station);
    const memberIndex = members.findIndex((candidate) => candidate.id === appliance.id);
    return memberIndex >= 0
      ? stationGrid(STATION_LEFT[station], 590, memberIndex)
      : null;
  }

  return null;
}

function stationMembers(station: string) {
  return findAppliancesForEnvironment("Forming")
    .filter((appliance) => appliance.tags.includes(station))
    .sort(compareStationMembers);
}

function stationGrid(left: number, top: number, index: number): EquipmentPosition {
  return {
    x: left + (index % 2) * 205,
    y: top + Math.floor(index / 2) * 125,
  };
}

function compareStationMembers(left: FrontendAppliance, right: FrontendAppliance) {
  const rank = (appliance: FrontendAppliance) => {
    if (appliance.kind === "hmi") return 0;
    if (appliance.kind === "remote-io") return 1;
    if (appliance.kind === "field-sensor") return 2;
    if (appliance.kind === "field-actuator") return 3;
    return 4;
  };
  return rank(left) - rank(right) || left.id.localeCompare(right.id);
}

function toProcessEquipment(
  appliance: FrontendAppliance,
  viewMode: ViewMode,
): ProcessEquipment {
  const isField = ["field-sensor", "field-actuator", "safety-interface"].includes(
    appliance.kind,
  );
  const isOperator = appliance.behaviorFamily === "operator-interface";
  const controllerId = "area-02-vplc-01";
  const switchId = "area-02-sw-01";

  let upstream: string | null = null;
  if (appliance.kind === "virtual-plc") upstream = switchId;
  else if (appliance.kind === "robot-controller" || appliance.kind === "service-cluster") upstream = switchId;
  else if (isOperator || appliance.kind === "remote-io") upstream = controllerId;
  else if (isField) upstream = remoteIoFor(appliance);

  let physicalUpstream = upstream;
  if (isOperator || appliance.kind === "remote-io" || appliance.kind === "robot-controller") physicalUpstream = switchId;

  const physical = physicalPresentation(appliance);
  return {
    id: appliance.id,
    label: appliance.label,
    kind: displayKind(appliance),
    role: appliance.role,
    icon: iconFor(appliance),
    accent: accentFor(appliance),
    slot: slotFor(appliance),
    linkKind: appliance.kind === "safety-interface"
      ? "safety-status"
      : isField
        ? "io"
        : "ethernet",
    upstream,
    physicalUpstream,
    configRef: appliance.sourcePath,
    facts: [appliance.summary, ...appliance.behaviorFacts.slice(0, 3)],
    ...(viewMode === "physical" && physical ? { physical } : {}),
  };
}

function remoteIoFor(appliance: FrontendAppliance) {
  if (appliance.tags.includes("guarded-cell")) return "area-02-cell-rio-01";
  for (let index = 1; index <= 4; index += 1) {
    if (appliance.tags.includes(`station-mould-0${index}`)) {
      return `area-02-m0${index}-rio-01`;
    }
  }
  return "area-02-rio-01";
}

function physicalPresentation(appliance: FrontendAppliance) {
  const station = mouldStationNumber(appliance);
  if (appliance.kind === "hmi" && station) {
    return physicalDetails(appliance, `MOULD ${station} HMI`, "local mould operator panel");
  }
  if (appliance.kind === "remote-io" && station) {
    return physicalDetails(appliance, `MOULD ${station} CONTROL CABINET`, "mould control and remote-I/O cabinet");
  }
  if (appliance.id.includes("manifold") && station) {
    return physicalDetails(appliance, `MOULD ${station} UTILITY SECTION`, "mould-embedded air, water, vacuum, and slip valve section");
  }
  if (appliance.id.includes("handoff") && station) {
    return physicalDetails(appliance, `MOULD ${station} TRANSFER`, "guarded operator handoff shuttle");
  }
  if (appliance.id.includes("mould") && station) {
    return physicalDetails(appliance, `MOULD ${station} / EMBEDDED UTILITIES`, "pressure-casting mould with an embedded utility section");
  }
  if (appliance.id === "area-02-cell-guard-safe-01") {
    return physicalDetails(appliance, "CELL ACCESS GATE", "interlocked fence entrance and safety sensor");
  }
  if (appliance.id === "area-02-robot-01") {
    return physicalDetails(appliance, "DEMOULD ROBOT", "six-axis guarded-cell manipulator");
  }
  if (appliance.id === "area-02-robot-controller-01") {
    return physicalDetails(appliance, "ROBOT CONTROLLER", "robot controller and servo cabinet");
  }
  if (appliance.id === "area-02-joystick-01") {
    return physicalDetails(appliance, "ROBOT PENDANT", "robot operation and programming pendant");
  }
  if (appliance.kind === "virtual-plc") {
    return {
      label: "OT-vPLC-HOST-01 / 02",
      kind: "redundant industrial control compute",
      role: "Hosts AREA-02-vPLC-01 as an isolated real-time workload",
      icon: "server" as ProcessIconKey,
      configRefs: [
        "config/appliances/factory/platform/ot-vplc-host-01.yaml",
        "config/appliances/factory/platform/ot-vplc-host-02.yaml",
      ],
      facts: [
        "Factory-local Level 3 compute cluster",
        "Dedicated OT-AREA-02 network presented to the runtime",
        "The controller remains the sole standard process-control authority",
      ],
    };
  }
  if (appliance.kind === "scada-workstation") {
    return {
      label: "MACHINE PC",
      kind: "machine-integrated industrial PC",
      role: "Runs the embedded SCADA, recipe, parameter, trend, and audit services",
      icon: "monitor" as ProcessIconKey,
      configRefs: [appliance.sourcePath],
      facts: [
        "Installed at the forming machine",
        "Supervises all four mould stations",
        "Has no robot motion authority",
      ],
    };
  }
  return undefined;
}

function mouldStationNumber(appliance: FrontendAppliance) {
  const tag = appliance.tags.find((candidate) => /^station-mould-0[1-4]$/.test(candidate));
  return tag?.slice(-1) ?? null;
}

function physicalDetails(appliance: FrontendAppliance, label: string, kind: string) {
  return {
    label,
    kind,
    role: appliance.role,
    icon: iconFor(appliance),
    configRefs: [appliance.sourcePath],
    facts: [appliance.summary, ...appliance.behaviorFacts.slice(0, 3)],
  };
}

function displayKind(appliance: FrontendAppliance) {
  if (appliance.kind === "hmi") {
    return appliance.tags.includes("robot-joystick") ? "robot joystick" : "mould HMI";
  }
  if (appliance.kind === "remote-io") {
    return appliance.tags.some((tag) => tag.startsWith("station-mould"))
      ? "mould remote I/O"
      : "cell remote I/O";
  }
  const names: Record<string, string> = {
    "layer-2-switch": "industrial switch",
    "virtual-plc": "control runtime",
    "scada-workstation": "embedded SCADA PC",
    "field-sensor": "process input",
    "field-actuator": "process output",
    "safety-interface": "safety interface",
    "robot-controller": "robot controller cabinet",
    "service-cluster": "supervisory runtime node",
  };
  return names[appliance.kind] ?? appliance.kind;
}

function slotFor(appliance: FrontendAppliance): ProcessEquipment["slot"] {
  if (appliance.kind === "layer-2-switch") return "switch";
  if (appliance.kind === "virtual-plc") return "controller";
  if (appliance.kind === "robot-controller") return "controller";
  if (["hmi", "scada-workstation"].includes(appliance.kind)) return "hmi";
  if (appliance.kind === "remote-io") return "remote-io";
  if (appliance.kind === "field-sensor") return "sensor-a";
  if (appliance.kind === "field-actuator") return "actuator-a";
  return "safety";
}

function iconFor(appliance: FrontendAppliance): ProcessIconKey {
  if (appliance.kind === "layer-2-switch") return "network";
  if (appliance.kind === "virtual-plc") return "cpu";
  if (appliance.kind === "robot-controller") return "cpu";
  if (appliance.kind === "service-cluster") return "server";
  if (appliance.tags.includes("robot-joystick")) return "joystick";
  if (["hmi", "scada-workstation"].includes(appliance.kind)) return "monitor";
  if (appliance.kind === "remote-io") return "remote-io";
  if (appliance.kind === "safety-interface") return "shield";
  if (appliance.tags.includes("temperature")) return "thermometer";
  if (appliance.tags.includes("pressure")) return "gauge";
  if (appliance.tags.includes("water")) return "droplets";
  if (appliance.tags.includes("vacuum")) return "wind";
  if (appliance.tags.includes("position")) return "scan";
  if (appliance.tags.includes("part-detection")) return "eye";
  if (appliance.tags.includes("robot")) return "robot";
  if (appliance.tags.includes("slip") || appliance.tags.includes("viscosity")) return "droplets";
  if (appliance.kind === "field-actuator") return "valve";
  return "boxes";
}

function accentFor(appliance: FrontendAppliance) {
  if (appliance.kind === "field-sensor") return "#51704c";
  if (appliance.kind === "field-actuator") return "#b65034";
  if (appliance.kind === "safety-interface") return "#9e3f2f";
  if (appliance.kind === "scada-workstation") return "#3f6488";
  if (appliance.kind === "robot-controller") return "#725e2a";
  if (appliance.kind === "service-cluster") return "#566a62";
  if (appliance.tags.includes("robot-joystick")) return "#7b5b91";
  if (appliance.kind === "hmi") return "#426d9d";
  if (appliance.kind === "virtual-plc" || appliance.kind === "remote-io") return "#267168";
  return "#3567a6";
}

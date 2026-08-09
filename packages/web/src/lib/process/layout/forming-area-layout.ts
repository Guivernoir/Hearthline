import {
  findAppliancesForEnvironment,
  type FrontendAppliance,
} from "../../config/appliance-config";
import type { ViewMode } from "../../shared/types";
import type { ProcessEquipment, ProcessIconKey } from "../process-model";
import type { EquipmentPosition, EquipmentView } from "./process-area-layout";

export const FORMING_WORLD_WIDTH = 1900;
export const FORMING_WORLD_HEIGHT = 1360;

type FormingModule =
  | "module-slip-supply"
  | "module-pressure-casting"
  | "module-robotic-demould"
  | "module-utilities";

interface ModuleLayout {
  hmi: EquipmentPosition;
  equipment: EquipmentPosition[];
}

const modules: Record<FormingModule, ModuleLayout> = {
  "module-slip-supply": {
    hmi: { x: 155, y: 575 },
    equipment: fieldGrid(65),
  },
  "module-pressure-casting": {
    hmi: { x: 615, y: 575 },
    equipment: fieldGrid(525),
  },
  "module-robotic-demould": {
    hmi: { x: 1075, y: 575 },
    equipment: fieldGrid(985),
  },
  "module-utilities": {
    hmi: { x: 1535, y: 575 },
    equipment: fieldGrid(1445),
  },
};

const controlPositions: Record<ViewMode, Record<string, EquipmentPosition>> = {
  physical: {
    "virtual-plc": { x: 65, y: 210 },
    "layer-2-switch": { x: 315, y: 210 },
    "scada-workstation": { x: 565, y: 210 },
    "remote-io": { x: 815, y: 210 },
  },
  logical: {
    "scada-workstation": { x: 65, y: 210 },
    "virtual-plc": { x: 315, y: 210 },
    "remote-io": { x: 565, y: 210 },
    "layer-2-switch": { x: 1125, y: 210 },
  },
};

export function buildFormingEquipment(viewMode: ViewMode): EquipmentView[] {
  const appliances = findAppliancesForEnvironment("Forming");
  const moduleMembers = new Map<FormingModule, FrontendAppliance[]>();

  for (const module of Object.keys(modules) as FormingModule[]) {
    moduleMembers.set(
      module,
      appliances
        .filter((appliance) => appliance.tags.includes(module))
        .sort(compareModuleMembers),
    );
  }

  return appliances
    .map((appliance) => {
      const position = positionFor(appliance, moduleMembers, viewMode);
      if (!position) return null;
      return {
        ...toProcessEquipment(appliance, viewMode),
        ...position,
      };
    })
    .filter((item): item is EquipmentView => item !== null);
}

function fieldGrid(left: number): EquipmentPosition[] {
  return [
    { x: left, y: 705 },
    { x: left + 200, y: 705 },
    { x: left, y: 830 },
    { x: left + 200, y: 830 },
    { x: left, y: 955 },
    { x: left + 200, y: 955 },
    { x: left, y: 1080 },
    { x: left + 200, y: 1080 },
  ];
}

function positionFor(
  appliance: FrontendAppliance,
  moduleMembers: Map<FormingModule, FrontendAppliance[]>,
  viewMode: ViewMode,
) {
  const module = moduleFor(appliance);
  if (module) {
    if (appliance.kind === "hmi") return modules[module].hmi;
    const members = moduleMembers
      .get(module)
      ?.filter((member) => member.kind !== "hmi");
    const index = members?.findIndex((member) => member.id === appliance.id) ?? -1;
    return index >= 0 ? modules[module].equipment[index] : null;
  }
  return controlPositions[viewMode][appliance.kind] ?? null;
}

function moduleFor(appliance: FrontendAppliance) {
  return (Object.keys(modules) as FormingModule[]).find((module) =>
    appliance.tags.includes(module),
  );
}

function compareModuleMembers(left: FrontendAppliance, right: FrontendAppliance) {
  const rank = (appliance: FrontendAppliance) => {
    if (appliance.kind === "hmi") return 0;
    if (appliance.kind === "field-sensor") return 1;
    if (appliance.kind === "field-actuator") return 2;
    return 3;
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
  const remoteIoId = "area-02-rio-01";

  let upstream: string | null = null;
  if (appliance.kind === "virtual-plc") upstream = switchId;
  else if (isOperator || appliance.kind === "remote-io") upstream = controllerId;
  else if (isField) upstream = remoteIoId;

  let physicalUpstream = upstream;
  if (isOperator || appliance.kind === "remote-io") physicalUpstream = switchId;

  const physical = appliance.kind === "virtual-plc"
    ? {
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
      }
    : undefined;

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

function displayKind(appliance: FrontendAppliance) {
  const names: Record<string, string> = {
    "layer-2-switch": "industrial switch",
    "virtual-plc": "control runtime",
    "scada-workstation": "SCADA workstation",
    hmi: "module HMI",
    "remote-io": "distributed I/O",
    "field-sensor": "process input",
    "field-actuator": "process output",
    "safety-interface": "safety status",
  };
  return names[appliance.kind] ?? appliance.kind;
}

function slotFor(appliance: FrontendAppliance): ProcessEquipment["slot"] {
  if (appliance.kind === "layer-2-switch") return "switch";
  if (appliance.kind === "virtual-plc") return "controller";
  if (["hmi", "scada-workstation"].includes(appliance.kind)) return "hmi";
  if (appliance.kind === "remote-io") return "remote-io";
  if (appliance.kind === "field-sensor") return "sensor-a";
  if (appliance.kind === "field-actuator") return "actuator-a";
  return "safety";
}

function iconFor(appliance: FrontendAppliance): ProcessIconKey {
  if (appliance.kind === "layer-2-switch") return "network";
  if (appliance.kind === "virtual-plc") return "cpu";
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
  if (appliance.kind === "hmi") return "#426d9d";
  if (appliance.kind === "virtual-plc" || appliance.kind === "remote-io") return "#267168";
  return "#3567a6";
}

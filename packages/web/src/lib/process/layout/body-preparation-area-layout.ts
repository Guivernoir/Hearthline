import {
  findAppliancesForEnvironment,
  type FrontendAppliance,
} from "../../config/appliance-config";
import type { ViewMode } from "../../shared/types";
import type { ProcessEquipment, ProcessIconKey } from "../process-model";
import type { EquipmentPosition, EquipmentView } from "./process-area-layout";

export type BodyPreparationScope = "slip" | "water" | "glaze";

export interface BodyPreparationScopeMetadata {
  label: string;
  zone: string;
  subtitle: string;
  worldWidth: number;
  physicalWorldHeight: number;
  logicalWorldHeight: number;
}

const SCOPE_METADATA: Record<BodyPreparationScope, BodyPreparationScopeMetadata> = {
  slip: {
    label: "Slip Preparation",
    zone: "OT-AREA-01 / BODY",
    subtitle: "Dry batching, blunging, conditioning, release, and controlled transfer to Forming",
    worldWidth: 1900,
    physicalWorldHeight: 1200,
    logicalWorldHeight: 1900,
  },
  water: {
    label: "Water Preparation and Distribution",
    zone: "OT-AREA-01 / UTILITIES",
    subtitle: "Industrial-water treatment and distribution with segregated return-water recovery",
    worldWidth: 2600,
    physicalWorldHeight: 1850,
    logicalWorldHeight: 3150,
  },
  glaze: {
    label: "Glaze Preparation",
    zone: "OT-AREA-01 / GLAZE",
    subtitle: "Powder batching, wet milling, finishing, release, storage, and glazing transfer",
    worldWidth: 2000,
    physicalWorldHeight: 1200,
    logicalWorldHeight: 1500,
  },
};

const SCOPE_RIOS: Record<BodyPreparationScope, string[]> = {
  slip: ["area-01-rio-01", "area-01-rio-02"],
  water: ["area-01-rio-03", "area-01-rio-06", "area-01-rio-04", "area-01-rio-07"],
  glaze: ["area-01-rio-05"],
};

const SCOPE_CONTROL_IDS: Record<BodyPreparationScope, string[]> = {
  slip: ["area-01-hmi-01", "area-01-vplc-01", "area-01-sw-01"],
  water: [
    "area-01-wt-hmi-01", "area-01-wt-vplc-01", "area-01-wd-hmi-01", "area-01-wd-vplc-01",
    "area-01-rw-hmi-01", "area-01-rw-vplc-01", "area-01-rc-hmi-01", "area-01-rc-vplc-01",
    "area-01-wt-sw-01",
  ],
  glaze: ["area-01-gl-hmi-01", "area-01-gl-vplc-01", "area-01-gl-sw-01"],
};

const CONTROL_POSITIONS: Record<string, EquipmentPosition> = {
  "area-01-hmi-01": { x: 70, y: 225 },
  "area-01-vplc-01": { x: 330, y: 225 },
  "area-01-sw-01": { x: 590, y: 225 },
  "area-01-rio-01": { x: 920, y: 225 },
  "area-01-rio-02": { x: 1310, y: 225 },
  "area-01-rio-03": { x: 1700, y: 225 },
  "area-01-rio-04": { x: 2090, y: 225 },
  "area-01-rio-05": { x: 2480, y: 225 },
  "area-01-wt-hmi-01": { x: 70, y: 410 },
  "area-01-wt-vplc-01": { x: 330, y: 410 },
  "area-01-wt-sw-01": { x: 590, y: 410 },
  "area-01-gl-hmi-01": { x: 70, y: 595 },
  "area-01-gl-vplc-01": { x: 330, y: 595 },
  "area-01-gl-sw-01": { x: 590, y: 595 },
};

const PHYSICAL_POSITIONS: Record<string, EquipmentPosition> = {
  "area-01-wt-lt-01": { x: 90, y: 275 },
  "area-01-wt-pmp-01": { x: 335, y: 290 },
  "area-01-wt-fil-01": { x: 90, y: 545 },
  "area-01-wt-carb-01": { x: 325, y: 545 },
  "area-01-wt-soft-01": { x: 90, y: 800 },
  "area-01-wt-ro-01": { x: 325, y: 800 },
  "area-01-wt-lt-02": { x: 530, y: 785 },
  "area-01-feed-01": { x: 735, y: 265 },
  "area-01-feed-02": { x: 950, y: 265 },
  "area-01-feed-03": { x: 1165, y: 265 },
  "area-01-feed-04": { x: 1380, y: 265 },
  "area-01-wit-01": { x: 950, y: 470 },
  "area-01-dp-01": { x: 735, y: 650 },
  "area-01-ag-01": { x: 990, y: 670 },
  "area-01-scr-01": { x: 735, y: 900 },
  "area-01-mag-01": { x: 965, y: 900 },
  "area-01-ag-02": { x: 1220, y: 745 },
  "area-01-pmp-01": { x: 1415, y: 930 },
  "area-01-gl-wit-01": { x: 1640, y: 300 },
  "area-01-gl-xv-01": { x: 1875, y: 300 },
  "area-01-gl-dp-01": { x: 2100, y: 300 },
  "area-01-gl-mill-01": { x: 1810, y: 575 },
  "area-01-gl-scr-01": { x: 1615, y: 855 },
  "area-01-gl-mag-01": { x: 1845, y: 855 },
  "area-01-gl-ag-01": { x: 2100, y: 700 },
  "area-01-gl-pmp-01": { x: 2290, y: 920 },
  "area-01-hmi-01": { x: 2650, y: 235 },
  "area-01-rio-01": { x: 2500, y: 490 },
  "area-01-rio-02": { x: 2700, y: 490 },
  "area-01-rio-03": { x: 2500, y: 710 },
  "area-01-rio-04": { x: 2700, y: 710 },
  "area-01-rio-05": { x: 2600, y: 925 },
  "area-01-rw-lt-01": { x: 120, y: 1300 },
  "area-01-rw-lt-02": { x: 400, y: 1300 },
  "area-01-rw-clar-01": { x: 760, y: 1300 },
  "area-01-rw-fp-01": { x: 1070, y: 1300 },
  "area-01-rw-lt-03": { x: 1450, y: 1300 },
  "area-01-rw-lt-04": { x: 1740, y: 1300 },
  "area-01-rw-xv-01": { x: 2040, y: 1320 },
};

const SCOPED_PHYSICAL_POSITIONS: Record<
  BodyPreparationScope,
  Record<string, EquipmentPosition>
> = {
  slip: {
    "area-01-feed-01": { x: 70, y: 245 },
    "area-01-feed-02": { x: 280, y: 245 },
    "area-01-feed-03": { x: 490, y: 245 },
    "area-01-feed-04": { x: 700, y: 245 },
    "area-01-wit-01": { x: 930, y: 245 },
    "area-01-dp-01": { x: 1150, y: 245 },
    "area-01-ag-01": { x: 1390, y: 245 },
    "area-01-scr-01": { x: 170, y: 555 },
    "area-01-mag-01": { x: 430, y: 555 },
    "area-01-ag-02": { x: 730, y: 555 },
    "area-01-pmp-01": { x: 1040, y: 555 },
    "area-01-rio-01": { x: 1450, y: 720 },
    "area-01-rio-02": { x: 1660, y: 720 },
    "area-01-hmi-01": { x: 1375, y: 910 },
    "area-01-sw-01": { x: 1600, y: 910 },
  },
  water: {
  },
  glaze: {
    "area-01-gl-wit-01": { x: 70, y: 245 },
    "area-01-gl-xv-01": { x: 300, y: 245 },
    "area-01-gl-dp-01": { x: 530, y: 245 },
    "area-01-gl-mill-01": { x: 770, y: 245 },
    "area-01-gl-scr-01": { x: 1020, y: 245 },
    "area-01-gl-mag-01": { x: 1260, y: 245 },
    "area-01-gl-ag-01": { x: 1500, y: 245 },
    "area-01-gl-pmp-01": { x: 1500, y: 600 },
    "area-01-rio-05": { x: 1740, y: 600 },
    "area-01-gl-hmi-01": { x: 1510, y: 850 },
    "area-01-gl-sw-01": { x: 1740, y: 850 },
  },
};

const WATER_CONTROL_CELLS = [
  { hmi: "area-01-wt-hmi-01", controller: "area-01-wt-vplc-01", rio: "area-01-rio-03" },
  { hmi: "area-01-wd-hmi-01", controller: "area-01-wd-vplc-01", rio: "area-01-rio-06" },
  { hmi: "area-01-rw-hmi-01", controller: "area-01-rw-vplc-01", rio: "area-01-rio-04" },
  { hmi: "area-01-rc-hmi-01", controller: "area-01-rc-vplc-01", rio: "area-01-rio-07" },
] as const;

const WATER_PHYSICAL_ROWS = [
  ["area-01-wt-lt-01", "area-01-wt-pmp-01", "area-01-wt-fil-01", "area-01-wt-carb-01", "area-01-wt-soft-01", "area-01-wt-ro-01", "area-01-wt-lt-02"],
  ["area-01-wd-pmp-01a", "area-01-wd-pmp-01b", "area-01-wd-pmp-02a", "area-01-wd-pmp-02b", "area-01-wd-pmp-03a", "area-01-wd-pmp-03b", "area-01-wd-pmp-04a", "area-01-wd-pmp-04b"],
  ["area-01-rw-lt-01", "area-01-rw-lt-02", "area-01-rw-clar-01", "area-01-rw-fp-01", "area-01-rw-lt-03", "area-01-rw-lt-04", "area-01-rw-xv-01"],
  ["area-01-rc-pmp-01a", "area-01-rc-pmp-01b", "area-01-rc-pmp-02a", "area-01-rc-pmp-02b", "area-01-rc-pmp-03a", "area-01-rc-pmp-03b", "area-01-rc-pmp-04a", "area-01-rc-pmp-04b"],
] as const;

interface LogicalGroup {
  ids: string[];
  x: number;
  y: number;
  columns: number;
}

const LOGICAL_GROUPS: LogicalGroup[] = [
  group(["area-01-wit-01", "area-01-ft-02", "area-01-xv-02", "area-01-feed-01", "area-01-feed-02", "area-01-feed-03", "area-01-feed-04"], 80, 630, 3),
  group(["area-01-dp-01", "area-01-lt-02", "area-01-tt-01", "area-01-pwt-01", "area-01-ag-01"], 800, 630, 2),
  group(["area-01-scr-01", "area-01-mag-01", "area-01-ag-02", "area-01-ht-01"], 80, 1110, 2),
  group(["area-01-dt-01", "area-01-vis-01", "area-01-vis-02", "area-01-thix-01", "area-01-psa-01", "area-01-res-01", "area-01-cr-01"], 550, 1110, 3),
  group(["area-01-tt-02", "area-01-lt-01", "area-01-ft-01", "area-01-pmp-01", "area-01-xv-01", "area-01-intlk-01", "area-01-slip-pit-01", "area-01-slip-pit-02", "area-01-slip-ft-02", "area-01-slip-ae-01", "area-01-slip-ld-01"], 80, 1590, 4),
  group(["area-01-wt-lt-01", "area-01-wt-tur-01", "area-01-wt-cnd-01", "area-01-wt-hard-01", "area-01-wt-pmp-01"], 1520, 630, 3),
  group(["area-01-wt-fil-01", "area-01-wt-dpit-01", "area-01-wt-carb-01", "area-01-wt-soft-01", "area-01-wt-ro-01"], 2210, 630, 3),
  group(["area-01-wt-ft-01", "area-01-wt-ft-02", "area-01-wt-cnd-02", "area-01-wt-hard-02", "area-01-wt-tur-02", "area-01-wt-lt-02", "area-01-ws-pit-01", "area-01-ws-ft-01", "area-01-ws-ld-01", "area-01-wg-pit-01", "area-01-wg-ft-01", "area-01-wg-ld-01", "area-01-wt-intlk-01"], 1520, 1080, 4),
  group(["area-01-rw-lt-01", "area-01-rw-lt-02", "area-01-rw-ag-01", "area-01-rw-dp-01", "area-01-rw-clar-01", "area-01-rw-tur-01", "area-01-rw-tur-02", "area-01-rw-ft-01"], 1520, 1720, 4),
  group(["area-01-rw-fp-01", "area-01-rw-wit-01", "area-01-rw-xv-01", "area-01-rw-lt-03", "area-01-rw-lt-04"], 1520, 2100, 3),
  group(["area-01-gl-wit-01", "area-01-gl-xv-01", "area-01-gl-dp-01", "area-01-gl-feed-01", "area-01-gl-feed-02", "area-01-gl-feed-03", "area-01-gl-feed-04", "area-01-gl-feed-05", "area-01-gl-feed-06", "area-01-gl-feed-07"], 80, 2710, 5),
  group(["area-01-gl-mill-01", "area-01-gl-scr-01", "area-01-gl-mag-01", "area-01-gl-dt-01", "area-01-gl-fc-01", "area-01-gl-psa-01", "area-01-gl-res-01", "area-01-gl-ag-01", "area-01-gl-lt-01", "area-01-gl-pmp-01", "area-01-gl-ft-01", "area-01-gl-pit-01", "area-01-gl-pit-02", "area-01-gl-ft-02", "area-01-gl-ld-01", "area-01-gl-intlk-01"], 80, 3120, 5),
];

const RIO_CHANNELS = new Map<string, string>(
  [
    ["area-01-rio-01", LOGICAL_GROUPS.slice(0, 2).flatMap((item) => item.ids)],
    ["area-01-rio-02", LOGICAL_GROUPS.slice(2, 5).flatMap((item) => item.ids)],
    ["area-01-rio-03", LOGICAL_GROUPS.slice(5, 8).flatMap((item) => item.ids)],
    ["area-01-rio-04", LOGICAL_GROUPS.slice(8, 10).flatMap((item) => item.ids)],
    ["area-01-rio-05", LOGICAL_GROUPS.slice(10).flatMap((item) => item.ids)],
  ].flatMap(([rio, ids]) => (ids as string[]).map((id) => [id, rio as string])),
);

export function bodyPreparationScopeForRoute(routeKey: string): BodyPreparationScope | null {
  const value = routeKey.startsWith("body-preparation/")
    ? routeKey.slice("body-preparation/".length)
    : "";
  return value === "slip" || value === "water" || value === "glaze" ? value : null;
}

export function bodyPreparationScopeMetadata(scope: BodyPreparationScope) {
  return SCOPE_METADATA[scope];
}

export function buildBodyPreparationEquipment(
  viewMode: ViewMode,
  scope: BodyPreparationScope | null = null,
): EquipmentView[] {
  return findAppliancesForEnvironment("Body Preparation")
    .filter((appliance) => !scope || applianceBelongsToScope(appliance, scope))
    .map((appliance) => {
      const position = positionFor(appliance.id, viewMode, scope);
      return position ? { ...toEquipment(appliance, viewMode), ...position } : null;
    })
    .filter((item): item is EquipmentView => item !== null);
}

function group(ids: string[], x: number, y: number, columns: number): LogicalGroup {
  return { ids, x, y, columns };
}

function applianceBelongsToScope(appliance: FrontendAppliance, scope: BodyPreparationScope) {
  if (SCOPE_CONTROL_IDS[scope].includes(appliance.id) || SCOPE_RIOS[scope].includes(appliance.id)) return true;
  if (scope === "water") {
    return waterRioFor(appliance.id) !== null
      || appliance.tags.includes("water-treatment")
      || appliance.tags.includes("return-water")
      || appliance.tags.includes("water-handoff");
  }
  return appliance.tags.includes(scope) || appliance.tags.includes(`${scope}-handoff`);
}

function positionFor(
  id: string,
  viewMode: ViewMode,
  scope: BodyPreparationScope | null,
) {
  if (scope) return scopedPositionFor(id, viewMode, scope);
  if (viewMode === "physical") return PHYSICAL_POSITIONS[id] ?? null;
  if (CONTROL_POSITIONS[id]) return CONTROL_POSITIONS[id];
  for (const item of LOGICAL_GROUPS) {
    const index = item.ids.indexOf(id);
    if (index >= 0) {
      return {
        x: item.x + (index % item.columns) * 225,
        y: item.y + Math.floor(index / item.columns) * 145,
      };
    }
  }
  return null;
}

function scopedPositionFor(id: string, viewMode: ViewMode, scope: BodyPreparationScope) {
  if (scope === "water") return waterPositionFor(id, viewMode);
  if (viewMode === "physical") return SCOPED_PHYSICAL_POSITIONS[scope][id] ?? null;
  const [hmi, vplc, cellSwitch] = SCOPE_CONTROL_IDS[scope];
  const controlPositions: Record<string, EquipmentPosition> = {
    [hmi]: { x: 70, y: 220 },
    [vplc]: { x: 340, y: 220 },
    [cellSwitch]: { x: 610, y: 220 },
  };
  if (controlPositions[id]) return controlPositions[id];
  const rioIndex = SCOPE_RIOS[scope].indexOf(id);
  if (rioIndex >= 0) {
    return {
      x: scope === "glaze" ? 850 : 390 + rioIndex * 980,
      y: 475,
    };
  }
  const owningRio = RIO_CHANNELS.get(id);
  if (!owningRio) return null;
  const ownerIndex = SCOPE_RIOS[scope].indexOf(owningRio);
  if (ownerIndex < 0) return null;
  const ownedIds = [...RIO_CHANNELS.entries()]
    .filter(([, rio]) => rio === owningRio)
    .map(([fieldId]) => fieldId);
  const index = ownedIds.indexOf(id);
  const columns = scope === "glaze" ? 8 : 4;
  const baseX = scope === "glaze" ? 70 : 65 + ownerIndex * 980;
  return {
    x: baseX + (index % columns) * 210,
    y: 735 + Math.floor(index / columns) * 145,
  };
}

function waterPositionFor(id: string, viewMode: ViewMode): EquipmentPosition | null {
  if (viewMode === "physical") {
    const hmiIndex = WATER_CONTROL_CELLS.findIndex((cell) => cell.hmi === id);
    if (hmiIndex >= 0) return { x: 2310, y: 245 + hmiIndex * 425 };
    for (let row = 0; row < WATER_PHYSICAL_ROWS.length; row += 1) {
      const index = (WATER_PHYSICAL_ROWS[row] as readonly string[]).indexOf(id);
      if (index >= 0) return { x: 65 + index * 260, y: 245 + row * 425 };
    }
    return null;
  }
  if (id === "area-01-wt-sw-01") return { x: 60, y: 250 };
  const cellIndex = WATER_CONTROL_CELLS.findIndex((cell) =>
    cell.hmi === id || cell.controller === id || cell.rio === id
  );
  if (cellIndex >= 0) {
    const cell = WATER_CONTROL_CELLS[cellIndex];
    const x = 310 + cellIndex * 570;
    if (id === cell.controller) return { x, y: 205 };
    if (id === cell.hmi) return { x, y: 365 };
    return { x: x + 235, y: 365 };
  }
  const rio = waterRioFor(id);
  const ownerIndex = WATER_CONTROL_CELLS.findIndex((cell) => cell.rio === rio);
  if (!rio || ownerIndex < 0) return null;
  const ids = findAppliancesForEnvironment("Body Preparation")
    .filter((appliance) => waterRioFor(appliance.id) === rio)
    .map((appliance) => appliance.id);
  const index = ids.indexOf(id);
  if (index < 0) return null;
  return { x: 65 + (index % 10) * 250, y: 760 + ownerIndex * 610 + Math.floor(index / 10) * 145 };
}

function waterRioFor(id: string): string | null {
  if (/area-01-(wd-|ws-|wg-|wf-)/.test(id) && !id.includes("-hmi-") && !id.includes("-vplc-")) return "area-01-rio-06";
  if (/area-01-(rc-|rb-|rbd-|rg-|rgd-)/.test(id) && !id.includes("-hmi-") && !id.includes("-vplc-")) return "area-01-rio-07";
  if (id.startsWith("area-01-rw-") && !id.includes("-hmi-") && !id.includes("-vplc-")) return "area-01-rio-04";
  if (id.startsWith("area-01-wt-") && !id.includes("-hmi-") && !id.includes("-vplc-") && !id.includes("-sw-")) return "area-01-rio-03";
  return null;
}

function toEquipment(appliance: FrontendAppliance, viewMode: ViewMode): ProcessEquipment {
  const isField = ["field-sensor", "field-actuator", "safety-interface"].includes(appliance.kind);
  let upstream: string | null = null;
  const scope = controlScopeFor(appliance.id);
  const controls = scope ? SCOPE_CONTROL_IDS[scope] : SCOPE_CONTROL_IDS.slip;
  const waterCell = WATER_CONTROL_CELLS.find((cell) =>
    cell.hmi === appliance.id || cell.controller === appliance.id || cell.rio === appliance.id
  );
  if (appliance.kind === "virtual-plc") upstream = scope === "water" ? "area-01-wt-sw-01" : controls[2];
  else if (appliance.kind === "hmi" || appliance.kind === "remote-io") upstream = waterCell?.controller ?? controls[1];
  else if (isField) upstream = waterRioFor(appliance.id) ?? RIO_CHANNELS.get(appliance.id) ?? null;
  const presentation = physicalPresentation(appliance);
  return {
    id: appliance.id,
    label: appliance.label,
    kind: displayKind(appliance),
    role: appliance.role,
    icon: iconFor(appliance),
    accent: accentFor(appliance),
    slot: slotFor(appliance),
    linkKind: appliance.kind === "safety-interface" ? "safety-status" : isField ? "io" : "ethernet",
    upstream,
    physicalUpstream: null,
    configRef: appliance.sourcePath,
    facts: [appliance.summary, ...appliance.behaviorFacts.slice(0, 3)],
    ...(viewMode === "physical" ? { physical: presentation } : {}),
  };
}

function controlScopeFor(id: string): BodyPreparationScope | null {
  for (const scope of Object.keys(SCOPE_CONTROL_IDS) as BodyPreparationScope[]) {
    if (SCOPE_CONTROL_IDS[scope].includes(id) || SCOPE_RIOS[scope].includes(id)) return scope;
  }
  const rio = waterRioFor(id) ?? RIO_CHANNELS.get(id);
  return rio ? controlScopeFor(rio) : null;
}

function physicalPresentation(appliance: FrontendAppliance) {
  return {
    label: physicalLabelFor(appliance.id, appliance.role).toUpperCase(),
    kind: displayKind(appliance),
    role: appliance.role,
    icon: iconFor(appliance),
    configRefs: [appliance.sourcePath],
    facts: [appliance.summary, ...appliance.behaviorFacts.slice(0, 3)],
  };
}

function physicalLabelFor(id: string, fallback: string) {
  const labels: Record<string, string> = {
    "area-01-wt-lt-01": "Raw-water equalization tank",
    "area-01-wt-pmp-01": "Raw-water transfer pump",
    "area-01-wt-fil-01": "Multimedia pressure filter",
    "area-01-wt-carb-01": "Activated-carbon contactor",
    "area-01-wt-soft-01": "Ion-exchange softener pair",
    "area-01-wt-ro-01": "Reverse-osmosis skid",
    "area-01-wt-lt-02": "Treated-water storage tank",
    "area-01-wit-01": "Slip weigh hopper",
    "area-01-dp-01": "Sodium-silicate dosing skid",
    "area-01-ag-01": "Wet-mixing blunger",
    "area-01-scr-01": "Slip vibrating screen",
    "area-01-mag-01": "Slip magnetic separator",
    "area-01-ag-02": "Conditioning and release tank",
    "area-01-pmp-01": "Released-slip transfer pump",
    "area-01-gl-wit-01": "Glaze powder weigh hopper",
    "area-01-gl-xv-01": "Glaze charge-water valve",
    "area-01-gl-dp-01": "Glaze dispersant dosing skid",
    "area-01-gl-mill-01": "Glaze wet ball mill",
    "area-01-gl-scr-01": "63-um glaze vibrating screen",
    "area-01-gl-mag-01": "Glaze magnetic separator",
    "area-01-gl-ag-01": "Agitated glaze-storage tank",
    "area-01-gl-pmp-01": "Glaze transfer pump",
    "area-01-rw-lt-01": "Body-return equalization tank",
    "area-01-rw-lt-02": "Glaze-return equalization tank",
    "area-01-rw-clar-01": "Lamella clarifier",
    "area-01-rw-fp-01": "Sludge filter press",
    "area-01-rw-lt-03": "Body-water reuse tank",
    "area-01-rw-lt-04": "Glaze-water reuse tank",
    "area-01-rw-xv-01": "Quality-based reuse diverter",
  };
  return labels[id] ?? fallback;
}

function displayKind(appliance: FrontendAppliance) {
  const names: Record<string, string> = {
    "layer-2-switch": "industrial switch",
    "virtual-plc": "area controller",
    hmi: "area HMI",
    "remote-io": "distributed I/O",
    "field-sensor": "process input",
    "field-actuator": "process output",
    "safety-interface": "safety interface",
  };
  return names[appliance.kind] ?? appliance.kind;
}

function slotFor(appliance: FrontendAppliance): ProcessEquipment["slot"] {
  if (appliance.kind === "layer-2-switch") return "switch";
  if (appliance.kind === "virtual-plc") return "controller";
  if (appliance.kind === "hmi") return "hmi";
  if (appliance.kind === "remote-io") return "remote-io";
  if (appliance.kind === "field-sensor") return "sensor-a";
  if (appliance.kind === "field-actuator") return "actuator-a";
  return "safety";
}

function iconFor(appliance: FrontendAppliance): ProcessIconKey {
  if (appliance.kind === "layer-2-switch") return "network";
  if (appliance.kind === "virtual-plc") return "cpu";
  if (appliance.kind === "hmi") return "monitor";
  if (appliance.kind === "remote-io") return "remote-io";
  if (appliance.kind === "safety-interface") return "shield";
  if (appliance.tags.includes("temperature")) return "thermometer";
  if (appliance.tags.includes("water-treatment") || appliance.tags.includes("return-water")) return "droplets";
  if (appliance.tags.includes("feeder") || appliance.tags.includes("mass")) return "boxes";
  if (appliance.kind === "field-actuator") return "valve";
  return "gauge";
}

function accentFor(appliance: FrontendAppliance) {
  if (appliance.tags.includes("water-treatment")) return "#39798a";
  if (appliance.tags.includes("return-water")) return "#5d7168";
  if (appliance.tags.includes("glaze")) return "#6f678d";
  if (appliance.kind === "field-sensor") return "#51704c";
  if (appliance.kind === "field-actuator") return "#a85c38";
  if (appliance.kind === "safety-interface") return "#9e3f2f";
  return "#3567a6";
}

import type { ViewMode } from "../../shared/types";
import type { ProcessEquipment } from "../process-model";
import type { EquipmentPresentation, EquipmentView } from "./process-area-layout";

const NODE_WIDTH = 190;
const NODE_HEIGHT = 105;

export function equipmentPresentation(
  item: ProcessEquipment,
  viewMode: ViewMode,
): EquipmentPresentation {
  if (viewMode === "physical" && item.physical) return item.physical;
  return {
    label: item.label,
    kind: item.kind,
    role: item.role,
    icon: item.icon,
    facts: item.facts,
  };
}

export function connectionPath(
  item: EquipmentView,
  equipment: EquipmentView[],
  viewMode: ViewMode,
  routeKey: string,
) {
  const upstreamId = viewMode === "physical" && item.physicalUpstream !== undefined
    ? item.physicalUpstream
    : item.upstream;
  const upstream = equipment.find((candidate) => candidate.id === upstreamId);
  if (!upstream) return "";
  const source = equipmentCenter(upstream);
  const target = equipmentCenter(item);
  if (routeKey === "forming") {
    if (item.linkKind === "io" || item.linkKind === "safety-status") {
      const fieldBus = target.y > 1600 ? 1665 : source.y < 500 ? 495 : 690;
      return `M${source.x} ${source.y} V${fieldBus} H${target.x} V${target.y}`;
    }
    if (isOperatorEquipment(item) || item.kind.includes("remote I/O")) {
      const operatorBus = target.y < 500 ? 185 : 485;
      return `M${source.x} ${source.y} V${operatorBus} H${target.x} V${target.y}`;
    }
  }
  if (routeKey.startsWith("body-preparation")) {
    if (item.linkKind === "io" || item.linkKind === "safety-status") {
      if (routeKey === "body-preparation/water") {
        const fieldBus = target.y > 2400
          ? 2450
          : target.y > 1800
            ? 1840
            : target.y > 1200
              ? 1230
              : 620;
        return `M${source.x} ${source.y} V${fieldBus} H${target.x} V${target.y}`;
      }
      const fieldBus = target.y > 2500
        ? 2590
        : target.y > 1550
          ? 1620
          : 590;
      return `M${source.x} ${source.y} V${fieldBus} H${target.x} V${target.y}`;
    }
    const controlBus = 195;
    return `M${source.x} ${source.y} V${controlBus} H${target.x} V${target.y}`;
  }
  if (item.linkKind === "safety-status") {
    return `M${source.x} ${source.y} V810 H${target.x} V${target.y}`;
  }
  const midpoint = source.x + (target.x - source.x) / 2;
  return `M${source.x} ${source.y} H${midpoint} V${target.y} H${target.x}`;
}

export function isOperatorEquipment(item: EquipmentView) {
  return ["mould HMI", "robot joystick", "embedded SCADA PC"].includes(item.kind);
}

function equipmentCenter(item: EquipmentView) {
  return { x: item.x + NODE_WIDTH / 2, y: item.y + NODE_HEIGHT / 2 };
}

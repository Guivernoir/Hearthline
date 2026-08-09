import type { ProcessEquipment } from "../process-model";

export interface EquipmentPosition {
  x: number;
  y: number;
}

export interface EquipmentView extends ProcessEquipment, EquipmentPosition {}

export interface EquipmentPresentation {
  label: string;
  kind: string;
  role: string;
  icon: ProcessEquipment["icon"];
  facts: string[];
}

export const physicalSlotPositions: Record<
  ProcessEquipment["slot"],
  EquipmentPosition
> = {
  controller: { x: 70, y: 155 },
  hmi: { x: 315, y: 155 },
  switch: { x: 70, y: 400 },
  "remote-io": { x: 315, y: 400 },
  "sensor-a": { x: 620, y: 210 },
  "sensor-b": { x: 620, y: 535 },
  "actuator-a": { x: 965, y: 210 },
  "actuator-b": { x: 965, y: 535 },
  safety: { x: 965, y: 710 },
};

export const logicalSlotPositions: Record<
  ProcessEquipment["slot"],
  EquipmentPosition
> = {
  switch: { x: 70, y: 390 },
  controller: { x: 315, y: 390 },
  hmi: { x: 315, y: 145 },
  "remote-io": { x: 620, y: 390 },
  "sensor-a": { x: 620, y: 185 },
  "sensor-b": { x: 620, y: 585 },
  "actuator-a": { x: 965, y: 185 },
  "actuator-b": { x: 965, y: 585 },
  safety: { x: 965, y: 710 },
};

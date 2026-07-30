import type { Component } from "svelte";

export type OfficeEnvironment =
  | "it-dmz"
  | "business-it"
  | "operations-intelligence"
  | "ot-dmz";

interface NodePosition {
  x: number;
  y: number;
}

export interface OfficeNode {
  id: string;
  label: string;
  role: string;
  area: string;
  address: string;
  facts: string[];
  accent: string;
  icon: Component<any>;
  physical: NodePosition;
  logical: NodePosition;
}

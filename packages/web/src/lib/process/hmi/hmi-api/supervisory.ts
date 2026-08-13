export interface HmiSupervisoryState {
  namespace: string;
  modelId: string;
  repository: HmiSupervisoryRepository;
  templates: HmiSupervisoryTemplate[];
  assets: HmiSupervisoryAsset[];
  deploymentNodes: HmiSupervisoryNode[];
  identity: HmiSupervisoryIdentity;
  tags: HmiSupervisoryTag[];
  events: HmiSupervisoryEvent[];
}

export interface HmiSupervisoryRepository {
  id: string;
  revision: string;
  deployedRevision: string;
  synchronized: boolean;
}

export interface HmiSupervisoryTemplate {
  id: string;
  label: string;
  parent: string | null;
  attributes: string[];
  alarmCapable: boolean;
  historyCapable: boolean;
}

export interface HmiSupervisoryAsset {
  id: string;
  label: string;
  template: string;
  parent: string | null;
  components: string[];
  historizedTags: string[];
}

export interface HmiSupervisoryNode {
  id: string;
  label: string;
  host: string;
  role: string;
  state: string;
  redundancyGroup: string | null;
}

export interface HmiSupervisoryIdentity {
  user: string;
  role: string;
  authentication: string;
  permissions: string[];
}

export interface HmiSupervisorySample {
  timestampMs: number;
  value: number;
  qualityGood: boolean;
}

export interface HmiSupervisoryTag {
  tag: string;
  value: number;
  unit: string;
  quality: "good" | "bad";
  timestampMs: number;
  samples: HmiSupervisorySample[];
}

export interface HmiSupervisoryEvent {
  sequence: number;
  category: "alarm" | "operator-audit";
  source: string;
  message: string;
  state: string;
}

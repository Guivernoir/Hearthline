export type ScenarioTraceKind =
  | "transmission"
  | "delivery"
  | "application"
  | "media"
  | "drop"
  | "observation"
  | "process";

export interface UdpTransport {
  protocol: "udp";
  source_port: number;
  destination_port: number;
}

export interface TcpTransport {
  protocol: "tcp";
  source_port: number;
  destination_port: number;
  syn: boolean;
  ack: boolean;
  fin: boolean;
  rst: boolean;
}

export interface IcmpEchoTransport {
  protocol: "icmp-echo";
  identifier: number;
  sequence: number;
}

export type ScenarioTransport =
  | UdpTransport
  | TcpTransport
  | IcmpEchoTransport;

export type ScenarioApplication =
  | { kind: "none" }
  | { kind: "dns-query"; name: string }
  | {
      kind: "http-request";
      method: "get" | "head" | "post" | "put" | "patch" | "delete" | "options";
      host: string;
      path: string;
      body: string | null;
      body_bytes: number;
    }
  | { kind: "service"; service: string };

export interface ScenarioPacket {
  source_ip: string;
  destination_ip: string;
  ttl: number;
  wire_length_bytes: number;
  transport: ScenarioTransport;
  application: ScenarioApplication;
}

export interface ScenarioExpectation {
  component: string;
  outcome: "delivered" | "forwarded" | "dropped";
  service: string | null;
  target: string | null;
  reason_contains: string | null;
}

export interface ScenarioConnectionOverride {
  connection: string;
  operational: boolean;
}

export interface ScenarioConnectionState {
  id: string;
  label: string;
  endpoint_a: string;
  endpoint_b: string;
  configured_operational: boolean;
  operational: boolean;
}

export type FirstHopRole = "active" | "standby";

export interface ScenarioFirstHopOverride {
  appliance: string;
  interface: string;
  role: FirstHopRole;
}

export interface ScenarioFirstHopState extends ScenarioFirstHopOverride {
  protocol: "vrrp";
  group: number;
  virtual_ip: string;
  virtual_mac: string;
  priority: number;
  preempt: boolean;
  configured_role: FirstHopRole;
}

export type FirewallHaRole = "active" | "standby";

export interface ScenarioFirewallHaOverride {
  appliance: string;
  role: FirewallHaRole;
}

export interface ScenarioFirewallHaState extends ScenarioFirewallHaOverride {
  peer: string;
  domain: string;
  configured_role: FirewallHaRole;
  sync_interface: string;
  sync_connection: string;
  sync_operational: boolean;
  session_sync: boolean;
  heartbeat_interval_ms: number;
  failure_hold_ms: number;
  monitored_interfaces: string[];
}

export type SpanningTreePortRole =
  | "root"
  | "designated"
  | "alternate"
  | "disabled";

export type SpanningTreePortState = "forwarding" | "discarding";

export interface ScenarioSpanningTreeState {
  appliance: string;
  interface: string;
  connection: string;
  protocol: "rapid-pvst";
  vlan: number;
  root_bridge: string;
  root_path_cost: number;
  port_path_cost: number;
  role: SpanningTreePortRole;
  state: SpanningTreePortState;
}

export interface ScenarioLinkAggregationState {
  appliance: string;
  interface: string;
  connection: string;
  group: string;
  logical_id: string;
  protocol: "lacp";
  mode: "active" | "passive";
  system_id: string;
  partner_system_id: string;
  multi_chassis_domain: string | null;
  selected: boolean;
  collecting: boolean;
  distributing: boolean;
  bundle_operational: boolean;
  active_members: number;
  minimum_active_members: number;
  peer_forwarding: boolean;
}

export interface ScenarioRecovery {
  label: string;
  summary: string;
  connection_overrides: ScenarioConnectionOverride[];
  first_hop_overrides: ScenarioFirstHopOverride[];
  firewall_ha_overrides: ScenarioFirewallHaOverride[];
  expectation: ScenarioExpectation;
}

export type ScenarioContinuityFault =
  | { type: "sync-link-loss"; at_us: number }
  | { type: "standby-session-loss"; at_us: number };

export interface ScenarioContinuity {
  failed_appliance: string;
  failure_at_us: number;
  continuation_at_us: number;
  source: string;
  packet: ScenarioPacket;
  faults: ScenarioContinuityFault[];
  connection_overrides: ScenarioConnectionOverride[];
  expectation: ScenarioExpectation;
}

export interface ScenarioHaIsolation {
  standby_appliance: string;
  isolation_at_us: number;
  continuation_at_us: number;
  source: string;
  packet: ScenarioPacket;
  connection_overrides: ScenarioConnectionOverride[];
  expectation: ScenarioExpectation;
}

export interface ScenarioLocalAutonomy {
  hmi: string;
  safety_interface: string;
  actuator: string;
  command_tag: string;
  command_value: string;
  expected_actuator_state: string;
}

export type TraceFilter = "all" | "network" | "media" | "drops";

export type SecuritySeverity =
  | "informational"
  | "low"
  | "medium"
  | "high"
  | "critical";

export interface ScenarioSecurityConfig {
  tactic: string;
  technique: string;
  severity: SecuritySeverity;
  detector: string;
  defender: string;
  control: string;
}

export interface ScenarioSummary {
  schema_version: string;
  id: string;
  label: string;
  summary: string;
  category: string;
  participants: string[];
  source: string;
  packet: ScenarioPacket;
  connection_states: ScenarioConnectionState[];
  first_hop_states: ScenarioFirstHopState[];
  firewall_ha_states: ScenarioFirewallHaState[];
  link_aggregation_states: ScenarioLinkAggregationState[];
  spanning_tree_states: ScenarioSpanningTreeState[];
  recovery: ScenarioRecovery | null;
  continuity: ScenarioContinuity | null;
  ha_isolation: ScenarioHaIsolation | null;
  local_autonomy: ScenarioLocalAutonomy | null;
  expectation: ScenarioExpectation;
  security: ScenarioSecurityConfig | null;
}

export interface SimulationCatalog {
  schema_version: string;
  scenarios: ScenarioSummary[];
}

export interface ScenarioStatistics {
  events: number;
  transmissions: number;
  media_transits: number;
  deliveries: number;
  drops: number;
  observations: number;
}

export interface ScenarioHttpDocument {
  title: string;
  heading: string;
  body: string;
}

export interface ScenarioHttpResponse {
  status: number;
  document: ScenarioHttpDocument | null;
}

export interface ScenarioTraceEntry {
  sequence: number;
  time_us: number;
  component: string;
  kind: ScenarioTraceKind;
  summary: string;
  egress: string | null;
  connection: string | null;
  peer: string | null;
  source_ip: string | null;
  destination_ip: string | null;
  protocol: string | null;
}

export interface ScenarioSecurityEvent extends ScenarioSecurityConfig {
  schema_version: string;
  scenario_id: string;
  disposition: "prevented" | "control-failed";
  source_ip: string;
  destination_ip: string;
  observed_at_us: number;
  evidence: string;
}

export interface ScenarioContinuityReport {
  failed_appliance: string;
  promoted_appliance: string;
  failure_at_us: number;
  last_heartbeat_us: number;
  promotion_at_us: number;
  interruption_us: number;
  synchronized_sessions: number;
  sessions_after_continuation: number;
  replicated_updates: number;
  sync_operational_at_failure: boolean;
  faults: ScenarioContinuityFault[];
  continuation_expectation_met: boolean;
}

export interface ScenarioHaIsolationReport {
  active_appliance: string;
  standby_appliance: string;
  isolation_at_us: number;
  last_heartbeat_us: number;
  evaluation_at_us: number;
  promotion_inhibited_at_us: number;
  active_members: number;
  standby_sessions: number;
  sync_operational: boolean;
  peer_failure_confirmed: boolean;
  continuation_expectation_met: boolean;
}

export interface ScenarioControlTraceEntry {
  sequence: number;
  component: string;
  stage: string;
  detail: string;
}

export interface ScenarioLocalAutonomyReport {
  hmi: string;
  controller: string;
  remote_io: string;
  safety_interface: string;
  actuator: string;
  command_tag: string;
  command_value: string;
  expected_actuator_state: string;
  actuator_state: string;
  outage_connections: string[];
  local_path_connections: string[];
  local_path_operational: boolean;
  safety_reset_applied: boolean;
  command_applied: boolean;
  northbound_expectation_met: boolean;
  autonomy_expectation_met: boolean;
  control_trace: ScenarioControlTraceEntry[];
}

export interface ScenarioReport {
  schema_version: string;
  scenario_id: string;
  scenario_label: string;
  status: "passed" | "failed";
  expectation_mode:
    | "baseline"
    | "recovery"
    | "continuity"
    | "isolation"
    | "autonomy";
  expectation_met: boolean;
  duration_us: number;
  appliance_count: number;
  link_count: number;
  packet: ScenarioPacket;
  connection_states: ScenarioConnectionState[];
  first_hop_states: ScenarioFirstHopState[];
  firewall_ha_states: ScenarioFirewallHaState[];
  link_aggregation_states: ScenarioLinkAggregationState[];
  spanning_tree_states: ScenarioSpanningTreeState[];
  expectation: ScenarioExpectation;
  statistics: ScenarioStatistics;
  http_response: ScenarioHttpResponse | null;
  security: ScenarioSecurityEvent | null;
  continuity: ScenarioContinuityReport | null;
  ha_isolation: ScenarioHaIsolationReport | null;
  local_autonomy: ScenarioLocalAutonomyReport | null;
  trace: ScenarioTraceEntry[];
}

interface ErrorResponse {
  error?: string;
}

export async function loadSimulationCatalog(): Promise<SimulationCatalog> {
  return requestJson("/api/simulations");
}

export async function runSimulation(
  id: string,
  packet: ScenarioPacket | null,
  connectionOverrides: ScenarioConnectionOverride[] | null,
  firstHopOverrides: ScenarioFirstHopOverride[] | null,
  firewallHaOverrides: ScenarioFirewallHaOverride[] | null,
): Promise<ScenarioReport> {
  return requestJson(`/api/simulations/${encodeURIComponent(id)}/run`, {
    method: "POST",
    headers: {
      Accept: "application/json",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      packet,
      connection_overrides: connectionOverrides,
      first_hop_overrides: firstHopOverrides,
      firewall_ha_overrides: firewallHaOverrides,
    }),
  });
}

async function requestJson<T>(
  endpoint: string,
  init?: RequestInit,
): Promise<T> {
  const response = await fetch(endpoint, {
    headers: { Accept: "application/json" },
    ...init,
  });
  if (!response.ok) {
    let message = `Simulation request failed (${response.status})`;
    try {
      const body = (await response.json()) as ErrorResponse;
      if (body.error) message = body.error;
    } catch {
      // Preserve the status-based message when the API did not return JSON.
    }
    throw new Error(message);
  }
  return (await response.json()) as T;
}

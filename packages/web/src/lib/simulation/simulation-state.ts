import type {
  FirewallHaRole,
  ScenarioConnectionState,
  ScenarioFirewallHaState,
  ScenarioFirstHopState,
  ScenarioRecovery,
} from "./simulation-api";

export interface ScenarioRecoveryState {
  connectionStates: ScenarioConnectionState[];
  firstHopStates: ScenarioFirstHopState[];
  firewallHaStates: ScenarioFirewallHaState[];
}

export function isScenarioRecoveryApplied(
  recovery: ScenarioRecovery | null,
  current: ScenarioRecoveryState,
): boolean {
  if (!recovery) return false;
  return (
    recovery.connection_overrides.every((overrideState) =>
      current.connectionStates.some(
        (state) =>
          state.id === overrideState.connection &&
          state.operational === overrideState.operational,
      ),
    ) &&
    recovery.first_hop_overrides.every((overrideState) =>
      current.firstHopStates.some(
        (state) =>
          state.appliance === overrideState.appliance &&
          state.interface === overrideState.interface &&
          state.role === overrideState.role,
      ),
    ) &&
    recovery.firewall_ha_overrides.every((overrideState) =>
      current.firewallHaStates.some(
        (state) =>
          state.appliance === overrideState.appliance &&
          state.role === overrideState.role,
      ),
    )
  );
}

export function transitionFirewallHaRole(
  states: ScenarioFirewallHaState[],
  firstHopStates: ScenarioFirstHopState[],
  appliance: string,
  role: FirewallHaRole,
): Pick<ScenarioRecoveryState, "firewallHaStates" | "firstHopStates"> | null {
  const selected = states.find((state) => state.appliance === appliance);
  if (!selected) return null;

  const activeAppliance = role === "active" ? appliance : selected.peer;
  const firewallHaStates = states.map((state) =>
    state.domain === selected.domain
      ? {
          ...state,
          role: state.appliance === activeAppliance
            ? ("active" as const)
            : ("standby" as const),
        }
      : state,
  );
  const roles = new Map(
    firewallHaStates.map((state) => [state.appliance, state.role]),
  );
  const monitored = new Map(
    firewallHaStates.map((state) => [
      state.appliance,
      new Set(state.monitored_interfaces),
    ]),
  );

  return {
    firewallHaStates,
    firstHopStates: firstHopStates.map((state) => {
      const memberRole = roles.get(state.appliance);
      return memberRole && monitored.get(state.appliance)?.has(state.interface)
        ? { ...state, role: memberRole }
        : state;
    }),
  };
}

export function applyScenarioRecovery(
  recovery: ScenarioRecovery,
  current: ScenarioRecoveryState,
): ScenarioRecoveryState {
  const connectionOverrides = new Map(
    recovery.connection_overrides.map((state) => [
      state.connection,
      state.operational,
    ]),
  );
  const connectionStates = current.connectionStates.map((state) => ({
    ...state,
    operational: connectionOverrides.get(state.id) ?? state.operational,
  }));
  const firstHopOverrides = new Map(
    recovery.first_hop_overrides.map((state) => [
      `${state.appliance}:${state.interface}`,
      state.role,
    ]),
  );
  const firstHopStates = current.firstHopStates.map((state) => ({
    ...state,
    role:
      firstHopOverrides.get(`${state.appliance}:${state.interface}`) ??
      state.role,
  }));
  const firewallOverrides = new Map(
    recovery.firewall_ha_overrides.map((state) => [
      state.appliance,
      state.role,
    ]),
  );
  const firewallHaStates = current.firewallHaStates.map((state) => ({
    ...state,
    role: firewallOverrides.get(state.appliance) ?? state.role,
    sync_operational:
      connectionStates.find(
        (connection) => connection.id === state.sync_connection,
      )?.operational ?? false,
  }));

  return { connectionStates, firstHopStates, firewallHaStates };
}

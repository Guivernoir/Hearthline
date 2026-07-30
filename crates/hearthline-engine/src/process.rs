use std::collections::{BTreeMap, BTreeSet};

use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, NetworkPayload, PortId, ProcessEvent,
    ProcessSignal, ServiceKind, SignalValue,
};

use crate::{DropReason, Effect, ProcessEffect, SimulatedComponent, SimulationEvent};

fn is_industrial_communication(frame: &hearthline_model::EthernetFrame) -> bool {
    let NetworkPayload::Ipv4(packet) = &frame.payload else {
        return false;
    };
    matches!(
        packet.application,
        ApplicationData::Service(ServiceKind::IndustrialIo | ServiceKind::PlcEngineering)
    ) || matches!(packet.transport.destination_port(), Some(502 | 4840))
}

#[derive(Clone, Debug, PartialEq)]
pub enum Comparison {
    BoolEquals(bool),
    AnalogGreaterThan(f64),
    AnalogLessThan(f64),
    IntegerGreaterThan(i64),
    IntegerLessThan(i64),
}

impl Comparison {
    fn matches(&self, value: &SignalValue) -> bool {
        match (self, value) {
            (Self::BoolEquals(expected), SignalValue::Bool(actual)) => expected == actual,
            (Self::AnalogGreaterThan(limit), SignalValue::Analog(actual)) => actual > limit,
            (Self::AnalogLessThan(limit), SignalValue::Analog(actual)) => actual < limit,
            (Self::IntegerGreaterThan(limit), SignalValue::Integer(actual)) => actual > limit,
            (Self::IntegerLessThan(limit), SignalValue::Integer(actual)) => actual < limit,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogicRule {
    pub input: String,
    pub comparison: Comparison,
    pub output: String,
    pub value_when_true: SignalValue,
    pub value_when_false: SignalValue,
}

#[derive(Clone, Debug)]
pub struct VirtualPlc {
    id: ComponentId,
    ports: BTreeSet<PortId>,
    scan_period_ms: u64,
    elapsed_ms: u64,
    inputs: BTreeMap<String, ProcessSignal>,
    outputs: BTreeMap<String, SignalValue>,
    rules: Vec<LogicRule>,
    operational: bool,
}

impl VirtualPlc {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        scan_period_ms: u64,
        rules: Vec<LogicRule>,
    ) -> Self {
        assert!(scan_period_ms > 0, "scan period must be positive");
        Self {
            id,
            ports: ports.into_iter().collect(),
            scan_period_ms,
            elapsed_ms: 0,
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            rules,
            operational: true,
        }
    }

    pub fn outputs(&self) -> &BTreeMap<String, SignalValue> {
        &self.outputs
    }

    fn scan(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        for rule in &self.rules {
            let Some(input) = self.inputs.get(&rule.input) else {
                effects.push(Effect::Process(ProcessEffect::Alarm {
                    code: "PLC-MISSING-INPUT".into(),
                    active: true,
                    message: format!("{} has no value for {}", self.id, rule.input),
                }));
                continue;
            };
            let value = if input.quality_good && rule.comparison.matches(&input.value) {
                rule.value_when_true.clone()
            } else {
                rule.value_when_false.clone()
            };
            let changed = self.outputs.get(&rule.output) != Some(&value);
            self.outputs.insert(rule.output.clone(), value.clone());
            if changed {
                effects.push(Effect::Process(ProcessEffect::Output {
                    tag: rule.output.clone(),
                    value,
                }));
            }
        }
        effects
    }
}

impl SimulatedComponent for VirtualPlc {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::VirtualPlc
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        if !self.operational && !matches!(event, SimulationEvent::SetOperational(_)) {
            return vec![Effect::Drop(DropReason::ComponentDown)];
        }
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.ports.contains(&ingress.port) {
                    return vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))];
                }
                let NetworkPayload::Ipv4(packet) = ingress.frame.payload else {
                    return vec![Effect::Drop(DropReason::UnsupportedProtocol)];
                };
                let industrial =
                    matches!(
                        packet.application,
                        ApplicationData::Service(
                            ServiceKind::IndustrialIo | ServiceKind::PlcEngineering
                        )
                    ) || matches!(packet.transport.destination_port(), Some(502 | 4840));
                if industrial {
                    vec![Effect::Deliver {
                        service: ServiceKind::IndustrialIo,
                        detail: format!("{} accepted modeled controller communication", self.id),
                    }]
                } else {
                    vec![Effect::Drop(DropReason::PolicyDenied { rule: None })]
                }
            }
            SimulationEvent::Process(ProcessEvent::Signals(signals)) => {
                self.inputs.extend(signals);
                Vec::new()
            }
            SimulationEvent::Process(ProcessEvent::Tick { elapsed_ms }) => {
                self.elapsed_ms = self.elapsed_ms.saturating_add(elapsed_ms);
                let mut effects = Vec::new();
                while self.elapsed_ms >= self.scan_period_ms {
                    self.elapsed_ms -= self.scan_period_ms;
                    effects.extend(self.scan());
                }
                effects
            }
            SimulationEvent::Process(ProcessEvent::Command(command)) => {
                vec![Effect::Process(ProcessEffect::Command(command))]
            }
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.outputs.clear();
                vec![Effect::Process(ProcessEffect::Alarm {
                    code: "PLC-EXTERNAL-TRIP".into(),
                    active: true,
                    message: cause,
                })]
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                vec![Effect::Process(ProcessEffect::State {
                    name: "reset-request".into(),
                    value: if authorized {
                        "accepted".into()
                    } else {
                        "denied".into()
                    },
                })]
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.outputs.clear();
                }
                vec![Effect::Observe {
                    detail: format!("operational={operational}"),
                }]
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct OperatorInterface {
    id: ComponentId,
    ports: BTreeSet<PortId>,
    allowed_command_tags: BTreeSet<String>,
    operational: bool,
}

impl OperatorInterface {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        allowed_command_tags: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            id,
            ports: ports.into_iter().collect(),
            allowed_command_tags: allowed_command_tags.into_iter().collect(),
            operational: true,
        }
    }
}

impl SimulatedComponent for OperatorInterface {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::Hmi
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        if !self.operational && !matches!(event, SimulationEvent::SetOperational(_)) {
            return vec![Effect::Drop(DropReason::ComponentDown)];
        }
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.ports.contains(&ingress.port) {
                    vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))]
                } else if is_industrial_communication(&ingress.frame) {
                    vec![Effect::Deliver {
                        service: ServiceKind::IndustrialIo,
                        detail: "HMI received controller state".into(),
                    }]
                } else {
                    vec![Effect::Drop(DropReason::PolicyDenied { rule: None })]
                }
            }
            SimulationEvent::Process(ProcessEvent::Command(command)) => {
                if self.allowed_command_tags.contains(&command.tag) {
                    vec![Effect::Process(ProcessEffect::Command(command))]
                } else {
                    vec![Effect::Process(ProcessEffect::Alarm {
                        code: "HMI-COMMAND-DENIED".into(),
                        active: true,
                        message: format!("command tag is not allowed: {}", command.tag),
                    })]
                }
            }
            SimulationEvent::Process(ProcessEvent::Signals(signals)) => signals
                .into_values()
                .map(ProcessEffect::Signal)
                .map(Effect::Process)
                .collect(),
            SimulationEvent::Process(ProcessEvent::Tick { .. }) => Vec::new(),
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                vec![Effect::Process(ProcessEffect::Alarm {
                    code: "HMI-TRIP-DISPLAY".into(),
                    active: true,
                    message: cause,
                })]
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                vec![Effect::Process(ProcessEffect::State {
                    name: "reset-request".into(),
                    value: format!("authorized={authorized}"),
                })]
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                vec![Effect::Observe {
                    detail: format!("operational={operational}"),
                }]
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoDirection {
    Input,
    Output,
}

#[derive(Clone, Debug)]
pub struct RemoteIo {
    id: ComponentId,
    ports: BTreeSet<PortId>,
    channels: BTreeMap<String, IoDirection>,
    values: BTreeMap<String, SignalValue>,
    operational: bool,
}

impl RemoteIo {
    pub fn new(
        id: ComponentId,
        ports: impl IntoIterator<Item = PortId>,
        channels: impl IntoIterator<Item = (String, IoDirection)>,
    ) -> Self {
        Self {
            id,
            ports: ports.into_iter().collect(),
            channels: channels.into_iter().collect(),
            values: BTreeMap::new(),
            operational: true,
        }
    }

    pub fn values(&self) -> &BTreeMap<String, SignalValue> {
        &self.values
    }
}

impl SimulatedComponent for RemoteIo {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::RemoteIo
    }

    fn has_port(&self, port: &PortId) -> bool {
        self.ports.contains(port)
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        if !self.operational && !matches!(event, SimulationEvent::SetOperational(_)) {
            return vec![Effect::Drop(DropReason::ComponentDown)];
        }
        match event {
            SimulationEvent::Network(ingress) => {
                if !self.ports.contains(&ingress.port) {
                    vec![Effect::Drop(DropReason::InvalidIngress(ingress.port))]
                } else if is_industrial_communication(&ingress.frame) {
                    vec![Effect::Deliver {
                        service: ServiceKind::IndustrialIo,
                        detail: "remote I/O exchanged channel data".into(),
                    }]
                } else {
                    vec![Effect::Drop(DropReason::PolicyDenied { rule: None })]
                }
            }
            SimulationEvent::Process(ProcessEvent::Signals(signals)) => {
                let mut effects = Vec::new();
                for (tag, signal) in signals {
                    if self.channels.get(&tag) == Some(&IoDirection::Input) {
                        self.values.insert(tag, signal.value.clone());
                        effects.push(Effect::Process(ProcessEffect::Signal(signal)));
                    } else {
                        effects.push(Effect::Process(ProcessEffect::Alarm {
                            code: "RIO-INVALID-INPUT".into(),
                            active: true,
                            message: format!("input signal is not mapped: {tag}"),
                        }));
                    }
                }
                effects
            }
            SimulationEvent::Process(ProcessEvent::Command(command)) => {
                if self.channels.get(&command.tag) == Some(&IoDirection::Output) {
                    self.values
                        .insert(command.tag.clone(), command.value.clone());
                    vec![Effect::Process(ProcessEffect::Output {
                        tag: command.tag,
                        value: command.value,
                    })]
                } else {
                    vec![Effect::Process(ProcessEffect::Alarm {
                        code: "RIO-INVALID-OUTPUT".into(),
                        active: true,
                        message: format!("output channel is not mapped: {}", command.tag),
                    })]
                }
            }
            SimulationEvent::Process(ProcessEvent::Tick { .. }) => Vec::new(),
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.values.clear();
                vec![Effect::Process(ProcessEffect::Alarm {
                    code: "RIO-TRIP".into(),
                    active: true,
                    message: cause,
                })]
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                vec![Effect::Process(ProcessEffect::State {
                    name: "reset-request".into(),
                    value: format!("authorized={authorized}"),
                })]
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.values.clear();
                }
                vec![Effect::Observe {
                    detail: format!("operational={operational}"),
                }]
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct FieldSensor {
    id: ComponentId,
    tag: String,
    raw_value: f64,
    gain: f64,
    offset: f64,
    quality_good: bool,
    sample_period_ms: u64,
    elapsed_ms: u64,
    timestamp_ms: u64,
    operational: bool,
}

impl FieldSensor {
    pub fn new(
        id: ComponentId,
        tag: impl Into<String>,
        sample_period_ms: u64,
        gain: f64,
        offset: f64,
    ) -> Self {
        assert!(sample_period_ms > 0, "sample period must be positive");
        Self {
            id,
            tag: tag.into(),
            raw_value: 0.0,
            gain,
            offset,
            quality_good: true,
            sample_period_ms,
            elapsed_ms: 0,
            timestamp_ms: 0,
            operational: true,
        }
    }

    pub fn set_raw_value(&mut self, value: f64) {
        self.raw_value = value;
    }
}

impl SimulatedComponent for FieldSensor {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::FieldSensor
    }

    fn has_port(&self, _port: &PortId) -> bool {
        false
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Process(ProcessEvent::Tick { elapsed_ms }) => {
                self.elapsed_ms = self.elapsed_ms.saturating_add(elapsed_ms);
                self.timestamp_ms = self.timestamp_ms.saturating_add(elapsed_ms);
                if self.elapsed_ms < self.sample_period_ms {
                    return Vec::new();
                }
                self.elapsed_ms %= self.sample_period_ms;
                vec![Effect::Process(ProcessEffect::Signal(ProcessSignal {
                    tag: self.tag.clone(),
                    value: SignalValue::Analog(self.raw_value * self.gain + self.offset),
                    quality_good: self.operational && self.quality_good,
                    timestamp_ms: self.timestamp_ms,
                }))]
            }
            SimulationEvent::Process(ProcessEvent::Command(command)) if command.tag == self.tag => {
                match command.value {
                    SignalValue::Analog(value) => {
                        self.raw_value = value;
                        Vec::new()
                    }
                    _ => vec![Effect::Process(ProcessEffect::Alarm {
                        code: "SENSOR-TYPE-MISMATCH".into(),
                        active: true,
                        message: format!("{} requires an analog value", self.tag),
                    })],
                }
            }
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.quality_good = false;
                vec![Effect::Process(ProcessEffect::Alarm {
                    code: "SENSOR-BAD-QUALITY".into(),
                    active: true,
                    message: cause,
                })]
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                if authorized {
                    self.quality_good = true;
                }
                Vec::new()
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                Vec::new()
            }
            SimulationEvent::Network(_)
            | SimulationEvent::Process(ProcessEvent::Signals(_))
            | SimulationEvent::Process(ProcessEvent::Command(_)) => {
                vec![Effect::Drop(DropReason::UnsupportedProtocol)]
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Actuator {
    id: ComponentId,
    tag: String,
    actual: SignalValue,
    safe_value: SignalValue,
    failed: bool,
    operational: bool,
}

impl Actuator {
    pub fn new(
        id: ComponentId,
        tag: impl Into<String>,
        initial: SignalValue,
        safe_value: SignalValue,
    ) -> Self {
        Self {
            id,
            tag: tag.into(),
            actual: initial,
            safe_value,
            failed: false,
            operational: true,
        }
    }

    pub fn value(&self) -> &SignalValue {
        &self.actual
    }

    pub fn set_failed(&mut self, failed: bool) {
        self.failed = failed;
        if failed {
            self.actual = self.safe_value.clone();
        }
    }
}

impl SimulatedComponent for Actuator {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::FieldActuator
    }

    fn has_port(&self, _port: &PortId) -> bool {
        false
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Process(ProcessEvent::Command(command)) if command.tag == self.tag => {
                if !self.operational || self.failed {
                    return vec![Effect::Drop(DropReason::ComponentDown)];
                }
                self.actual = command.value;
                vec![Effect::Process(ProcessEffect::Output {
                    tag: self.tag.clone(),
                    value: self.actual.clone(),
                })]
            }
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.actual = self.safe_value.clone();
                vec![
                    Effect::Drop(DropReason::SafetyTrip(cause)),
                    Effect::Process(ProcessEffect::Output {
                        tag: self.tag.clone(),
                        value: self.actual.clone(),
                    }),
                ]
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.actual = self.safe_value.clone();
                }
                vec![Effect::Process(ProcessEffect::Output {
                    tag: self.tag.clone(),
                    value: self.actual.clone(),
                })]
            }
            SimulationEvent::Process(ProcessEvent::Tick { .. })
            | SimulationEvent::Process(ProcessEvent::Signals(_))
            | SimulationEvent::Process(ProcessEvent::Reset { .. }) => Vec::new(),
            SimulationEvent::Network(_) | SimulationEvent::Process(ProcessEvent::Command(_)) => {
                vec![Effect::Drop(DropReason::UnsupportedProtocol)]
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SafetyInterface {
    id: ComponentId,
    required_permissives: BTreeSet<String>,
    permissives: BTreeMap<String, bool>,
    trip_latched: bool,
    trip_cause: Option<String>,
    operational: bool,
}

impl SafetyInterface {
    pub fn new(id: ComponentId, required_permissives: impl IntoIterator<Item = String>) -> Self {
        let required_permissives = required_permissives.into_iter().collect::<BTreeSet<_>>();
        let permissives = required_permissives
            .iter()
            .cloned()
            .map(|tag| (tag, false))
            .collect();
        Self {
            id,
            required_permissives,
            permissives,
            trip_latched: true,
            trip_cause: Some("permissives not established".into()),
            operational: true,
        }
    }

    pub const fn trip_latched(&self) -> bool {
        self.trip_latched
    }

    fn all_permissive(&self) -> bool {
        self.required_permissives
            .iter()
            .all(|tag| self.permissives.get(tag) == Some(&true))
    }
}

impl SimulatedComponent for SafetyInterface {
    fn id(&self) -> &ComponentId {
        &self.id
    }

    fn kind(&self) -> ComponentKind {
        ComponentKind::SafetyInterface
    }

    fn has_port(&self, _port: &PortId) -> bool {
        false
    }

    fn handle(&mut self, event: SimulationEvent) -> Vec<Effect> {
        match event {
            SimulationEvent::Process(ProcessEvent::Signals(signals)) => {
                for (tag, signal) in signals {
                    if self.required_permissives.contains(&tag) {
                        let value =
                            matches!(signal.value, SignalValue::Bool(true)) && signal.quality_good;
                        self.permissives.insert(tag.clone(), value);
                        if !value {
                            self.trip_latched = true;
                            self.trip_cause = Some(format!("permissive lost: {tag}"));
                        }
                    }
                }
                vec![Effect::Process(ProcessEffect::Output {
                    tag: "safety-permissive".into(),
                    value: SignalValue::Bool(
                        self.operational && !self.trip_latched && self.all_permissive(),
                    ),
                })]
            }
            SimulationEvent::Process(ProcessEvent::Trip { cause }) => {
                self.trip_latched = true;
                self.trip_cause = Some(cause.clone());
                vec![Effect::Drop(DropReason::SafetyTrip(cause))]
            }
            SimulationEvent::Process(ProcessEvent::Reset { authorized }) => {
                if authorized && self.operational && self.all_permissive() {
                    self.trip_latched = false;
                    self.trip_cause = None;
                    vec![Effect::Process(ProcessEffect::Output {
                        tag: "safety-permissive".into(),
                        value: SignalValue::Bool(true),
                    })]
                } else {
                    vec![Effect::Drop(DropReason::SafetyTrip(
                        self.trip_cause
                            .clone()
                            .unwrap_or_else(|| "reset conditions not satisfied".into()),
                    ))]
                }
            }
            SimulationEvent::SetOperational(operational) => {
                self.operational = operational;
                if !operational {
                    self.trip_latched = true;
                    self.trip_cause = Some("safety interface unavailable".into());
                }
                Vec::new()
            }
            SimulationEvent::Process(ProcessEvent::Tick { .. })
            | SimulationEvent::Process(ProcessEvent::Command(_)) => Vec::new(),
            SimulationEvent::Network(_) => {
                vec![Effect::Drop(DropReason::UnsupportedProtocol)]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> ComponentId {
        ComponentId::new(value).expect("test ID")
    }

    fn signal(tag: &str, value: SignalValue) -> (String, ProcessSignal) {
        (
            tag.into(),
            ProcessSignal {
                tag: tag.into(),
                value,
                quality_good: true,
                timestamp_ms: 0,
            },
        )
    }

    #[test]
    fn virtual_plc_scans_only_after_period_and_updates_output() {
        let mut plc = VirtualPlc::new(
            id("area-01-vplc-01"),
            [],
            100,
            vec![LogicRule {
                input: "level-high".into(),
                comparison: Comparison::BoolEquals(true),
                output: "pump-run".into(),
                value_when_true: SignalValue::Bool(false),
                value_when_false: SignalValue::Bool(true),
            }],
        );
        plc.handle(SimulationEvent::Process(ProcessEvent::Signals(
            [signal("level-high", SignalValue::Bool(false))]
                .into_iter()
                .collect(),
        )));
        assert!(
            plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
                elapsed_ms: 99
            }))
            .is_empty()
        );
        let effects = plc.handle(SimulationEvent::Process(ProcessEvent::Tick {
            elapsed_ms: 1,
        }));
        assert_eq!(effects.len(), 1);
        assert_eq!(
            plc.outputs().get("pump-run"),
            Some(&SignalValue::Bool(true))
        );
    }

    #[test]
    fn safety_reset_requires_authorization_and_all_permissives() {
        let mut safety = SafetyInterface::new(
            id("area-06-bms-01"),
            ["airflow-ok".into(), "gas-pressure-ok".into()],
        );
        safety.handle(SimulationEvent::Process(ProcessEvent::Signals(
            [
                signal("airflow-ok", SignalValue::Bool(true)),
                signal("gas-pressure-ok", SignalValue::Bool(true)),
            ]
            .into_iter()
            .collect(),
        )));
        let denied = safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
            authorized: false,
        }));
        assert!(matches!(denied[0], Effect::Drop(DropReason::SafetyTrip(_))));
        safety.handle(SimulationEvent::Process(ProcessEvent::Reset {
            authorized: true,
        }));
        assert!(!safety.trip_latched());
    }
}

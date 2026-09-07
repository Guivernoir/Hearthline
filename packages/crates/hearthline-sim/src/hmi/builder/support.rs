use hearthline_engine::{Effect, EffectList, ProcessEffect};
use hearthline_model::{ComponentId, PortId, SignalValue, TELEMETRY_NOMINAL_PAYLOAD_BYTES};
use serde::Serialize;

use crate::{ConfigError, ScenarioApplicationConfig, ScenarioPacketConfig};

use super::super::{HmiSnapshot, HmiTraceEntry};

#[derive(Serialize)]
#[doc = "hearthline:projection-boundary"]
struct FormingTelemetryPayload<'a> {
    v: u8,
    cell: &'a str,
    mould: &'a str,
    phase: &'a str,
    cycle: u64,
    tank: f64,
    temp_c: f64,
    density: f64,
    visc: f64,
    press_bar: f64,
    moist_pct: f64,
    air_bar: f64,
    vac_kpa: f64,
    robot_mm: f64,
    grip: bool,
}

pub fn build_forming_telemetry_packet(
    snapshot: &HmiSnapshot,
    mut packet: ScenarioPacketConfig,
) -> Result<ScenarioPacketConfig, ConfigError> {
    let process = snapshot.process.as_ref().ok_or_else(|| {
        ConfigError::new(format!(
            "operator interface {} has no process state",
            snapshot.id
        ))
    })?;
    if process.model != "ceramic-slip-pressure-casting-cell" {
        return Err(ConfigError::new(format!(
            "operator interface {} does not expose the Forming process model",
            snapshot.id
        )));
    }
    let service = match &packet.application {
        ScenarioApplicationConfig::Service { service }
        | ScenarioApplicationConfig::Telemetry { service, .. } => service.clone(),
        _ => {
            return Err(ConfigError::new(
                "Forming telemetry requires a service or telemetry scenario packet",
            ));
        }
    };
    let mould = snapshot
        .moulds
        .iter()
        .find(|mould| mould.running || mould.paused)
        .or_else(|| snapshot.moulds.first())
        .ok_or_else(|| ConfigError::new("Forming telemetry requires at least one mould"))?;
    let payload = FormingTelemetryPayload {
        v: 1,
        cell: &snapshot.zone,
        mould: &mould.target,
        phase: mould.phase,
        cycle: mould.cycle_count,
        tank: telemetry_precision(signal(snapshot, "area-02-lt-01")?),
        temp_c: telemetry_precision(signal(snapshot, "area-02-tt-01")?),
        density: telemetry_precision(signal(snapshot, "area-02-dt-01")?),
        visc: telemetry_precision(signal(snapshot, "area-02-vis-01")?),
        press_bar: telemetry_precision(signal(snapshot, "area-02-pt-02")?),
        moist_pct: telemetry_precision(signal(snapshot, "area-02-mt-02")?),
        air_bar: telemetry_precision(signal(snapshot, "area-02-pt-04")?),
        vac_kpa: telemetry_precision(signal(snapshot, "area-02-vt-01")?),
        robot_mm: telemetry_precision(signal(snapshot, "area-02-pos-03")?),
        grip: signal(snapshot, "area-02-pe-01")? >= 0.5,
    };
    let payload = serde_json::to_string(&payload)
        .map_err(|error| ConfigError::new(format!("cannot encode Forming telemetry: {error}")))?;
    if payload.len() > TELEMETRY_NOMINAL_PAYLOAD_BYTES {
        return Err(ConfigError::new(format!(
            "Forming telemetry uses {} bytes and exceeds its {}-byte nominal budget",
            payload.len(),
            TELEMETRY_NOMINAL_PAYLOAD_BYTES
        )));
    }
    packet.wire_length_bytes = u16::try_from(54_usize.saturating_add(payload.len()))
        .map_err(|_| ConfigError::new("Forming telemetry frame length exceeds u16"))?
        .max(64);
    packet.application = ScenarioApplicationConfig::Telemetry {
        service,
        source: snapshot.controller.clone(),
        sequence: mould.scan_count,
        payload,
    };
    packet.validate()?;
    Ok(packet)
}

fn telemetry_precision(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

fn signal(snapshot: &HmiSnapshot, tag: &str) -> Result<f64, ConfigError> {
    snapshot
        .signals
        .iter()
        .find(|signal| signal.tag == tag)
        .map(|signal| signal.value)
        .ok_or_else(|| {
            ConfigError::new(format!(
                "operator interface {} is missing telemetry signal {tag}",
                snapshot.id
            ))
        })
}

pub(in crate::hmi) fn component_id(value: &str) -> ComponentId {
    ComponentId::new(value).expect("validated HMI component id")
}

pub(in crate::hmi) fn ports(values: &[String]) -> impl Iterator<Item = PortId> + '_ {
    values
        .iter()
        .map(|value| PortId::new(value).expect("validated HMI port id"))
}

pub(in crate::hmi) fn forwards_command(effects: EffectList) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::Process(ProcessEffect::Command(_))))
}

pub(in crate::hmi) fn produces_output(effects: EffectList) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::Process(ProcessEffect::Output { .. })))
}

pub(in crate::hmi) fn produces_true_output(effects: EffectList) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::Process(ProcessEffect::Output {
                value: SignalValue::Bool(true),
                ..
            })
        )
    })
}

pub(in crate::hmi) fn signal_value_text(value: &SignalValue) -> String {
    match value {
        SignalValue::Text(value) => value.to_string(),
        SignalValue::Bool(value) => value.to_string(),
        SignalValue::Analog(value) => value.to_string(),
        SignalValue::Integer(value) => value.to_string(),
    }
}

pub(in crate::hmi) fn trace_entry(component: &str, stage: &str, detail: String) -> HmiTraceEntry {
    HmiTraceEntry {
        sequence: 0,
        component: component.into(),
        stage: stage.into(),
        detail,
    }
}

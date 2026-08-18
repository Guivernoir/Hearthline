use std::collections::BTreeMap;

use hearthline_engine::{FormingMeasurements, FormingSetpoints};

use crate::appliance::source_revision;
use crate::{
    BehaviorConfig, ConfigError, ConfigRepository, MouldControlCabinetConfig,
    MouldUtilityCabinetConfig,
};

use super::super::HmiParameter;
use super::super::actions::process::ConfiguredControlProgram;
use super::super::state::{MouldProcessRuntime, RobotRuntime};

pub(super) fn robot_runtime(
    appliances: &ConfigRepository,
    environment: &str,
    zone: &str,
) -> Result<Option<RobotRuntime>, ConfigError> {
    let profile = appliances.appliances().find_map(|candidate| {
        if candidate.config.environment != environment || candidate.config.zone != zone {
            return None;
        }
        let BehaviorConfig::FieldActuator {
            motion_profile: Some(profile),
            ..
        } = &candidate.config.behavior
        else {
            return None;
        };
        Some(profile)
    });
    let Some(profile) = profile else {
        return Ok(None);
    };
    let (_, source) = appliances.read_project_source(&profile.program_ref)?;
    let revision = source_revision(&source);
    RobotRuntime::from_profile(profile, source, revision).map(Some)
}

pub(super) fn forming_moulds(
    appliances: &ConfigRepository,
    environment: &str,
    zone: &str,
    program: &ConfiguredControlProgram,
    parameters: &[HmiParameter],
) -> Result<BTreeMap<String, MouldProcessRuntime>, ConfigError> {
    let value = |tag| forming_initial_value(appliances, environment, zone, tag);
    let station_tags = [
        (
            "area-02-pt-02",
            "area-02-tt-02",
            "area-02-pos-01",
            "area-02-pos-02",
            "area-02-mt-02",
        ),
        (
            "area-02-m02-pt-01",
            "area-02-m02-tt-01",
            "area-02-m02-pos-01",
            "area-02-m02-pos-02",
            "area-02-m02-mt-01",
        ),
        (
            "area-02-m03-pt-01",
            "area-02-m03-tt-01",
            "area-02-m03-pos-01",
            "area-02-m03-pos-02",
            "area-02-m03-mt-01",
        ),
        (
            "area-02-m04-pt-01",
            "area-02-m04-tt-01",
            "area-02-m04-pos-01",
            "area-02-m04-pos-02",
            "area-02-m04-mt-01",
        ),
    ];
    let mut moulds = BTreeMap::new();
    for (index, (pressure, temperature, fill_head, position, moisture)) in
        station_tags.into_iter().enumerate()
    {
        let number = index + 1;
        let target = format!("mould-{number:02}");
        let measurements = FormingMeasurements {
            slip_tank_level_percent: value("area-02-lt-01")?,
            slip_density_g_cm3: value("area-02-dt-01")?,
            slip_viscosity_mpa_s: value("area-02-vis-01")?,
            slip_temperature_c: value("area-02-tt-01")?,
            slip_feed_flow_l_min: value("area-02-ft-01")?,
            slip_feed_pressure_bar: value("area-02-pt-01")?,
            mould_pressure_bar: value(pressure)?,
            mould_temperature_c: value(temperature)?,
            fill_head_position_mm: value(fill_head)?,
            mould_position_mm: value(position)?,
            water_flow_l_min: value("area-02-ft-02")?,
            excess_slip_drain_flow_l_min: value("area-02-ft-03")?,
            mould_moisture_percent: value(moisture)?,
            compressed_air_pressure_bar: value("area-02-pt-04")?,
            vacuum_pressure_kpa: value("area-02-vt-01")?,
            robot_position_mm: value("area-02-pos-03")?,
            piece_gripped: value("area-02-pe-01")? >= 0.5,
            piece_moisture_percent: 20.5,
            predicted_drying_shrinkage_percent: 2.1,
            drying_energy_factor: 1.0,
            green_strength_index: 100.0,
            fired_defect_risk_percent: 3.0,
        };
        let setpoints = forming_setpoints(parameters, &target)?;
        let (control_cabinet, utility_cabinet) = mould_cabinets(appliances, &target);
        moulds.insert(
            target.clone(),
            MouldProcessRuntime::new(
                target,
                format!("Mould {number}"),
                measurements,
                program.clone(),
                setpoints,
                control_cabinet,
                utility_cabinet,
            ),
        );
    }
    Ok(moulds)
}

type MouldCabinets = (
    Option<(String, MouldControlCabinetConfig)>,
    Option<(String, MouldUtilityCabinetConfig)>,
);

fn mould_cabinets(appliances: &ConfigRepository, target: &str) -> MouldCabinets {
    let control = appliances.appliances().find_map(|candidate| {
        let BehaviorConfig::RemoteIo {
            control_cabinet: Some(cabinet),
            ..
        } = &candidate.config.behavior
        else {
            return None;
        };
        (cabinet.target == target).then(|| (candidate.config.id.clone(), cabinet.clone()))
    });
    let utility = appliances.appliances().find_map(|candidate| {
        let BehaviorConfig::FieldActuator {
            utility_cabinet: Some(cabinet),
            ..
        } = &candidate.config.behavior
        else {
            return None;
        };
        (cabinet.target == target).then(|| (candidate.config.id.clone(), cabinet.clone()))
    });
    (control, utility)
}

fn forming_setpoints(
    parameters: &[HmiParameter],
    target: &str,
) -> Result<FormingSetpoints, ConfigError> {
    let value = |suffix: &str| {
        parameters
            .iter()
            .find(|parameter| parameter.target == target && parameter.id.ends_with(suffix))
            .map(|parameter| parameter.value)
            .ok_or_else(|| {
                ConfigError::new(format!(
                    "forming mould {target} requires machine parameter *{suffix}"
                ))
            })
    };
    Ok(FormingSetpoints {
        fill_ms: value("-fill-ms")?.round() as u64,
        pressure_bar: value("-pressure-bar")?,
        dwell_ms: value("-dwell-ms")?.round() as u64,
        drain_ms: value("-drain-ms")?.round() as u64,
        pickup_delay_ms: value("-pickup-delay-ms")?.round() as u64,
        wash_ms: value("-wash-ms")?.round() as u64,
        vacuum_ms: value("-vacuum-ms")?.round() as u64,
    })
}

pub(super) fn forming_initial_value(
    appliances: &ConfigRepository,
    environment: &str,
    zone: &str,
    tag: &str,
) -> Result<f64, ConfigError> {
    appliances
        .appliances()
        .find_map(|candidate| {
            if candidate.config.environment != environment || candidate.config.zone != zone {
                return None;
            }
            let BehaviorConfig::FieldSensor {
                signal_tag,
                initial_value,
                ..
            } = &candidate.config.behavior
            else {
                return None;
            };
            (signal_tag == tag).then_some(*initial_value)
        })
        .flatten()
        .ok_or_else(|| ConfigError::new(format!("forming process requires initial signal {tag}")))
}

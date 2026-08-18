use super::{BodyPreparationFault, GlazePhase, ReturnWaterPhase, SlipPhase, WaterQuality};

pub const WATER_NETWORK_PUMP_COUNT: usize = 16;
pub const WATER_NETWORK_ROUTE_COUNT: usize = 8;
pub const PUMP_HEARTBEAT_INTERVAL_MS: u64 = 500;
pub const PUMP_HEARTBEAT_TIMEOUT_MS: u64 = 1_500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PumpMaintenanceState {
    Normal,
    Required,
    Dispatched,
}

impl PumpMaintenanceState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Required => "required",
            Self::Dispatched => "dispatched",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterPumpMeasurements {
    pub id: &'static str,
    pub label: &'static str,
    pub group_id: &'static str,
    pub service: &'static str,
    pub preferred_duty: bool,
    pub commanded: bool,
    pub running_feedback: bool,
    pub heartbeat_sequence: u64,
    pub heartbeat_age_ms: u64,
    pub heartbeat_ok: bool,
    pub maintenance: PumpMaintenanceState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterRouteMeasurements {
    pub id: &'static str,
    pub label: &'static str,
    pub network: &'static str,
    pub source: &'static str,
    pub destination: &'static str,
    pub pump_group: &'static str,
    pub demanded: bool,
    pub available: bool,
    pub inlet_flow_l_min: f64,
    pub outlet_flow_l_min: f64,
    pub inlet_pressure_bar: f64,
    pub outlet_pressure_bar: f64,
    pub leak_detected: bool,
    pub quality: WaterQuality,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterNetworkMeasurements {
    pub pumps: [WaterPumpMeasurements; WATER_NETWORK_PUMP_COUNT],
    pub routes: [WaterRouteMeasurements; WATER_NETWORK_ROUTE_COUNT],
}

#[derive(Clone, Copy, Debug)]
struct PumpRuntime {
    failed: bool,
    heartbeat_sequence: u64,
    heartbeat_elapsed_ms: u64,
    heartbeat_age_ms: u64,
    maintenance: PumpMaintenanceState,
}

impl PumpRuntime {
    const fn new() -> Self {
        Self {
            failed: false,
            heartbeat_sequence: 0,
            heartbeat_elapsed_ms: 0,
            heartbeat_age_ms: 0,
            maintenance: PumpMaintenanceState::Normal,
        }
    }

    fn tick(&mut self, elapsed_ms: u64) {
        if self.failed {
            self.heartbeat_age_ms = self.heartbeat_age_ms.saturating_add(elapsed_ms);
            if self.heartbeat_age_ms >= PUMP_HEARTBEAT_TIMEOUT_MS
                && self.maintenance == PumpMaintenanceState::Normal
            {
                self.maintenance = PumpMaintenanceState::Required;
            }
            return;
        }
        self.heartbeat_elapsed_ms = self.heartbeat_elapsed_ms.saturating_add(elapsed_ms);
        let pulses = self.heartbeat_elapsed_ms / PUMP_HEARTBEAT_INTERVAL_MS;
        self.heartbeat_elapsed_ms %= PUMP_HEARTBEAT_INTERVAL_MS;
        self.heartbeat_sequence = self.heartbeat_sequence.saturating_add(pulses);
        self.heartbeat_age_ms = 0;
        self.maintenance = PumpMaintenanceState::Normal;
    }

    const fn heartbeat_ok(self) -> bool {
        self.heartbeat_age_ms < PUMP_HEARTBEAT_TIMEOUT_MS
    }
}

#[derive(Clone, Debug)]
pub(in crate::industrial::process::body_preparation) struct WaterNetworkRuntime {
    pumps: [PumpRuntime; WATER_NETWORK_PUMP_COUNT],
    pub measurements: WaterNetworkMeasurements,
}

const PUMP_DEFINITIONS: [(&str, &str, &str, &str, bool); WATER_NETWORK_PUMP_COUNT] = [
    (
        "area-01-wd-pmp-01a",
        "Header pump A",
        "industrial-header",
        "Industrial-water header",
        true,
    ),
    (
        "area-01-wd-pmp-01b",
        "Header pump B",
        "industrial-header",
        "Industrial-water header",
        false,
    ),
    (
        "area-01-wd-pmp-02a",
        "Slip booster A",
        "industrial-slip",
        "Slip Preparation delivery",
        true,
    ),
    (
        "area-01-wd-pmp-02b",
        "Slip booster B",
        "industrial-slip",
        "Slip Preparation delivery",
        false,
    ),
    (
        "area-01-wd-pmp-03a",
        "Glaze booster A",
        "industrial-glaze",
        "Glaze Preparation delivery",
        true,
    ),
    (
        "area-01-wd-pmp-03b",
        "Glaze booster B",
        "industrial-glaze",
        "Glaze Preparation delivery",
        false,
    ),
    (
        "area-01-wd-pmp-04a",
        "Forming booster A",
        "industrial-forming",
        "Forming service-water delivery",
        true,
    ),
    (
        "area-01-wd-pmp-04b",
        "Forming booster B",
        "industrial-forming",
        "Forming service-water delivery",
        false,
    ),
    (
        "area-01-rc-pmp-01a",
        "Body-return pump A",
        "return-body-collection",
        "Body-return collection",
        true,
    ),
    (
        "area-01-rc-pmp-01b",
        "Body-return pump B",
        "return-body-collection",
        "Body-return collection",
        false,
    ),
    (
        "area-01-rc-pmp-02a",
        "Glaze-return pump A",
        "return-glaze-collection",
        "Glaze-return collection",
        true,
    ),
    (
        "area-01-rc-pmp-02b",
        "Glaze-return pump B",
        "return-glaze-collection",
        "Glaze-return collection",
        false,
    ),
    (
        "area-01-rc-pmp-03a",
        "Body-reuse pump A",
        "return-body-reuse",
        "Body-water reuse delivery",
        true,
    ),
    (
        "area-01-rc-pmp-03b",
        "Body-reuse pump B",
        "return-body-reuse",
        "Body-water reuse delivery",
        false,
    ),
    (
        "area-01-rc-pmp-04a",
        "Glaze-reuse pump A",
        "return-glaze-reuse",
        "Glaze-water reuse delivery",
        true,
    ),
    (
        "area-01-rc-pmp-04b",
        "Glaze-reuse pump B",
        "return-glaze-reuse",
        "Glaze-water reuse delivery",
        false,
    ),
];

impl WaterNetworkRuntime {
    pub const fn new() -> Self {
        let default_quality = WaterQuality::treated_default();
        Self {
            pumps: [PumpRuntime::new(); WATER_NETWORK_PUMP_COUNT],
            measurements: WaterNetworkMeasurements {
                pumps: pump_measurements(),
                routes: route_measurements(default_quality),
            },
        }
    }

    pub fn tick(&mut self, elapsed_ms: u64, context: WaterNetworkContext) {
        for pump in &mut self.pumps {
            pump.tick(elapsed_ms);
        }
        let demands = [
            true,
            context.slip_phase == SlipPhase::WaterCharge,
            context.glaze_phase == GlazePhase::WaterCharge,
            context.return_phase != ReturnWaterPhase::Faulted,
            context.return_phase != ReturnWaterPhase::Faulted,
            true,
            context.slip_phase == SlipPhase::WaterCharge,
            context.glaze_phase == GlazePhase::WaterCharge,
        ];
        let flows = [72.0, 80.0, 55.0, 36.0, 42.0, 28.0, 32.0, 24.0];
        let pressures = [4.2, 3.1, 3.0, 3.4, 2.8, 2.6, 2.9, 2.7];
        let qualities = [
            context.industrial_quality,
            context.industrial_quality,
            context.industrial_quality,
            context.industrial_quality,
            context.body_return_quality,
            context.glaze_return_quality,
            context.body_return_quality,
            context.glaze_return_quality,
        ];
        let leaks = [
            false,
            context.fault == Some(BodyPreparationFault::WaterToSlipLeak),
            context.fault == Some(BodyPreparationFault::WaterToGlazeLeak),
            false,
            false,
            false,
            false,
            false,
        ];

        for group in 0..WATER_NETWORK_ROUTE_COUNT {
            let primary = group * 2;
            let standby = primary + 1;
            let primary_ok = self.pumps[primary].heartbeat_ok();
            let standby_ok = self.pumps[standby].heartbeat_ok();
            let selected = if primary_ok {
                Some(primary)
            } else if standby_ok {
                Some(standby)
            } else {
                None
            };
            let available = selected.is_some();
            for index in [primary, standby] {
                let definition = PUMP_DEFINITIONS[index];
                let running = demands[group] && selected == Some(index);
                let runtime = self.pumps[index];
                self.measurements.pumps[index] = WaterPumpMeasurements {
                    id: definition.0,
                    label: definition.1,
                    group_id: definition.2,
                    service: definition.3,
                    preferred_duty: definition.4,
                    commanded: running,
                    running_feedback: running && runtime.heartbeat_ok(),
                    heartbeat_sequence: runtime.heartbeat_sequence,
                    heartbeat_age_ms: runtime.heartbeat_age_ms,
                    heartbeat_ok: runtime.heartbeat_ok(),
                    maintenance: runtime.maintenance,
                };
            }
            let leaking = leaks[group] && demands[group] && available;
            let route = &mut self.measurements.routes[group];
            route.demanded = demands[group];
            route.available = available;
            route.inlet_flow_l_min = if demands[group] && available {
                flows[group]
            } else {
                0.0
            };
            route.outlet_flow_l_min = route.inlet_flow_l_min * if leaking { 0.72 } else { 0.99 };
            route.inlet_pressure_bar = if available { pressures[group] } else { 0.0 };
            route.outlet_pressure_bar = route.inlet_pressure_bar
                * if leaking {
                    0.46
                } else if demands[group] {
                    0.91
                } else {
                    0.97
                };
            route.leak_detected = leaking;
            route.quality = qualities[group];
        }
    }

    pub fn set_pump_failed(&mut self, id: &str, failed: bool) -> bool {
        let Some(index) = PUMP_DEFINITIONS.iter().position(|pump| pump.0 == id) else {
            return false;
        };
        let pump = &mut self.pumps[index];
        pump.failed = failed;
        if !failed {
            pump.heartbeat_age_ms = 0;
            pump.maintenance = PumpMaintenanceState::Normal;
        }
        true
    }

    pub fn dispatch_maintenance(&mut self, id: &str) -> bool {
        let Some(index) = PUMP_DEFINITIONS.iter().position(|pump| pump.0 == id) else {
            return false;
        };
        if self.pumps[index].maintenance != PumpMaintenanceState::Required {
            return false;
        }
        self.pumps[index].maintenance = PumpMaintenanceState::Dispatched;
        self.measurements.pumps[index].maintenance = PumpMaintenanceState::Dispatched;
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::industrial::process::body_preparation) struct WaterNetworkContext {
    pub industrial_quality: WaterQuality,
    pub body_return_quality: WaterQuality,
    pub glaze_return_quality: WaterQuality,
    pub slip_phase: SlipPhase,
    pub glaze_phase: GlazePhase,
    pub return_phase: ReturnWaterPhase,
    pub fault: Option<BodyPreparationFault>,
}

const fn pump_measurements() -> [WaterPumpMeasurements; WATER_NETWORK_PUMP_COUNT] {
    let mut values = [WaterPumpMeasurements {
        id: "",
        label: "",
        group_id: "",
        service: "",
        preferred_duty: false,
        commanded: false,
        running_feedback: false,
        heartbeat_sequence: 0,
        heartbeat_age_ms: 0,
        heartbeat_ok: true,
        maintenance: PumpMaintenanceState::Normal,
    }; WATER_NETWORK_PUMP_COUNT];
    let mut index = 0;
    while index < WATER_NETWORK_PUMP_COUNT {
        let definition = PUMP_DEFINITIONS[index];
        values[index].id = definition.0;
        values[index].label = definition.1;
        values[index].group_id = definition.2;
        values[index].service = definition.3;
        values[index].preferred_duty = definition.4;
        index += 1;
    }
    values
}

const fn route_measurements(
    quality: WaterQuality,
) -> [WaterRouteMeasurements; WATER_NETWORK_ROUTE_COUNT] {
    [
        route(
            "industrial-header",
            "Industrial-water header",
            "industrial",
            "Treated-water tank",
            "Factory ring main",
            quality,
        ),
        route(
            "industrial-slip",
            "Slip Preparation branch",
            "industrial",
            "Factory ring main",
            "Slip Preparation",
            quality,
        ),
        route(
            "industrial-glaze",
            "Glaze Preparation branch",
            "industrial",
            "Factory ring main",
            "Glaze Preparation",
            quality,
        ),
        route(
            "industrial-forming",
            "Forming service-water branch",
            "industrial",
            "Factory ring main",
            "Forming",
            quality,
        ),
        route(
            "return-body-collection",
            "Body-return collection",
            "return",
            "Body process drains",
            "Body equalization",
            quality,
        ),
        route(
            "return-glaze-collection",
            "Glaze-return collection",
            "return",
            "Glaze process drains",
            "Glaze equalization",
            quality,
        ),
        route(
            "return-body-reuse",
            "Body-water reuse delivery",
            "return",
            "Body reuse tank",
            "Slip Preparation",
            quality,
        ),
        route(
            "return-glaze-reuse",
            "Glaze-water reuse delivery",
            "return",
            "Glaze reuse tank",
            "Glaze Preparation",
            quality,
        ),
    ]
}

const fn route(
    id: &'static str,
    label: &'static str,
    network: &'static str,
    source: &'static str,
    destination: &'static str,
    quality: WaterQuality,
) -> WaterRouteMeasurements {
    WaterRouteMeasurements {
        id,
        label,
        network,
        source,
        destination,
        pump_group: id,
        demanded: false,
        available: true,
        inlet_flow_l_min: 0.0,
        outlet_flow_l_min: 0.0,
        inlet_pressure_bar: 0.0,
        outlet_pressure_bar: 0.0,
        leak_detected: false,
        quality,
    }
}

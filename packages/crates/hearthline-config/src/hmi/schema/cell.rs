use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiGuardedCellState {
    pub guard: HmiCellGuardState,
    pub handoff_stations: Vec<HmiHandoffStationState>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiCellGuardState {
    pub safety_component: String,
    pub position_sensor: String,
    pub position: &'static str,
    pub closed_permissive: bool,
    pub reset_required: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HmiHandoffStationState {
    pub mould: String,
    pub actuator: String,
    pub state: &'static str,
    pub progress_percent: f64,
    pub in_cell_sensor: String,
    pub operator_side_sensor: String,
    pub in_cell_confirmed: bool,
    pub operator_side_confirmed: bool,
    pub piece_present: bool,
}

mod body_preparation;
mod control;
mod field;
mod forming;
mod io;
mod safety;

mod storage {
    use heapless::Vec as FixedList;
    use hearthline_model::{PortId, Text};

    use crate::runtime::collect_fixed;

    pub(crate) type Ports = FixedList<PortId, 32>;
    pub(crate) type TaggedValues<T> = FixedList<(Text<64>, T), 64>;

    pub(crate) fn collect_ports(values: impl IntoIterator<Item = PortId>) -> Ports {
        collect_fixed(values)
    }

    pub(crate) fn tagged_values<T>(
        values: impl IntoIterator<Item = (Text<64>, T)>,
    ) -> TaggedValues<T> {
        collect_fixed(values)
    }

    pub(crate) fn get<'a, T>(values: &'a TaggedValues<T>, tag: &Text<64>) -> Option<&'a T> {
        values
            .iter()
            .find(|(candidate, _)| candidate == tag)
            .map(|(_, value)| value)
    }

    pub(crate) fn upsert<T>(values: &mut TaggedValues<T>, tag: Text<64>, value: T) {
        if let Some((_, current)) = values.iter_mut().find(|(candidate, _)| *candidate == tag) {
            *current = value;
        } else {
            assert!(
                values.push((tag, value)).is_ok(),
                "tagged runtime table exceeds capacity"
            );
        }
    }
}

use hearthline_model::{ApplicationData, NetworkPayload, ServiceKind};

pub use body_preparation::{
    BodyPreparationFault, BodyPreparationMeasurements, BodyPreparationOutputs,
    BodyPreparationPhase, BodyPreparationPipelineMeasurements, BodyPreparationProcess,
    BodyPreparationSetpoints, BodyPreparationStartError, BodyPreparationTick, BodyPreparationTrip,
    CeramicSlipBatch, DownstreamMaterialEffects, GlazeBatch, GlazeMeasurements, GlazePhase,
    GlazeSetpoints, HandoffPipelineMeasurements, PUMP_HEARTBEAT_INTERVAL_MS,
    PUMP_HEARTBEAT_TIMEOUT_MS, PreparationTrain, PumpMaintenanceState, ReturnWaterMeasurements,
    ReturnWaterPhase, SIMULATED_MS_PER_PROCESS_MINUTE, SlipMeasurements, SlipPhase, SlipSetpoints,
    WATER_NETWORK_PUMP_COUNT, WATER_NETWORK_ROUTE_COUNT, WaterMeasurements,
    WaterNetworkMeasurements, WaterPhase, WaterPumpMeasurements, WaterQuality,
    WaterRouteMeasurements, WaterSetpoints,
};
pub use control::{Comparison, LogicRule, OperatorInterface, VirtualPlc};
pub use field::{Actuator, FieldSensor};
pub use forming::{
    FormingFault, FormingMeasurements, FormingOutputs, FormingPhase, FormingProcess,
    FormingSetpoints, FormingStartError, FormingTick, FormingTrip,
};
pub use io::{IoDirection, RemoteIo};
pub use safety::SafetyInterface;

fn is_industrial_communication(frame: &hearthline_model::EthernetFrame) -> bool {
    let NetworkPayload::Ipv4(packet) = &frame.payload else {
        return false;
    };
    matches!(
        packet.application,
        ApplicationData::Service(ServiceKind::IndustrialIo | ServiceKind::PlcEngineering)
    ) || matches!(packet.transport.destination_port(), Some(502 | 4840))
}

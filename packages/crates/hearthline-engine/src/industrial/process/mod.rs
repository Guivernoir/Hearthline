mod control;
mod field;
mod forming;
mod io;
mod safety;
mod storage;

use hearthline_model::{ApplicationData, NetworkPayload, ServiceKind};

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

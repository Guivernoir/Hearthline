mod media;
mod port;

pub use media::{
    CarrierMedium, ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, FiberMedium,
    FiberMode, FieldWiringMedium, MediaError, MediaFacts, MediaText, MediumKind, RadioMedium,
    SimulatedMedium, TelephoneMedium, VirtualMedium,
};
pub use port::{
    PortDuplex, PortHardwareKind, PortSettings, PortState, PortStateConfig, SimulatedPort,
    appliance_supports_port,
};

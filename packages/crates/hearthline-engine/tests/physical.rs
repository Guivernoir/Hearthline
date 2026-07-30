use hearthline_engine::{MediumKind, PortHardwareKind, appliance_supports_port};
use hearthline_model::ComponentKind;

#[test]
fn router_rejects_telephone_cabling() {
    assert!(!appliance_supports_port(
        ComponentKind::Router,
        PortHardwareKind::TelephoneRj11
    ));
    assert!(appliance_supports_port(
        ComponentKind::VoiceGateway,
        PortHardwareKind::TelephoneRj11
    ));
}

#[test]
fn port_hardware_owns_media_capability() {
    assert!(PortHardwareKind::EthernetRj45.supports(MediumKind::Copper));
    assert!(!PortHardwareKind::EthernetRj45.supports(MediumKind::Telephone));
    assert!(PortHardwareKind::TelephoneRj11.supports(MediumKind::Telephone));
}

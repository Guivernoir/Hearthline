use core::fmt::{self, Display, Formatter};

use hearthline_model::ComponentKind;
use serde::Deserialize;

use super::media::MediumKind;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PortHardwareKind {
    EthernetRj45,
    EthernetSfp,
    WirelessRadio,
    CarrierDemarc,
    VirtualNic,
    FieldIoChannel,
    TelephoneRj11,
}

impl PortHardwareKind {
    pub const fn supports(self, medium: MediumKind) -> bool {
        matches!(
            (self, medium),
            (Self::EthernetRj45, MediumKind::Copper)
                | (Self::EthernetSfp, MediumKind::Fiber)
                | (Self::WirelessRadio, MediumKind::Radio)
                | (Self::CarrierDemarc, MediumKind::Carrier)
                | (Self::VirtualNic, MediumKind::Virtual)
                | (Self::FieldIoChannel, MediumKind::FieldWiring)
                | (Self::TelephoneRj11, MediumKind::Telephone)
        )
    }

    pub const fn supported_media(self) -> &'static [MediumKind] {
        match self {
            Self::EthernetRj45 => &[MediumKind::Copper],
            Self::EthernetSfp => &[MediumKind::Fiber],
            Self::WirelessRadio => &[MediumKind::Radio],
            Self::CarrierDemarc => &[MediumKind::Carrier],
            Self::VirtualNic => &[MediumKind::Virtual],
            Self::FieldIoChannel => &[MediumKind::FieldWiring],
            Self::TelephoneRj11 => &[MediumKind::Telephone],
        }
    }
}

impl Display for PortHardwareKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::EthernetRj45 => "ethernet-rj45",
            Self::EthernetSfp => "ethernet-sfp",
            Self::WirelessRadio => "wireless-radio",
            Self::CarrierDemarc => "carrier-demarc",
            Self::VirtualNic => "virtual-nic",
            Self::FieldIoChannel => "field-io-channel",
            Self::TelephoneRj11 => "telephone-rj11",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PortState {
    Up,
    Down,
}

impl Display for PortState {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Up => formatter.write_str("up"),
            Self::Down => formatter.write_str("down"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PortStateConfig {
    pub administrative: PortState,
    pub initial_operational: PortState,
}

impl PortStateConfig {
    pub const fn initially_usable(self) -> bool {
        matches!(
            (self.administrative, self.initial_operational),
            (PortState::Up, PortState::Up)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum PortDuplex {
    Auto,
    Full,
    Half,
}

impl Display for PortDuplex {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Auto => "auto",
            Self::Full => "full",
            Self::Half => "half",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PortSettings {
    pub speed_mbps: u64,
    pub duplex: PortDuplex,
    pub mtu: u32,
}

impl PortSettings {
    pub fn validate(self) -> Result<(), &'static str> {
        if self.speed_mbps == 0 {
            return Err("configured port speed must be non-zero");
        }
        if self.mtu < 64 {
            return Err("configured port MTU must be at least 64 bytes");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulatedPort {
    pub hardware: PortHardwareKind,
    pub state: PortStateConfig,
    pub settings: PortSettings,
}

impl SimulatedPort {
    pub const fn is_operational(self) -> bool {
        self.state.initially_usable()
    }

    pub fn set_operational(&mut self, state: PortState) {
        self.state.initial_operational = state;
    }
}

pub const fn appliance_supports_port(appliance: ComponentKind, port: PortHardwareKind) -> bool {
    match port {
        PortHardwareKind::EthernetRj45 => !matches!(
            appliance,
            ComponentKind::WanCircuit
                | ComponentKind::VirtualPlc
                | ComponentKind::FieldSensor
                | ComponentKind::FieldActuator
                | ComponentKind::SafetyInterface
        ),
        PortHardwareKind::EthernetSfp => matches!(
            appliance,
            ComponentKind::Layer2Switch
                | ComponentKind::Layer3Switch
                | ComponentKind::Router
                | ComponentKind::NatRouter
                | ComponentKind::Firewall
                | ComponentKind::VirtualizationHost
        ),
        PortHardwareKind::WirelessRadio => matches!(
            appliance,
            ComponentKind::WirelessAccessPoint
                | ComponentKind::Workstation
                | ComponentKind::Printer
        ),
        PortHardwareKind::CarrierDemarc => matches!(
            appliance,
            ComponentKind::Router
                | ComponentKind::Firewall
                | ComponentKind::TransparentCpe
                | ComponentKind::WanCircuit
                | ComponentKind::EncryptedConduit
        ),
        PortHardwareKind::VirtualNic => matches!(
            appliance,
            ComponentKind::Layer3Switch
                | ComponentKind::VirtualizationHost
                | ComponentKind::VirtualPlc
                | ComponentKind::EncryptedConduit
        ),
        PortHardwareKind::FieldIoChannel => matches!(
            appliance,
            ComponentKind::RemoteIo
                | ComponentKind::FieldSensor
                | ComponentKind::FieldActuator
                | ComponentKind::SafetyInterface
        ),
        PortHardwareKind::TelephoneRj11 => {
            matches!(appliance, ComponentKind::VoiceGateway)
        }
    }
}

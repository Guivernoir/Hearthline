use hearthline_model::ServiceKind;

use crate::ConfigError;

pub(crate) fn parse_service_kind(value: &str) -> Result<ServiceKind, ConfigError> {
    match value {
        "dns" | "dns-tcp" | "dns-udp" => Ok(ServiceKind::Dns),
        "dhcp" => Ok(ServiceKind::Dhcp),
        "http" => Ok(ServiceKind::Http),
        "https" => Ok(ServiceKind::Https),
        "ssh" | "sftp" => Ok(ServiceKind::Ssh),
        "rdp" => Ok(ServiceKind::Rdp),
        "snmp" => Ok(ServiceKind::Snmp),
        "syslog" => Ok(ServiceKind::Syslog),
        "ntp" => Ok(ServiceKind::Ntp),
        "pki" => Ok(ServiceKind::Pki),
        "identity" | "radius" => Ok(ServiceKind::Identity),
        "policy-decision" => Ok(ServiceKind::PolicyDecision),
        "file-transfer" => Ok(ServiceKind::FileTransfer),
        "historian" | "historian-replication" => Ok(ServiceKind::HistorianReplication),
        "monitoring" => Ok(ServiceKind::Monitoring),
        "backup" => Ok(ServiceKind::Backup),
        "analytics" => Ok(ServiceKind::Analytics),
        "voice-signaling" => Ok(ServiceKind::VoiceSignaling),
        "printing" => Ok(ServiceKind::Printing),
        "plc-engineering" => Ok(ServiceKind::PlcEngineering),
        "industrial-io" => Ok(ServiceKind::IndustrialIo),
        "management" => Ok(ServiceKind::Management),
        "generic" => Ok(ServiceKind::Generic),
        other => Err(ConfigError::new(format!(
            "unsupported configured service {other}"
        ))),
    }
}

pub(crate) const fn service_name(service: ServiceKind) -> &'static str {
    match service {
        ServiceKind::Dns => "dns",
        ServiceKind::Dhcp => "dhcp",
        ServiceKind::Http => "http",
        ServiceKind::Https => "https",
        ServiceKind::Ssh => "ssh",
        ServiceKind::Rdp => "rdp",
        ServiceKind::Snmp => "snmp",
        ServiceKind::Syslog => "syslog",
        ServiceKind::Ntp => "ntp",
        ServiceKind::Pki => "pki",
        ServiceKind::Identity => "identity",
        ServiceKind::PolicyDecision => "policy-decision",
        ServiceKind::FileTransfer => "file-transfer",
        ServiceKind::HistorianReplication => "historian-replication",
        ServiceKind::Monitoring => "monitoring",
        ServiceKind::Backup => "backup",
        ServiceKind::Analytics => "analytics",
        ServiceKind::VoiceSignaling => "voice-signaling",
        ServiceKind::Printing => "printing",
        ServiceKind::PlcEngineering => "plc-engineering",
        ServiceKind::IndustrialIo => "industrial-io",
        ServiceKind::Management => "management",
        ServiceKind::Generic => "generic",
    }
}

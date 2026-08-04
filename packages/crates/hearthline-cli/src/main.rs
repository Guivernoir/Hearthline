use std::error::Error;
use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use hearthline_config::{
    ConfigRepository, ConfiguredNetwork, ConnectionRepository, ScenarioRepository, run_scenario,
};
use hearthline_engine::{
    ConnectionMedium, CopperCategory, CopperMedium, CopperWiring, Effect, LinkAppliance,
    LinkEndpoint, LinkMode, MediaLink, MediaLinkConfig, PortDuplex, PortHardwareKind, PortSettings,
    PortState, PortStateConfig, RENDERED_ROLE_CONTRACTS, RoutedInterface, ServiceNode,
    SimulatedPort, Simulator, appliance_contracts,
};
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, IcmpMessage, Ipv4InterfaceAddress,
    Ipv4Packet, MacAddress, NetworkPayload, PortId, ServiceKind, TcpFlags, TcpSegment, Transport,
    VlanId,
};

fn main() -> Result<(), Box<dyn Error>> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "help".into());
    match command.as_str() {
        "catalog" => print_catalog(),
        "coverage" => print_coverage(),
        "demo" => run_demo()?,
        "config-demo" => run_config_demo()?,
        "scenario-run" => run_configured_scenario()?,
        "config-validate" => validate_configs()?,
        "config-generate" => generate_frontend_configs()?,
        "version" | "--version" | "-V" => print_version(),
        "help" | "--help" | "-h" => print_help(),
        unknown => {
            eprintln!("unknown command: {unknown}");
            print_help();
            std::process::exit(2);
        }
    }
    Ok(())
}

fn print_version() {
    println!("hearthline {}", env!("CARGO_PKG_VERSION"));
}

fn print_help() {
    println!("Hearthline simulation CLI");
    println!();
    println!("USAGE:");
    println!("  cargo run -p hearthline-cli -- <command>");
    println!();
    println!("COMMANDS:");
    println!("  catalog  List every appliance kind and assigned behavior family");
    println!("  coverage List rendered roles and their Rust appliance kinds");
    println!("  demo     Run a small deterministic forwarding scenario");
    println!("  config-demo      Run the YAML-built Customer LAN scenario");
    println!("  scenario-run     Run a configured scenario by ID");
    println!("  config-validate  Validate appliance, connection, and scenario YAML");
    println!("  config-generate  Validate project YAML and generate Svelte config data");
    println!("  version          Print the Hearthline release version");
}

fn validate_configs() -> Result<(), Box<dyn Error>> {
    let appliances = ConfigRepository::load("project/config/appliances")?;
    let connections = ConnectionRepository::load("project/config/connections", &appliances)?;
    let scenarios =
        ScenarioRepository::load("project/config/scenarios", &appliances, &connections)?;
    println!(
        "validated {} appliance, {} connection, and {} scenario configuration files",
        appliances.len(),
        connections.len(),
        scenarios.len()
    );
    Ok(())
}

fn generate_frontend_configs() -> Result<(), Box<dyn Error>> {
    let appliances = ConfigRepository::load("project/config/appliances")?;
    let connections = ConnectionRepository::load("project/config/connections", &appliances)?;
    let scenarios =
        ScenarioRepository::load("project/config/scenarios", &appliances, &connections)?;
    let json = serde_json::to_string(&appliances.frontend_catalog(&connections))? + "\n";
    let output = Path::new("packages/web/src/generated/appliance-configs.json");
    let temporary = temporary_path(output);
    fs::write(&temporary, json)?;
    fs::rename(&temporary, output)?;
    let scenario_unit = if scenarios.len() == 1 {
        "scenario"
    } else {
        "scenarios"
    };
    println!(
        "generated {} from {} appliances, {} connections, and {} validated {}",
        output.display(),
        appliances.len(),
        connections.len(),
        scenarios.len(),
        scenario_unit
    );
    Ok(())
}

fn temporary_path(output: &Path) -> PathBuf {
    let mut path = output.as_os_str().to_owned();
    path.push(".tmp");
    PathBuf::from(path)
}

fn print_coverage() {
    println!("{:<38} APPLIANCE KIND", "RENDERED ROLE");
    for contract in RENDERED_ROLE_CONTRACTS {
        println!("{:<38} {}", contract.rendered_role, contract.kind);
    }
}

fn print_catalog() {
    println!("{:<30} {:<22} BASELINE", "APPLIANCE", "BEHAVIOR");
    for contract in appliance_contracts() {
        println!(
            "{:<30} {:<22} {}",
            contract.kind, contract.family, contract.baseline
        );
    }
}

fn run_demo() -> Result<(), Box<dyn Error>> {
    let cpe_id = component_id("customer-inet-cpe-01")?;
    let service_id = component_id("public-service-01")?;
    let customer_port = port_id("customer")?;
    let access_port = port_id("access")?;
    let network_port = port_id("network")?;

    let mut cpe = LinkAppliance::new(
        cpe_id.clone(),
        ComponentKind::TransparentCpe,
        [customer_port.clone(), access_port.clone()],
        LinkMode::Transparent,
    );
    let mut service = ServiceNode::new(
        service_id.clone(),
        ComponentKind::ServiceCluster,
        [RoutedInterface::new(
            network_port.clone(),
            MacAddress::new([0x02, 0, 0, 0, 0, 2]),
            [Ipv4InterfaceAddress::new(Ipv4Addr::new(192, 0, 2, 10), 24)
                .ok_or("invalid demo interface address")?],
            VlanId::new(10).ok_or("invalid demo VLAN")?,
            1_500,
        )],
        [ServiceKind::Https],
    );
    let mut connection = MediaLink::new(
        component_id("cpe-to-service")?,
        ethernet_endpoint(cpe_id.clone(), access_port.clone()),
        ethernet_endpoint(service_id.clone(), network_port.clone()),
        MediaLinkConfig::default(),
        ConnectionMedium::Copper {
            config: CopperMedium {
                wiring: CopperWiring::StraightThrough,
                category: CopperCategory::Cat6a,
                length_m: 10.0,
            },
        },
    )?;
    let mut simulator = Simulator::new();
    simulator.add(&mut cpe)?;
    simulator.add(&mut service)?;
    simulator.add_link(&mut connection)?;
    simulator.inject_network(
        &cpe_id,
        &customer_port,
        EthernetFrame {
            source: MacAddress::new([0x02, 0, 0, 0, 0, 1]),
            destination: MacAddress::new([0x02, 0, 0, 0, 0, 2]),
            vlan: VlanId::new(10).ok_or("invalid demo VLAN")?,
            payload: NetworkPayload::Ipv4(Ipv4Packet {
                source: Ipv4Addr::new(203, 0, 113, 2),
                destination: Ipv4Addr::new(192, 0, 2, 10),
                ttl: 64,
                transport: Transport::Tcp(TcpSegment {
                    source_port: 50_000,
                    destination_port: 443,
                    flags: TcpFlags {
                        syn: true,
                        ..TcpFlags::default()
                    },
                }),
                application: ApplicationData::Service(ServiceKind::Https),
            }),
            wire_len_bytes: 64,
        },
    )?;

    for entry in simulator.run(16)? {
        match &entry.effect {
            Effect::Transmit { egress, .. } => {
                println!(
                    "{:>5} ms  {:<28} transmit via {}",
                    entry.time_ms, entry.component, egress
                );
            }
            Effect::Deliver { service, detail } => {
                println!(
                    "{:>5} ms  {:<28} deliver {:?}: {}",
                    entry.time_ms, entry.component, service, detail
                );
            }
            other => {
                println!(
                    "{:>5} ms  {:<28} {:?}",
                    entry.time_ms, entry.component, other
                );
            }
        }
    }
    Ok(())
}

fn run_config_demo() -> Result<(), Box<dyn Error>> {
    let appliances = ConfigRepository::load("project/config/appliances")?;
    let connections = ConnectionRepository::load("project/config/connections", &appliances)?;
    let source = component_id("customer-pc-01")?;
    let mut network = ConfiguredNetwork::from_selection(
        &appliances,
        &connections,
        ["customer-pc-01", "customer-sw-01", "customer-rtr-01"],
    )?;
    let trace = network.run_ipv4(
        &source,
        Ipv4Packet {
            source: Ipv4Addr::new(192, 168, 0, 2),
            destination: Ipv4Addr::new(192, 168, 0, 1),
            ttl: 64,
            transport: Transport::Icmp(IcmpMessage::EchoRequest {
                identifier: 1,
                sequence: 1,
            }),
            application: ApplicationData::None,
        },
        64,
    )?;
    println!(
        "configured {} appliances and {} links",
        network.appliance_count(),
        network.link_count()
    );
    for entry in trace {
        println!(
            "{:>8} us  {:<28} {:?}",
            entry.time_us, entry.component, entry.effect
        );
    }
    Ok(())
}

fn run_configured_scenario() -> Result<(), Box<dyn Error>> {
    let id = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "customer-dns-lookup".into());
    let appliances = ConfigRepository::load("project/config/appliances")?;
    let connections = ConnectionRepository::load("project/config/connections", &appliances)?;
    let scenarios =
        ScenarioRepository::load("project/config/scenarios", &appliances, &connections)?;
    let scenario = scenarios
        .get(&id)
        .ok_or_else(|| format!("unknown configured scenario {id}"))?;
    let report = run_scenario(&appliances, &connections, &scenario.config, None)?;
    println!(
        "{}: {:?}; {} appliances, {} links, {} trace entries, {} us",
        report.scenario_label,
        report.status,
        report.appliance_count,
        report.link_count,
        report.statistics.events,
        report.duration_us
    );
    for entry in report.trace {
        println!(
            "{:>8} us  {:<28} {:<12?} {}",
            entry.time_us, entry.component, entry.kind, entry.summary
        );
    }
    Ok(())
}

fn component_id(value: &str) -> Result<ComponentId, Box<dyn Error>> {
    Ok(ComponentId::new(value)?)
}

fn port_id(value: &str) -> Result<PortId, Box<dyn Error>> {
    Ok(PortId::new(value)?)
}

fn ethernet_endpoint(component: ComponentId, port: PortId) -> LinkEndpoint {
    LinkEndpoint {
        component,
        port,
        profile: SimulatedPort {
            hardware: PortHardwareKind::EthernetRj45,
            state: PortStateConfig {
                administrative: PortState::Up,
                initial_operational: PortState::Up,
            },
            settings: PortSettings {
                speed_mbps: 1_000,
                duplex: PortDuplex::Full,
                mtu: 1_500,
            },
        },
    }
}

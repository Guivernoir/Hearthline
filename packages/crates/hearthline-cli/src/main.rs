use std::error::Error;
use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use hearthline_config::{ConfigRepository, ConnectionRepository};
use hearthline_engine::{
    Effect, LinkAppliance, LinkMode, RENDERED_ROLE_CONTRACTS, ServiceNode, Simulator,
    appliance_contracts,
};
use hearthline_model::{
    ApplicationData, ComponentId, ComponentKind, EthernetFrame, Ipv4Packet, MacAddress,
    NetworkPayload, PortId, ServiceKind, TcpFlags, TcpSegment, Transport, VlanId,
};

fn main() -> Result<(), Box<dyn Error>> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "help".into());
    match command.as_str() {
        "catalog" => print_catalog(),
        "coverage" => print_coverage(),
        "demo" => run_demo()?,
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
    println!("  config-validate  Validate all appliance and connection YAML files");
    println!("  config-generate  Validate project YAML and generate Svelte config data");
    println!("  version          Print the Hearthline release version");
}

fn validate_configs() -> Result<(), Box<dyn Error>> {
    let appliances = ConfigRepository::load("project/config/appliances")?;
    let connections = ConnectionRepository::load("project/config/connections", &appliances)?;
    println!(
        "validated {} appliance and {} connection configuration files",
        appliances.len(),
        connections.len()
    );
    Ok(())
}

fn generate_frontend_configs() -> Result<(), Box<dyn Error>> {
    let appliances = ConfigRepository::load("project/config/appliances")?;
    let connections = ConnectionRepository::load("project/config/connections", &appliances)?;
    let json = serde_json::to_string(&appliances.frontend_catalog(&connections))? + "\n";
    let output = Path::new("packages/web/src/generated/appliance-configs.json");
    let temporary = temporary_path(output);
    fs::write(&temporary, json)?;
    fs::rename(&temporary, output)?;
    println!(
        "generated {} from {} appliances and {} connections",
        output.display(),
        appliances.len(),
        connections.len()
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
        [network_port.clone()],
        [Ipv4Addr::new(192, 0, 2, 10)],
        [ServiceKind::Https],
    );
    let mut simulator = Simulator::new();
    simulator.add(&mut cpe)?;
    simulator.add(&mut service)?;
    simulator.connect(&cpe_id, &access_port, &service_id, &network_port)?;
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

fn component_id(value: &str) -> Result<ComponentId, Box<dyn Error>> {
    Ok(ComponentId::new(value)?)
}

fn port_id(value: &str) -> Result<PortId, Box<dyn Error>> {
    Ok(PortId::new(value)?)
}

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use hearthline_project::{CapacityStatus, ProjectCompiler};
use hearthline_sim::{
    CONDUIT_OVERLOAD_SCENARIO, QUANTIZATION_CONTRACT_VERSION, REPLAY_SCHEMA_VERSION,
    ReplayArtifact, ReplayCheckpoint, ReplayOutcome, ReplayVerifier, RunInput, RunManifest,
    ScenarioConfig, ScenarioContinuityFault, ScenarioReport, run_conduit_overload_contract,
    run_scenario,
};

const PROJECT_ROOT_ENV: &str = "HEARTHLINE_PROJECT_ROOT";

#[derive(Parser)]
#[command(
    name = "hearthline",
    version,
    about = "Deterministic industrial architecture simulation"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    Capacity {
        #[command(subcommand)]
        command: CapacityCommand,
    },
    Run {
        scenario: String,
        #[arg(long)]
        record: Option<PathBuf>,
    },
    Replay {
        #[command(subcommand)]
        command: ReplayCommand,
    },
}

#[derive(Subcommand)]
enum ModelCommand {
    Validate,
    Compile {
        #[arg(long)]
        locked: bool,
    },
    Lock {
        #[arg(long, required = true)]
        update: bool,
        #[arg(long, requires = "update")]
        reason: String,
    },
    Expand {
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
enum CapacityCommand {
    Report {
        #[arg(long, value_enum, default_value_t = ReportFormat::Text)]
        format: ReportFormat,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ReportFormat {
    Text,
    Json,
}

#[derive(Subcommand)]
enum ReplayCommand {
    Verify { artifact: PathBuf },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Model { command } => model(command)?,
        Command::Capacity { command } => capacity(command)?,
        Command::Run { scenario, record } => run(&scenario, record.as_deref())?,
        Command::Replay { command } => replay(command)?,
    }
    Ok(())
}

fn model(command: ModelCommand) -> Result<(), Box<dyn Error>> {
    let config_root = project_config_root()?;
    let compiler = ProjectCompiler::new(&config_root);
    match command {
        ModelCommand::Validate => {
            let project = compiler.compile("validation")?;
            println!(
                "validated {} appliances, {} connections, {} scenarios, {} cells, and {} blueprint instances at {}",
                project.appliances().len(),
                project.connections().len(),
                project.scenarios().len(),
                project.runtime_plan().partitions.len() + project.expanded_blueprints().len(),
                project.expanded_blueprints().len(),
                project.digest()
            );
        }
        ModelCommand::Compile { locked } => {
            let project = if locked {
                compiler.compile_locked()?
            } else {
                compiler.compile("unlocked compilation")?
            };
            write_generated_catalogs(&project)?;
            println!(
                "compiled immutable model {} with {} normalized objects",
                project.digest(),
                project.model_lock().object_digests.len()
            );
        }
        ModelCommand::Lock {
            update: true,
            reason,
        } => {
            let project = compiler.compile(reason)?;
            write_atomic(
                &config_root.join("model.lock.json"),
                &project.model_lock().to_json()?,
            )?;
            write_generated_catalogs(&project)?;
            println!("updated model lock at revision {}", project.digest());
        }
        ModelCommand::Lock { update: false, .. } => unreachable!("--update is required by clap"),
        ModelCommand::Expand { output } => {
            let project = compiler.compile("blueprint expansion")?;
            fs::create_dir_all(&output)?;
            for blueprint in project.expanded_blueprints() {
                let path = output.join(format!("{}.json", blueprint.instance));
                write_atomic(&path, &(serde_json::to_string_pretty(blueprint)? + "\n"))?;
            }
            println!(
                "expanded {} blueprint instances into {}",
                project.expanded_blueprints().len(),
                output.display()
            );
        }
    }
    Ok(())
}

fn capacity(command: CapacityCommand) -> Result<(), Box<dyn Error>> {
    let CapacityCommand::Report { format } = command;
    let project = ProjectCompiler::new(project_config_root()?).compile("capacity report")?;
    match format {
        ReportFormat::Json => println!("{}", serde_json::to_string_pretty(project.capacity())?),
        ReportFormat::Text => {
            println!(
                "RESOURCE                         SCOPE                         DEMAND REVIEWED RESERVE STATUS"
            );
            for item in &project.capacity().assessments {
                println!(
                    "{:<32} {:<29} {:>6} {:>8} {:>6}% {:?}",
                    format!("{:?}", item.resource),
                    item.scope,
                    item.demand,
                    item.reviewed_limit,
                    item.reserve_percent,
                    item.status
                );
            }
            if project
                .capacity()
                .assessments
                .iter()
                .any(|item| item.status != CapacityStatus::Accepted)
            {
                return Err("capacity plan requires review".into());
            }
        }
    }
    Ok(())
}

fn run(scenario_id: &str, record: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let project = ProjectCompiler::new(project_config_root()?).compile_locked()?;
    if scenario_id == CONDUIT_OVERLOAD_SCENARIO {
        let artifact = run_conduit_overload_contract(project.digest(), env!("CARGO_PKG_VERSION"))?;
        if let Some(path) = record {
            artifact.write(path)?;
        }
        println!(
            "{}: {}; {} events, digest {}",
            scenario_id,
            artifact.outcome.status,
            artifact.outcome.event_count,
            artifact.outcome.final_digest
        );
        return Ok(());
    }
    let scenario = project
        .scenarios()
        .get(scenario_id)
        .ok_or_else(|| format!("unknown scenario {scenario_id}"))?;
    let report = run_scenario(
        project.appliances(),
        project.connections(),
        &scenario.config,
        None,
    )?;
    let artifact = artifact(&project, &scenario.config, &report)?;
    if let Some(path) = record {
        artifact.write(path)?;
    }
    println!(
        "{}: {:?}; {} events, {} us, digest {}",
        report.scenario_label,
        report.status,
        report.statistics.events,
        report.duration_us,
        artifact.outcome.final_digest
    );
    Ok(())
}

fn replay(command: ReplayCommand) -> Result<(), Box<dyn Error>> {
    let ReplayCommand::Verify { artifact: path } = command;
    let expected = ReplayArtifact::load(&path)?;
    let project = ProjectCompiler::new(project_config_root()?).compile_locked()?;
    if expected.manifest.model_digest != project.digest() {
        return Err(format!(
            "replay model {} differs from current {}",
            expected.manifest.model_digest,
            project.digest()
        )
        .into());
    }
    if expected.manifest.scenario == CONDUIT_OVERLOAD_SCENARIO {
        let actual = run_conduit_overload_contract(project.digest(), env!("CARGO_PKG_VERSION"))?;
        let outcome = ReplayVerifier::verify(&expected, &actual)?;
        println!(
            "verified replay {} with {} events and digest {}",
            path.display(),
            outcome.event_count,
            outcome.final_digest
        );
        return Ok(());
    }
    let scenario = project
        .scenarios()
        .get(&expected.manifest.scenario)
        .ok_or_else(|| format!("unknown replay scenario {}", expected.manifest.scenario))?;
    let report = run_scenario(
        project.appliances(),
        project.connections(),
        &scenario.config,
        None,
    )?;
    let actual = artifact(&project, &scenario.config, &report)?;
    let outcome = ReplayVerifier::verify(&expected, &actual)?;
    println!(
        "verified replay {} with {} events and digest {}",
        path.display(),
        outcome.event_count,
        outcome.final_digest
    );
    Ok(())
}

fn artifact(
    project: &hearthline_project::CompiledProject,
    scenario: &ScenarioConfig,
    report: &ScenarioReport,
) -> Result<ReplayArtifact, Box<dyn Error>> {
    let initial_snapshot = report
        .runtime
        .initial
        .project_snapshot(project.digest(), &scenario.id)?;
    let final_snapshot = report
        .runtime
        .final_state
        .project_snapshot(project.digest(), &scenario.id)?;
    let final_digest = final_snapshot.digest();
    Ok(ReplayArtifact {
        schema_version: REPLAY_SCHEMA_VERSION.into(),
        manifest: RunManifest {
            schema_version: REPLAY_SCHEMA_VERSION.into(),
            model_digest: project.digest().into(),
            simulation_version: env!("CARGO_PKG_VERSION").into(),
            scenario: scenario.id.clone(),
            quantization_contract: QUANTIZATION_CONTRACT_VERSION.into(),
            clock_policy: "fixed-step".into(),
            clock_step_us: 1,
            seed: 0,
            initial_state_digest: initial_snapshot.digest(),
            event_limit: scenario.event_limit,
            limits: BTreeMap::from([
                ("event-limit".into(), scenario.event_limit as u64),
                (
                    "wire-length-bytes".into(),
                    scenario.packet.wire_length_bytes as u64,
                ),
            ]),
            expected_outcomes: vec![
                format!("status:{:?}", report.status).to_lowercase(),
                format!(
                    "expectation:{}:{:?}",
                    report.expectation.component, report.expectation.outcome
                )
                .to_lowercase(),
                format!("mode:{:?}", report.expectation_mode).to_lowercase(),
            ],
            inputs: scenario_inputs(scenario, report),
        },
        checkpoints: vec![
            ReplayCheckpoint::capture(0, &initial_snapshot),
            ReplayCheckpoint::capture(report.statistics.events, &final_snapshot),
        ],
        outcome: ReplayOutcome {
            status: format!("{:?}", report.status).to_lowercase(),
            final_digest,
            event_count: report.statistics.events,
            alarms: Vec::new(),
            metrics: BTreeMap::from([
                ("deliveries".into(), report.statistics.deliveries as u64),
                ("drops".into(), report.statistics.drops as u64),
                (
                    "transmissions".into(),
                    report.statistics.transmissions as u64,
                ),
            ]),
        },
    }
    .normalize()?)
}

fn scenario_inputs(scenario: &ScenarioConfig, report: &ScenarioReport) -> Vec<RunInput> {
    let mut inputs = vec![packet_input(0, &scenario.source, &scenario.packet)];
    inputs.extend(
        scenario
            .connection_overrides
            .iter()
            .map(|state| RunInput::Fault {
                at_us: 0,
                target: state.connection.clone(),
                fault: "connection-unavailable".into(),
                active: !state.operational,
            }),
    );
    inputs.extend(
        scenario
            .first_hop_overrides
            .iter()
            .map(|state| RunInput::Command {
                at_us: 0,
                source: "scenario".into(),
                target: state.appliance.clone(),
                command: format!("set-first-hop-role:{}:{}", state.interface, state.role),
                values: BTreeMap::new(),
            }),
    );
    inputs.extend(
        scenario
            .firewall_ha_overrides
            .iter()
            .map(|state| RunInput::Command {
                at_us: 0,
                source: "scenario".into(),
                target: state.appliance.clone(),
                command: format!("set-firewall-ha-role:{}", state.role),
                values: BTreeMap::new(),
            }),
    );
    if let Some(continuity) = &scenario.continuity {
        for fault in &continuity.faults {
            let (at_us, target, fault) = match fault {
                ScenarioContinuityFault::SyncLinkLoss { at_us } => (
                    *at_us,
                    continuity.failed_appliance.clone(),
                    "firewall-ha-sync-loss",
                ),
                ScenarioContinuityFault::StandbySessionLoss { at_us } => (
                    *at_us,
                    report.continuity.as_ref().map_or_else(
                        || "standby-firewall".into(),
                        |item| item.promoted_appliance.clone(),
                    ),
                    "standby-session-state-loss",
                ),
            };
            inputs.push(RunInput::Fault {
                at_us,
                target,
                fault: fault.into(),
                active: true,
            });
        }
        inputs.push(RunInput::Fault {
            at_us: continuity.failure_at_us,
            target: continuity.failed_appliance.clone(),
            fault: "appliance-unavailable".into(),
            active: true,
        });
        inputs.extend(
            continuity
                .connection_overrides
                .iter()
                .map(|state| RunInput::Fault {
                    at_us: continuity.failure_at_us,
                    target: state.connection.clone(),
                    fault: "connection-unavailable".into(),
                    active: !state.operational,
                }),
        );
        inputs.push(packet_input(
            continuity.continuation_at_us,
            &continuity.source,
            &continuity.packet,
        ));
    }
    if let Some(isolation) = &scenario.ha_isolation {
        inputs.push(RunInput::Fault {
            at_us: isolation.isolation_at_us,
            target: isolation.standby_appliance.clone(),
            fault: "ha-peer-isolation".into(),
            active: true,
        });
        inputs.extend(
            isolation
                .connection_overrides
                .iter()
                .map(|state| RunInput::Fault {
                    at_us: isolation.isolation_at_us,
                    target: state.connection.clone(),
                    fault: "connection-unavailable".into(),
                    active: !state.operational,
                }),
        );
        inputs.push(packet_input(
            isolation.continuation_at_us,
            &isolation.source,
            &isolation.packet,
        ));
    }
    if let Some(autonomy) = &scenario.local_autonomy {
        inputs.push(RunInput::Command {
            at_us: 1,
            source: autonomy.hmi.clone(),
            target: autonomy.safety_interface.clone(),
            command: "reset-safety".into(),
            values: BTreeMap::new(),
        });
        inputs.push(RunInput::Command {
            at_us: 2,
            source: autonomy.hmi.clone(),
            target: autonomy.actuator.clone(),
            command: format!("{}={}", autonomy.command_tag, autonomy.command_value),
            values: BTreeMap::new(),
        });
    }
    inputs
}

fn packet_input(
    at_us: u64,
    source: &str,
    packet: &hearthline_sim::ScenarioPacketConfig,
) -> RunInput {
    RunInput::Command {
        at_us,
        source: "scenario".into(),
        target: source.into(),
        command: "inject-ipv4".into(),
        values: BTreeMap::from([(
            "wire-length-bytes".into(),
            i64::from(packet.wire_length_bytes),
        )]),
    }
}

fn write_generated_catalogs(
    project: &hearthline_project::CompiledProject,
) -> Result<(), Box<dyn Error>> {
    let repository_root = repository_root_for_config(project.root())
        .ok_or("compiled configuration root has no repository parent")?;
    for (path, source) in project.generated_catalogs()? {
        write_atomic(&repository_root.join(path), &source)?;
    }
    Ok(())
}

fn repository_root_for_config(config_root: &Path) -> Option<&Path> {
    config_root.parent().and_then(Path::parent)
}

fn write_atomic(path: &Path, source: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("hearthline-tmp");
    fs::write(&temporary, source)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn project_config_root() -> Result<PathBuf, Box<dyn Error>> {
    if let Some(root) = std::env::var_os(PROJECT_ROOT_ENV) {
        let root = PathBuf::from(root);
        let config = root.join("project/config");
        if config.is_dir() {
            return Ok(config);
        }
        return Err(format!(
            "{PROJECT_ROOT_ENV}={} does not contain project/config",
            root.display()
        )
        .into());
    }
    let current = std::env::current_dir()?;
    for candidate in current.ancestors() {
        let config = candidate.join("project/config");
        if config.is_dir() {
            return Ok(config);
        }
    }
    Err(format!(
        "cannot find project/config from {}; set {PROJECT_ROOT_ENV}",
        current.display()
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::{ReplayCommand, replay, repository_root_for_config};
    use std::path::Path;

    #[test]
    fn generated_catalogs_are_anchored_to_the_repository() {
        assert_eq!(
            repository_root_for_config(Path::new("/workspace/project/config")),
            Some(Path::new("/workspace"))
        );
    }

    #[test]
    fn golden_replays_fit_one_mib_worker_stack() {
        std::thread::Builder::new()
            .name("golden-replay-stack-probe".into())
            .stack_size(1024 * 1024)
            .spawn(|| {
                let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../project/replays");
                for artifact in [
                    "safety-firewall-isolation.json",
                    "network-customer-dns.json",
                    "forming-historian.json",
                    "body-preparation-local-autonomy.json",
                    "conduit-overload.json",
                    "recovery-firewall-session.json",
                ] {
                    replay(ReplayCommand::Verify {
                        artifact: root.join(artifact),
                    })
                    .unwrap_or_else(|error| panic!("{artifact}: {error}"));
                }
            })
            .expect("constrained replay worker starts")
            .join()
            .expect("all golden replays complete on a constrained stack");
    }
}

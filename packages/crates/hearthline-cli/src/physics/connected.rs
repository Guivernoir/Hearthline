use std::error::Error;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Subcommand;
use hearthline_config::physics::connected::{CONNECTED_GAS_SOURCE_LIMIT, ConnectedGasDocument};
use hearthline_project::physics::DEFAULT_PHYSICS_BUDGET_BYTES;
use hearthline_project::physics::flow::connected::CompiledConnectedGas;
use hearthline_sim::physics::clock::PlaybackRate;
use hearthline_sim::physics::flow::connected::ConnectedGasSession;
use hearthline_sim::physics::worker::{PhysicsWorker, WorkerMemoryPlan, WorkerPolicy};
use serde_json::json;

use super::worker;

#[derive(Subcommand)]
pub enum GasCommand {
    /// Validate connections and report compiler/runtime/worker memory and work bounds.
    Budget { config: PathBuf },
    /// Execute the connected island on its dedicated worker, optionally continuing a snapshot.
    Run {
        config: PathBuf,
        #[arg(long, value_parser = worker::parse_rate, conflicts_with = "headless")]
        rate: Option<PlaybackRate>,
        #[arg(long, conflicts_with = "rate")]
        headless: bool,
        #[arg(long)]
        resume: Option<PathBuf>,
        /// Stream the accepted final/faulted state to a new file; never overwrite an existing file.
        #[arg(long)]
        snapshot: Option<PathBuf>,
    },
}

pub(super) fn execute(command: GasCommand) -> Result<(), Box<dyn Error>> {
    let config = match &command {
        GasCommand::Budget { config } | GasCommand::Run { config, .. } => config,
    };
    let mut source = String::new();
    File::open(config)?
        .take(CONNECTED_GAS_SOURCE_LIMIT as u64 + 1)
        .read_to_string(&mut source)?;
    let document = ConnectedGasDocument::from_yaml(&source)?;
    drop(source);
    let model = CompiledConnectedGas::compile_document(&document, DEFAULT_PHYSICS_BUDGET_BYTES)?;
    let id = document.id.clone();
    drop(document);
    let policy = WorkerPolicy::reviewed_default();
    match command {
        GasCommand::Budget { .. } => {
            let worker = WorkerMemoryPlan::compile_connected(&model, policy)?;
            let memory = model.memory();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "id": id,
                    "model_digest": model.digest(),
                    "cells": model.mesh().memory().cells,
                    "regions": model.mesh().regions().len(),
                    "connections": model.mesh().connections().len(),
                    "maximum_evaluations_per_horizon": model.evaluation_bound(),
                    "compiler_peak_bytes": memory.compiler_peak_bytes,
                    "numerical_runtime_bytes": memory.runtime_bytes,
                    "worker_planned_bytes": worker.planned_bytes,
                    "allocation_budget_bytes": memory.allocation_budget_bytes,
                    "qualification": "synthetic numerical fixture; not full-factory memory or physical validation"
                }))?
            );
        }
        GasCommand::Run {
            rate,
            resume,
            snapshot,
            ..
        } => {
            if snapshot
                .as_ref()
                .is_some_and(|path| path.try_exists().unwrap_or(true))
            {
                return Err("snapshot destination already exists or cannot be checked".into());
            }
            let model = Arc::new(model);
            let session = match resume {
                Some(path) => {
                    ConnectedGasSession::read_snapshot(model, BufReader::new(File::open(path)?))?
                }
                None => ConnectedGasSession::new(model)?,
            };
            let runtime = PhysicsWorker::spawn_connected_session(session, policy)?;
            let finished = worker::run_to_stop(&runtime.client(), rate)?;
            let session = runtime.shutdown()?;
            if let Some(path) = snapshot {
                save_snapshot(&session, &path)?;
            }
            let digest = session.state_digest()?;
            if let Some(fault) = finished.fault {
                return Err(format!(
                    "connected gas fault after {} committed steps, retained state {}: {fault:?}",
                    finished.committed_events,
                    std::str::from_utf8(&digest)?
                )
                .into());
            }
            let (requested, achieved) = worker::rate_summary(rate, finished);
            println!(
                "{id}: {} physical steps, {} us, digest {}; requested {requested}, achieved {achieved}; connected numerical fixture only, not plant qualification",
                finished.committed_events,
                finished.time_us,
                std::str::from_utf8(&digest)?
            );
        }
    }
    Ok(())
}

fn save_snapshot(session: &ConnectedGasSession, path: &Path) -> Result<(), Box<dyn Error>> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".hearthline-snapshot-")
        .tempfile_in(parent)?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        session.write_snapshot(&mut writer)?;
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(path)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

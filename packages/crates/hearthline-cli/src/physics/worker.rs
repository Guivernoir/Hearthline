use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use hearthline_project::physics::CompiledPhysicsProof;
use hearthline_sim::physics::clock::PlaybackRate;
use hearthline_sim::physics::worker::{
    ClockCommand, PhysicsWorker, PhysicsWorkerClient, WorkerPolicy,
};

pub(super) fn parse_rate(text: &str) -> Result<PlaybackRate, String> {
    match text {
        "1" => Ok(PlaybackRate::One),
        "2" => Ok(PlaybackRate::Two),
        "5" => Ok(PlaybackRate::Five),
        "10" => Ok(PlaybackRate::Ten),
        _ => Err("rate must be 1, 2, 5, or 10".into()),
    }
}

pub(super) fn execute(
    model: CompiledPhysicsProof,
    rate: Option<PlaybackRate>,
) -> Result<(), Box<dyn Error>> {
    let id = model.config().id.clone();
    let worker = PhysicsWorker::spawn(Arc::new(model), WorkerPolicy::reviewed_default())?;
    let client = worker.client();
    command(
        &client,
        0,
        rate.map_or(ClockCommand::Headless, ClockCommand::SetRate),
    )?;
    command(&client, 1, ClockCommand::Resume)?;
    let finished = loop {
        let status = client.projection()?;
        if let Some(fault) = status.fault {
            return Err(format!(
                "physics worker fault after {} committed steps: {fault:?}",
                status.step
            )
            .into());
        }
        if status.complete {
            break status;
        }
        if status.terminated {
            return Err("physics worker terminated before its declared horizon".into());
        }
        client.wait_for_change(status.generation, Duration::from_millis(100))?;
    };
    let session = worker.shutdown()?;
    let digest = session.state_digest()?;
    let requested = rate.map_or_else(
        || "headless".to_string(),
        |rate| format!("{}x", rate.multiplier()),
    );
    let achieved = finished.achieved_milli_rate().map_or_else(
        || "unmeasured".to_string(),
        |rate| format!("{}.{:03}x", rate / 1000, rate % 1000),
    );
    println!(
        "{id}: {} physical steps, {} us, digest {}; requested {requested}, achieved {achieved}; numerical worker only, not plant qualification",
        finished.step,
        finished.time_us,
        std::str::from_utf8(&digest)?
    );
    Ok(())
}

fn command(
    client: &PhysicsWorkerClient,
    revision: u64,
    command: ClockCommand,
) -> Result<(), Box<dyn Error>> {
    let ticket = client.try_submit(revision, command)?;
    loop {
        let status = client.projection()?;
        if let Some(reply) = client.try_take_reply(ticket)? {
            return reply
                .result
                .map_err(|error| format!("clock command rejected: {error:?}").into());
        }
        if status.terminated {
            return Err("physics worker terminated before acknowledging a command".into());
        }
        client.wait_for_change(status.generation, Duration::from_millis(100))?;
    }
}

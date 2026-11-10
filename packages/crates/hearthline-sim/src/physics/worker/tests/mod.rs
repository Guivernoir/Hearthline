use std::sync::Arc;
use std::time::{Duration, Instant};

use hearthline_config::physics::PhysicsProofConfig;
use hearthline_project::physics::CompiledPhysicsProof;

use super::*;
use crate::physics::{PhysicsProofSession, PhysicsReplay};

mod state;
mod threaded;

fn model() -> Arc<CompiledPhysicsProof> {
    let replay: PhysicsReplay = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/replays/wet-elastic-zero.json"
    )))
    .unwrap();
    Arc::new(
        CompiledPhysicsProof::compile_with_resources(
            replay.config,
            replay.geometry,
            replay.material,
        )
        .unwrap(),
    )
}

fn receiver(steps: u32) -> Arc<CompiledPhysicsProof> {
    let mut config: PhysicsProofConfig = serde_yaml_ng::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../project/physics/proofs/compressible-receiver.yaml"
    )))
    .unwrap();
    config.steps = steps;
    Arc::new(CompiledPhysicsProof::compile(config).unwrap())
}

fn request(revision: u64, command: ClockCommand) -> mailbox::Request {
    mailbox::Request {
        ticket: CommandTicket(revision),
        expected_revision: revision,
        command,
    }
}

fn reply(client: &PhysicsWorkerClient, ticket: CommandTicket) -> CommandReply {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = client.projection().unwrap();
        if let Some(reply) = client.try_take_reply(ticket).unwrap() {
            return reply;
        }
        assert!(
            !status.terminated && Instant::now() < deadline,
            "worker did not acknowledge: {status:?}"
        );
        client
            .wait_for_change(status.generation, Duration::from_millis(20))
            .unwrap();
    }
}

fn command(client: &PhysicsWorkerClient, revision: u64, command: ClockCommand) -> CommandReply {
    let ticket = client.try_submit(revision, command).unwrap();
    reply(client, ticket)
}

fn completed(client: &PhysicsWorkerClient) -> WorkerProjection {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = client.projection().unwrap();
        assert!(status.fault.is_none(), "{status:?}");
        if status.complete {
            return status;
        }
        assert!(
            !status.terminated && Instant::now() < deadline,
            "worker did not finish: {status:?}"
        );
        client
            .wait_for_change(status.generation, Duration::from_millis(20))
            .unwrap();
    }
}

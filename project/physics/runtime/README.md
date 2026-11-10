# Dedicated Physics Worker

Status: numerical qualification runtime. It hosts one compiled proof session;
the coupled factory, canonical API sessions and HMIs are not migrated yet.

## Ownership And Clock

`PhysicsWorker` transfers exclusive ownership of a `PhysicsProofSession` to a
named OS worker thread. A client cannot mutate or borrow its fields. The pinned
compiled model is immutable and retained by `Arc`; changing source files cannot
change an existing worker. State returns to the owner only after shutdown joins
the thread at a solver boundary.

Workers start paused. Supported commands are software Pause, Resume, SingleStep,
SetRate (1x, 2x, 5x, 10x), and Headless. SingleStep requires paused state; changing
mode or rate does not resume execution. A revision belongs to the control
configuration, not each physical step. Concurrent requests against the same
revision cannot both apply. Successful commands increment it; rejected commands
do not. Acknowledgment of SingleStep accepts the request, while the resulting
projection reports whether its numerical step succeeded.

One clock records accepted simulation time. Pacing cannot change the numerical
timestep, mesh, nonlinear limits or equations. Slow execution retains pacing
debt; it does not skip steps. Entering headless mode discards wall-pacing debt,
not physical state. Resuming from a validated checkpoint preserves simulation
time while starting a new paused pacing interval. Active wall time and automatic
simulated time provide measured achieved speed; manually requested steps are
excluded from that ratio. Wall measurements are not part of normalized replay.

Software pause is not a process Hold, Stop or emergency action. Those process
commands still require the future equipment/controller integration. Numerical
failure preserves the last committed fields and clock and latches a diagnostic.
It cannot be cleared by a clock Resume command. Restart requires an explicitly
constructed or restored session.

A client handle is bound to one worker instance. A restart creates a new handle
and fresh command revision; old handles remain terminated. The future HTTP
adapter must carry instance identity as well as control revision. A model digest
alone cannot distinguish two sessions of the same model.

## Bounded Communication

The preallocated mailbox has a configurable capacity and reviewed burst. The
reviewed burst must leave at least 25% reserve. The default policy has 16 slots
and a reviewed burst of 12; it is not a project-wide structural maximum.

Each slot transitions `Free -> Pending -> Executing -> Replied -> Free`.
Monotone tickets determine request order. The reply retains the same slot until
the client consumes it. Full capacity rejects the new submission explicitly;
neither requests nor replies are silently dropped or allocated elsewhere.
Outstanding count and high-water mark are visible in projections.

Unread replies can prevent further submissions but cannot suspend physics.
The worker takes a brief mailbox lock to claim a request or copy a projection,
never while solving or scanning spatial fields. Latest-value projections
coalesce updates; they are not an event log or historian. Shutdown has an
out-of-band flag and a mutex-coordinated condition-variable wake, so saturation
cannot prevent joining and no worker is detached by owner destruction.

Projections contain simulation time, mode, control revision, numerical failure,
and raw first-field/temperature extrema. These are engineering fields, not
simulated instrument readings. There is no claim that an operator can observe
the solver's hidden fields.

## Resource Evidence

`WorkerMemoryPlan` preflights numerical payloads, immutable numerical data,
mailbox allocation and the requested worker stack against the proof budget
with 25% reserve. The policy currently requires at least the tested 1 MiB stack.
Mailbox allocation bytes are checked against the allocation counter. The entire
sealed thread loop, including condition-variable waits, mailbox service,
projection publication and headless steps, is allocation-tested.

This plan does not measure total RSS: OS/thread metadata, model-document strings,
allocator overhead, HTTP and rendering still need application-level evidence.
It also does not replace the factory population resource studies. The
[multirate population schedule](schedule.md) now has a separate compiled event
order and full-state restart contract. It and physical connections must join
this runtime before it can host the canonical coupled factory.

## Execute And Verify

From the repository root:

```sh
cargo run --locked --manifest-path packages/Cargo.toml --bin hearthline -- physics worker project/physics/proofs/compressible-receiver.yaml --headless
cargo run --locked --manifest-path packages/Cargo.toml --bin hearthline -- physics worker project/physics/proofs/compressible-receiver.yaml --rate 5
cargo run --locked --manifest-path packages/Cargo.toml --bin hearthline -- physics worker project/physics/proofs/wet-elastic-zero.yaml --material project/physics/materials/synthetic-wet-drying.yaml --headless
cargo test --locked --manifest-path packages/Cargo.toml -p hearthline-sim --lib physics::worker
cargo test --locked --manifest-path packages/Cargo.toml -p hearthline-cli --test physics worker_cli
```

The CLI reports a full-state digest and requested/achieved speed. It hashes the
same fields as a checkpoint without allocating a spatial copy. It does not
write a replay or modify the source model. Rate defaults to headless; `--rate`
and `--headless` are mutually exclusive.

Tests exercise all pacing modes, pause/single-step, full checkpoint continuation,
stale and concurrent revisions, solver rejection, a full unread mailbox,
shutdown/wake races, poisoned-mailbox failure, CLI numerical-failure shutdown,
allocation counts and measured reserves. Native CI runs
worker tests and digest checks on all four supported architectures. Local Linux
results do not establish that native matrix's success. Fuzzing covers clock
transitions and atomic arithmetic failures; nightly mutation targets the clock,
runner and mailbox.

Still pending: integrating the multirate population schedule into this worker,
physical transfer boundaries,
simulation-time equipment/fault commands, controller and instrument integration,
generated API/TypeScript controls, streamed archival, historian, and spatial UI.
This worker is progress toward the integrated release, not completion of those
requirements or a physical-validation claim.

# Multirate Population Schedule

Status: event-order and restart foundation for the coupled runtime. This composes
the existing numerical populations; it does not yet connect physical ports,
equipment, controls or the canonical factory. The dedicated worker can now own
this aggregate through its existing revision-checked mailbox and one event clock.

## Compiled Model

`CompiledPhysicsSchedule` takes already compiled numerical models and qualified
`(site, cell, instance)` identities. It sorts identities and deduplicates profiles
by content digest. Distinct instances retain distinct mutable field banks;
identical profiles share immutable data and one serial solver workspace.
Profile/slot assignment and the schedule digest do not depend on source order.
Duplicate identities, empty input, zero budget and arithmetic overflow reject
compilation. This is a Rust compiler interface, not an additional source-file
format or a replacement for authoritative appliance/connection YAML.

Each profile retains its own numerical timestep and declared proof horizon.
The schedule advances the earliest pending `(time, site, cell, instance)` event.
It does not tick every object at the fastest timestep, interpolate committed
state, consult viewport visibility, skip work, or launch per-instance threads.
The earliest event is selected by a preallocated binary heap. There is exactly
one pending event per unfinished instance; advancing removes it before inserting
its successor. The heap reserves at least 25% unused capacity at construction.
The immutable registration set cannot grow after construction.

## Commit And Failure

One successful event commits one instance and advances the shared clock to that
event time. Equal-time events commit in identity order. A snapshot may therefore
contain a partially completed timestamp; it is not falsely represented as an
atomic factory-wide step. Earlier successful events remain committed when a
later event fails. The failed instance, pending event and last committed clock
remain unchanged, and the schedule latches the numerical diagnostic.

This contract is sufficient for independent proof populations. Coupled physical
connections still require conservative boundary exchange and synchronization
rules. It must not be used to infer that independently advanced fields already
exchange mass, energy or force. Equipment, instrument and network events also
need explicit ordering against this clock before factory integration.
The [connected gas kernel and compiler](connections.md) now provide a jointly
solved conforming mesh, but are not yet scheduled or archived by this aggregate.

## Snapshots And Replay Evidence

The versioned NDJSON snapshot contains a schedule header, complete existing-format
spatial checkpoints in instance order, and an aggregate digest. Valid capture
streams without allocating copies of the complete factory. Loading constructs
an unpublished runtime, decodes one bounded checkpoint at a time, and validates
all fields using the existing numerical readers.

Individually valid checkpoints are not enough: their step counts must form one
chronological event prefix. The last committed event must precede every pending
event, including identity ordering at equal times. Counts and the shared clock
must match that prefix. This reconstructs the heap without trusting a serialized
heap layout. A saved numerical failure, including its typed cause, raw residual
and retry diagnostics, must recur exactly at the declared pending event;
an arbitrary halted marker is rejected. No wall time enters the snapshot.

The [golden digests](schedule-golden.json) cover two independent copies of the
isothermal receiver proof (0.5-second steps, six steps each) and one drying
specimen (one-second steps, three steps). They pin the compiled model, initial
state, a four-event prefix inside an equal-time boundary, and final state.
Parameters otherwise come from the existing proof YAML. These are numerical
qualification cases, not production equipment configurations or calibrated
material predictions. The nine preceding single-profile replay artifacts remain
unchanged.

The [failure fixture](schedule-failure-golden.json) uses two six-step compressible
receiver instances. One accepts its first 100-microsecond event; the other has
one solver iteration, one allowed substep and a raw residual tolerance of one.
Its rejected event pins the prior committed fields, exact failure cause,
iteration/residual details and retry limits. These limits intentionally force a
numerical failure and are not proposed operating settings.
Flow retry durations are explicitly labeled `smallest_solver_step_us`: they are
millionths of the profile's nondimensional reference time (`length / velocity`),
not the physical microseconds used by the schedule clock. Residuals are raw Q9
solver values, not engineering sensor units.

Tests independently construct the expected event order, compare complete fields
with standalone executions, restart at every prefix, reject rehashed impossible
frontiers, and verify repeatable failure. A separate 1 MiB-stack case includes
compressible flow and two wet elastic specimens. Construction allocation bytes
are measured against the plan; stepping, streamed capture and digesting must
allocate zero bytes after construction. Native CI checks the golden digests on
all four supported targets; local Linux success is not evidence of the other
three targets' results.

```sh
cargo test --locked --manifest-path packages/Cargo.toml -p hearthline-sim --lib physics::schedule
```

## Resource Boundary And Remaining Work

The aggregate memory plan includes every population's state bank, metadata and
solver workspace, immutable numerical payloads, schedule metadata, population
objects and heap reserve. Snapshot loading adds the largest single-frame decoder
envelope. Runtime and restore each enforce the shared budget's 25% reserve;
individual profiles must also respect their own budgets. An in-budget runtime
can still reject snapshot capture if restoring it would exceed the budget.
Restore accounting covers the new runtime, not another live copy retained by a
caller. A session host must reserve both resident copies or retire the old one
before restoration; this API does not grant an additional per-session budget.

This is not total application RSS. Thread stacks/mailboxes, host document strings,
compiler peak memory, HTTP, renderer and allocator/OS overhead require integration
with the existing worker and factory resource studies. Remaining work includes
physical connections,
bounded transfer queues, equipment/fault/controller tasks, genealogy, incremental
projections and canonical model compilation. Production geometry, constitutive
validation and full-factory memory qualification remain open gates.

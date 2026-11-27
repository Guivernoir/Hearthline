# Multirate Population Schedule

Status: event-order and restart foundation for the coupled runtime. This composes
proof/drying populations and connected gas islands; it does not yet connect
physical ports between independently scheduled domains,
equipment, controls or the canonical factory. The dedicated worker can now own
this aggregate through its existing revision-checked mailbox and one event clock.

## Compiled Model

`CompiledPhysicsSchedule` takes already compiled numerical models and qualified
`(site, cell, instance)` identities. It sorts identities and deduplicates profiles
by numerical family and content digest. Distinct instances retain distinct mutable field banks;
identical profiles share immutable data and one serial solver workspace.
Profile/slot assignment and the schedule digest do not depend on source order.
Duplicate identities, empty input, zero budget and arithmetic overflow reject
compilation. Identical gas profiles must declare the same reviewed memory budget;
deduplication cannot silently discard a stricter policy. The aggregate digest
pins that budget separately from the standalone gas physics digest.
This is a Rust compiler interface, not an additional source-file
format or a replacement for authoritative appliance/connection YAML.

`CompiledNumericalProfile` distinguishes proof/specimen models from connected
gas models. Proof-only model digests retain their original contract; a schedule
containing gas islands uses the typed version-2 model contract. Each registered
island is an independent instance scope. Region identities inside its shared
immutable mesh identify locations within that scope, not aliases for regions
in another instance. Future physical-port bindings must identify both the outer
instance and its inner region; repeated profiles do not create implicit links.

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
later event fails. Its accepted fields, pending event and last committed clock
remain unchanged. The schedule latches the numerical diagnostic, and a gas
instance also retains its own failure/retry metadata.

This contract is sufficient for independent proof populations. Coupled physical
connections still require conservative boundary exchange and synchronization
rules. It must not be used to infer that independently advanced fields already
exchange mass, energy or force. Equipment, instrument and network events also
need explicit ordering against this clock before factory integration.
The [connected gas kernel and compiler](../connections.md) jointly solve every
region within one gas island. Those islands now participate in this aggregate;
separate islands and specimens do not exchange material or heat merely because
they share a worker or timestamp.

## Snapshots And Replay Evidence

Snapshot `0.2.0` contains a schedule header, typed length-delimited child frames
in instance order, and an aggregate digest. Proof frames contain a complete
checkpoint; gas frames contain their independently validated full cell archive,
including retry and fault state. Frame order, kind, length and consumption are
checked against the pinned model. A counting pass precedes each streamed child,
without retaining its serialized fields. Valid capture
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
Gas child failures must agree with the parent's declared event: hidden child
faults, missing child faults, or faults on another instance reject the archive.

The previous `0.1.0` proof-only format remains readable. Writers emit only `0.2.0`.
[Archived snapshots](v1/README.md) and the original golden digests are preserved;
tests independently verify their original hashes, migrate all fields/failures,
and compare with fresh current executions. The new frame/header format changes
aggregate state digests, not the previous cases' numerical fields or timing.

The [current golden digests](current.json) cover two independent copies of the
isothermal receiver proof (0.5-second steps, six steps each) and one drying
specimen (one-second steps, three steps). They pin the compiled model, initial
state, a four-event prefix inside an equal-time boundary, and final state.
Parameters otherwise come from the existing proof YAML. These are numerical
qualification cases, not production equipment configurations or calibrated
material predictions. The nine preceding single-profile replay artifacts remain
unchanged.

The [current failure fixture](failure-current.json) uses two six-step compressible
receiver instances. One accepts its first 100-microsecond event; the other has
one solver iteration, one allowed substep and a raw residual tolerance of one.
Its rejected event pins the prior committed fields, exact failure cause,
iteration/residual details and retry limits. These limits intentionally force a
numerical failure and are not proposed operating settings.
Flow retry durations are explicitly labeled `smallest_solver_step_us`: they are
millionths of the profile's nondimensional reference time (`length / velocity`),
not the physical microseconds used by the schedule clock. Residuals are raw Q9
solver values, not engineering sensor units.

The [mixed fixture](mixed.json) registers two independent copies of the synthetic
connected-gas retry source (six 1,001-microsecond steps each) and the existing
drying specimen (three one-second steps). It has fifteen events and different
declared end times for the two profile families. Gas instances stop only at
their configured proof horizon, not when the view changes or a slower event
runs. This finite verification case is not a continuously operating factory.
Every event is compared with independently advanced instances; every prefix is
captured/restored, including equal-time gas ties. A one-iteration gas variant
verifies fault atomicity, child/parent consistency and worker restart. The
connected retry duration is raw Q9 nondimensional time (`smallest_step_raw`),
distinct from both physical microseconds and the older proof-flow diagnostic.
The dedicated-worker tests run all pacing modes to the same final digest and
retain per-profile work counters without post-construction allocation.

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
solver workspace, immutable numerical payloads, schedule metadata,
boxed population objects, typed dispatch slots and heap reserve. Snapshot loading
keeps the largest proof row plus the largest child decoder/temporary-session
envelope, including a bounded hashing-reader buffer. Gas work plans also check
physical-step and worst-case evaluation counters before construction.
Runtime and restore each enforce the shared budget's 25% reserve;
individual profiles must also respect their own budgets. An in-budget runtime
can still reject snapshot capture if restoring it would exceed the budget.
Restore accounting covers the new runtime, not another live copy retained by a
caller. A session host must reserve both resident copies or retire the old one
before restoration; this API does not grant an additional per-session budget.

This is not total application RSS. Thread stacks/mailboxes, host document strings,
compiler peak memory, HTTP, renderer and allocator/OS overhead require integration
with the existing worker and factory resource studies. Remaining work includes
cross-domain physical connections,
bounded transfer queues, equipment/fault/controller tasks, genealogy, incremental
projections and canonical model compilation. Production geometry, constitutive
validation and full-factory memory qualification remain open gates.

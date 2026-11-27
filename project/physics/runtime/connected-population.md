# Connected Gas Population Storage

Status: experimental storage foundation for repeated gas domains in 0.4.0.
This extends the [connected island runtime](connections.md) and now participates
in the [mixed-profile scheduler](schedule/README.md). Canonical factory integration
and physical exchange between separately scheduled domains remain pending.

## Ownership

`ConnectedGasPopulation` holds one immutable model through `Arc`, one serial
`ConnectedGasSession` working copy, a preallocated accepted-field bank, and
per-instance metadata. Every instance has distinct cell inventories, time step,
cumulative boundary exchange, solver/retry reports, failure latch and field
extrema. Loading another instance replaces all this working metadata, including
failure state; scratch buffers are disposable and initialized by each solve.

An instance's complete connected mesh is solved jointly. Population instances
are separate domains: sharing a workspace is not a physical connection and
does not exchange material or heat between them. The scheduler assigns qualified
outer instance identities; cross-domain exchanges still require explicit
compilation and synchronization.
Slots cannot be added after construction. There is no fixed global population
limit; counts must fit checked addressing and the reviewed allocation budget.
`ConnectedPopulationWork` also preflights total physical steps, maximum solver
evaluations over the declared horizon, and worst-case fault-revalidation solves
at restore. All products use checked counters. These are numerical evaluation
bounds, not wall-clock predictions; each evaluation still visits its compiled
domain's cells/faces. Capture and parsing costs are separate from solver work.

Steps commit accepted fields and their metadata together under exclusive mutable
ownership. A numerical failure retains the accepted fields/time and records its
cause only in that instance. Visiting another slot cannot clear or inherit that
failure. Completion and invalid slot indices produce explicit errors.
The API exposes only borrowed, read-only accepted fields to host code.

## Memory And Capture

The population reuses one four-array working copy. Additional instances require
one accepted field array and a metadata record each, not four solver arrays.
Immutable geometry and coefficients remain shared; mutable accepted state never
does. `ConnectedPopulationMemory` charges the working session, bank, metadata,
population object and a transactional-restore peak under the model's original
budget, with 25% reserve. Restore accounts for one temporary session, its
bounded decoder envelope and the outer archive row. The child envelope charges
one row buffer, up to two rows for JSON string scratch growth, four rows for
concurrent error formatting and one row for fixed decoder/error overhead.
Fault causes decode directly into fixed typed fields rather than buffering an
arbitrary tagged map. Malformed-input allocation regressions guard this bound.
This is numerical storage accounting, not measured total
application RSS or full-factory qualification.

Host-side `restore_instance` validates a complete temporary session before
publishing its fields and metadata to a slot. An invalid stream cannot change
that slot or clear its failure. Restore allocates outside the sealed execution
path. Stepping, instance hashing and streaming capture allocate no storage when
the caller provides a nonallocating/preallocated writer.

Population snapshot schema `0.1.0` contains a bounded header, ordered frame
descriptors, length-delimited standalone connected snapshots and a footer. The
header pins the normalized child schema (`0.2.0`) as well as the model and
population contract. Every child retains its own cell-order, clock, inventory,
digest and fault-recurrence validation. A frame cannot consume a neighboring
frame or leave unconsumed declared bytes. Header/frame/footer rows are bounded
to 4 KiB; a child frame's upper bound derives from the compiled cell count.

The population digest hashes its contract, model digest, instance count and
ordered `(slot, normalized child-state digest)` pairs. It deliberately hashes
normalized state, not transport framing or whitespace. Supported previous child
snapshots migrate through the existing reader before normalization. A future
child-schema change must review the pinned population contract and migration;
it must not silently reinterpret prior root digests. As with the individual
snapshots, these hashes detect inconsistent/corrupt input, not maliciously
reauthored history. Native recomputation provides the golden evidence.

Capture performs a counting pass and a streaming pass for each child instead of
retaining its serialized spatial fields. An output failure leaves accepted
state unchanged. Full restore constructs an unpublished population; malformed
framing, bad counts, invalid children, a bad root digest or trailing input discard
that construction. Clocks are independently valid here; chronological frontier
validation belongs to the aggregate scheduler and is not claimed by this API.

## Verification

[connected-population.json](connected-population.json) pins three instances of
the existing synthetic retry fixture. The slot order is
`2, 0, 2, 1, 1, 0, 2, 0, 1`, with an intermediate checkpoint after four events.
Every interleaved step is compared with three separately executed sessions,
including complete fields and per-instance state digests. A separate one-iteration
case faults slot zero while the other slots remain unattempted and unfaulted.

Tests cover continuation after every event, fault isolation, invalid-instance
handling, transactional slot restore, malformed framing/counts/digests, output
failure, measured construction/restore allocations and a 1 MiB thread stack.
Capture and stepping are allocation-counted. Native CI recomputes the fixtures
on all four configured platforms; local results alone do not establish that
matrix. Snapshot fuzzing adds raw and mutated population archives, interleaved
execution and deliberately nonconvergent domains.

```sh
cargo test --locked --manifest-path packages/Cargo.toml -p hearthline-sim --test physics_connected_population
```

The fixture is a numerical/ownership check, not a calibrated air network.
Physical boundaries between scheduled domains, control
programs, instruments and full application memory evidence remain required.

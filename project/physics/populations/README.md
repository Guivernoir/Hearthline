# Pooled Numerical Populations

`PhysicsProofPopulation` runs independent instances of one immutable numerical
profile through a shared serial solver workspace. It supports the current
proof cases, including the sparse basin domain and moisture-dependent material
law. This is storage/runtime infrastructure, not the coupled factory scheduler,
equipment control system or a calibrated product model.

## Ownership And Execution

Each instance owns a separate range in a preallocated state bank. That range
contains the independent variables: pressure for the isothermal receiver,
moisture and temperature for drying, conserved density/momentum/energy for
compressible flow, and displacement for default previous-step elasticity.
Opt-in [zero-start elasticity](reconstruction.md) reconstructs displacement
with a complete deterministic equilibrium solve instead of retaining its history.
Compressible-flow pressure
and temperature and history-free elastic stress are reconstructed exactly in
the shared worker. They remain present in every checkpoint and its digest.
A separate fixed-size record holds its step, conservation residuals and accepted
solver metadata. The workspace
retains trial fields, coefficients and solver scratch once per identical
compiled profile. It also retains a working state copy; this extra copy is
included in the memory plan.

Before a step, the selected instance is copied into the workspace. Only a
successful step commits back to its bank range. Failed numerical steps leave
all resident instances unchanged. Switching instances cannot inherit another
instance's pressure, stress, time or accumulated residuals. No state-bank growth
is available after construction, and valid steps allocate no heap memory.

The constitutive laws and property tables are fixed by the compiled model.
Material-aware diffusion reconstructs local coefficients from the selected
instance's moisture field. Its accepted nonlinear residual and iteration/sweep
counts belong to that instance, not the shared worker. Optional enthalpy solves
also retain their sweep/root counts, true residual and latent-exchange diagnostic
per instance. Their scratch storage reuses the completed moisture solve's
buffers; it is not independent history. Stress is
reconstructible only for this history-free elastic law; this does not permit
discarding plastic strain, damage or any other independent history variable.
Any future evolving coefficient, material history, controller timer or RNG must
be added to instance state before using this execution path. Such additions require
updates to the layout contract, transfer codec, snapshot format and tests.
The proof bank does not yet include factory genealogy or controller state.

`PopulationMemoryPlan` checks state, per-instance metadata, the shared worker
arrays, immutable geometry and material tables before allocation. Construction is tested against
actual allocated byte counts, and the 25% reserve remains mandatory. These
measurements exclude full application RSS, browser memory and unrelated host
objects.

## Streamed Snapshots

`write_snapshot` emits newline-delimited UTF-8 records with LF endings:

1. A strict `0.1.0` header containing numerical contract, model digest, instance
   count and serialized values per instance, including reconstructed fields.
2. One existing-format `PhysicsCheckpoint` per instance, in stable index order.
3. A footer containing SHA-256 of every preceding byte, including LF delimiters.

The individual checkpoint digests remain identical to independent proof
sessions. The aggregate digest also protects instance ordering and cardinality.
Capture serializes borrowed fields directly and allocates no temporary field
arrays or complete trace. It requires mutable, exclusive scheduling ownership
to load each instance and reconstruct its projections; physics steps are not
run concurrently with a partially captured revision.

`read_snapshot` constructs a new, unpublished population against a supplied
immutable model. It reads one bounded row at a time, validates its digest,
shape, time, physical domain and derived mechanical fields, then validates the
aggregate footer before returning the population. It never partially replaces
a live bank. Load is a construction operation and may allocate within its
preflight envelope; it is not a post-seal command. The caller must account for
any old population it chooses to retain concurrently.

Headers/footers are limited to 4 KiB and individual checkpoint rows to the
existing 16 MiB standalone adapter ceiling. A checked per-profile row bound and
conservative decoder envelope are included in `SnapshotLoadMemoryPlan` before
state allocation. A model that cannot be captured and restored within its
reviewed budget is rejected before output begins. This ceiling is an explicit
adapter limitation, not permission to discard fields or coarsen a model.

The archive is streamed to a caller-owned writer. File flush, `fsync` and atomic
publication remain the adapter's responsibility; a write error can leave
partial output that must not be published as a complete artifact. The older
allocating `checkpoint` projection remains available for compatibility outside
the sealed worker path. No population archive schema precedes `0.1.0`.

## Evidence And Remaining Work

Tests compare interleaved pooled execution with independent sessions at every
checked boundary, compare complete checkpoint bytes with existing goldens,
continue the four earlier committed proof replays, a small basin fixture and
the material-aware proof on a 1 MiB stack, measure construction
allocations, enforce allocation-free stepping/capture,
and check recovery after failed steps. Archive tests cover corrupted/reordered
records, truncation, model mismatch, oversized frames, budget rejection and
short reads/writes. The snapshot fuzz target exercises raw archives and mutations
of generated scalar, elastic-drying, variable-drying and compressible-flow archives with a small
host budget, including reconstruction of derived fields.

`physics capacity <study.yaml> --pooled` compares these measured layouts with
the unchanged independent-workspace study. It groups instances by profile,
charges one workspace per profile, retains every physical state slot, and
charges the largest serial decoder envelope. It does not charge whole-factory
snapshot copies because the implemented archive does not retain them. Auxiliary
factory storage remains a declared envelope, not a runtime measurement.

The [factory comparison](../capacity/README.md) fits the reviewed budget for the
implemented fields, not for the complete release scope. The factory worker,
model/lot integration, production basin mesh qualification, evolving constitutive
fields, archival adapter and full application memory qualification remain open.

The basin compiler separately preflights temporary sparse-matrix assembly.
Its runtime matrix is shared immutable data only because the current elastic
coefficients are constant. Separate [source-backed studies](../capacity/source-backed.md)
now charge actual basin layouts and material tables while retaining the original
block baseline. Coarse-bank construction is allocation-checked at all 2,778
reserved product slots. The refined layout is over the reviewed budget and is
rejected before bank allocation. A separate opt-in zero-start refined study
fits the implemented-field budget and checks construction of all 2,778 slots
without changing mesh or workload. Neither strategy qualifies production
capacity or permits dropping future irreversible state. The normalized
preconditioner reaches the refined first-step mechanical tolerance; neither
memory fit nor one converged step establishes complete numerical qualification.

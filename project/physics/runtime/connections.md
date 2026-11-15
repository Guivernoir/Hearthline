# Conservative Gas Connections

Status: experimental numerical/runtime foundation for 0.4.0. Named gas regions
share finite-volume faces and advance in one implicit solve, including dedicated
worker execution and streamed full-state restart. This is not yet integrated
into the multirate aggregate, canonical appliance/connection YAML, equipment
controls or fault schedules. Existing
canonical configurations and the nine earlier physics replay artifacts are
unchanged.

## Compiled Topology

`CompiledFlowMesh` accepts qualified `(site, cell, instance)` region identities,
Cartesian region grids, declared exterior boundaries, and named connections
between opposite exterior patches. All regions must first be expressed in one
common nondimensional reference frame. Sorting identities and connection IDs,
and normalizing endpoint order, produces stable cell numbering and a topology
digest independent of input traversal order.

The current interface is an entire rectangular patch. Tangential cell counts
and spacing must match; normal cell widths may differ. Each face has exactly one
reciprocal peer or one exterior boundary. Unknown regions, reused patches,
duplicate IDs, invalid directions and nonconforming patches reject compilation.
Different-axis attachments, partial openings and nonconforming meshes need
explicit geometry/conservative interface support; they are not approximated by
copying a pressure value. A pipe must own its own volume, not just an edge label.

`CompiledConnectedGas` adds the immutable physical model around this topology.
It accepts SI region spacing, absolute pressure, density, velocities, wall
temperatures, constant transport coefficients and reference scales. Normalization
precedes mesh construction, so a small SI volume need not underflow before it is
scaled. Positive coefficients erased by scaling, unrepresentable clock horizons,
unresolved timesteps, invalid states and moving normal walls reject compilation.
Each region requires exactly one qualified initial state.

The enclosing digest includes topology, normalized initial states, reference
scales, gas/transport properties, timestep, iteration/residual limits and versioned
numerical contracts. An optional `GasRetryConfig` adds a maximum power-of-two
substep count and a minimum physical substep duration; its normalized policy
and advance contract also enter the digest. Models compiled without this policy
retain their preceding identity and single-solve behavior. Budget and platform-dependent object sizes do not enter
the physical digest. A standalone typed YAML adapter is available for numerical
verification. It is not yet a canonical appliance schema, editable project asset
or equipment/control contract.

## YAML And CLI

[connected-source.yaml](connected-source.yaml) reproduces the existing retry
golden from SI source quantities. Its `0.1.0` schema is generated from Rust in
[connected-gas.schema.json](../../contracts/schemas/connected-gas.schema.json).
This is the first source-schema version; the snapshot versions described below
are independent. Values are synthetic, including the unit gas constant and
pressures, and must not be treated as industrial air operating specifications.

Each region declares its qualified identity, Cartesian spacing/counts, initial
absolute pressure/density/velocity and all six named exterior faces. A `coupled`
face must appear in exactly one connection. A wall or external reservoir cannot
be silently replaced by a link; unknown, duplicate, missing or geometrically
incompatible connections fail compilation. All spacings, states, wall values
and coefficients are SI. Residual tolerance is normalized; retry minimum time
is physical microseconds. No gauge-pressure conversion is implicit.

The source reader limits raw input to 1 MiB, nesting to 32 levels, and expanded
YAML to a 1 MiB accounting budget (32 bytes per visited node plus string bytes),
before allocating typed objects. Text and decimal fields require YAML strings
(quote decimal values); numeric-to-string coercion is rejected so the expansion
budget cannot be bypassed with long numeric aliases. Aliases are allowed only within this budget;
duplicate/unknown fields, unsupported schemas and custom tags are rejected. The source
DTO allocations and face-validation table enter the compiler memory preflight.
This does not measure the YAML parser's peak RSS or the full application.
Region/list order, endpoint orientation and equivalent exact decimal spelling
produce the same physical model digest. Source labels and memory budgets do not
alter physical state identity.

From the repository root:

```sh
cargo run --locked --manifest-path packages/Cargo.toml --bin hearthline -- physics gas budget project/physics/runtime/connected-source.yaml
cargo run --locked --manifest-path packages/Cargo.toml --bin hearthline -- physics gas run project/physics/runtime/connected-source.yaml --rate 5 --snapshot /tmp/connected-final.ndjson
cargo run --locked --manifest-path packages/Cargo.toml --bin hearthline -- physics gas run project/physics/runtime/connected-source.yaml --resume /tmp/connected-final.ndjson --headless
```

`run` uses the dedicated worker, defaults to headless and reports requested and
achieved speed. `--resume` requires the same physical model digest. `--snapshot`
streams accepted final or faulted state through a synced temporary file and
publishes a new destination without replacing an existing file. The destination
directory must exist. A numerical fault returns a nonzero exit status even when
its snapshot is saved; restoring that snapshot does not clear the fault. These
snapshots are continuation checkpoints, not a replacement for command/fault
recording and independently verified `ReplayArtifact` execution.

## Conservation And Integration

The no-allocator kernel stores each cell's extensive mass, momentum vector and
total energy. Density-form conserved fields are reconstructed using the compiled
cell volume for the ideal-gas equation of state and existing Rusanov flux.
Constant-property Newtonian stress and Fourier conduction are supported across
the same connections, including unequal normal spacing. Slip, no-slip, thermal
walls and explicitly declared exterior reservoirs retain their separate meaning.
An exterior reservoir is a verification boundary, not an implicit factory supply.

For an internal face, one rounded transfer is evaluated from its face flux,
area and timestep. The exact same integer vector is subtracted from one cell
and added to its peer. Unequal volumes therefore do not independently round two
different mass or energy transfers. Wall force and exterior heat/material
exchange enter the signed boundary ledger. Every accepted step verifies that
the aggregate inventory change equals this ledger exactly in the quantized
representation; no correction or saturation hides a mismatch.

Backward Euler uses bounded damped fixed-point iteration. Only a conservative
candidate is published, after checking the equation residual at that candidate;
a relaxed iterate is never published. Invalid state, overflow, insufficient
resolution or exhausted iteration limits reject the entire step. Caller-owned
scratch arrays may change, but no authoritative cell changes on rejection.
With an explicit retry policy, nonconvergence or an invalid provisional gas
state retries the **whole horizon from its accepted origin**, doubling the
substep count within the declared maximum and minimum duration. Successful
substeps of a rejected attempt are discarded. Overflow, invalid input and
insufficient numerical resolution are not retried. Exhaustion latches the cause,
attempt count, substep count, completed substeps and smallest normalized duration.

Internal substeps split the positive normalized Q9 time integer by quotient and
remainder, assigning the remainder to leading substeps. Their sum is exactly the
requested normalized horizon; the physical clock advances once by the original
microsecond duration. No elapsed time is discarded, and no conservation
correction or relaxed tolerance substitutes for convergence. The compiler bounds worst-case evaluations over every admissible
attempt as `2 * iteration_limit * sum(substep_counts)`, rejecting overflow. The
success report distinguishes accepted-attempt iterations/evaluations from this
all-attempt work bound. This is convergence recovery, not an error estimator or
a substitute for independent time-refinement and physical-validation studies.

Extensive Q9 quantization means a smaller cell has a larger density increment
per inventory quantum. Reference scales, mesh and residual tolerance must be
reviewed together. The analytical wave test explicitly declares four inventory
quanta of density-form equation tolerance at each refinement. The solver never
increases that tolerance, shrinks fidelity, clips pressure, or accepts a failed
solve automatically.

Connected regions cannot be advanced as independent proof events with stale
boundary values: the coupled solve must own one accepted time boundary. Future
multirate coupling across separately advanced islands requires conservative
transfer accounting, explicit synchronization and rollback. The current
independent-population scheduler has not been relabeled as this coupled runtime.

## Session And Snapshots

`ConnectedGasSession` owns four preallocated extensive field arrays: accepted
state, a complete-step candidate and two iteration buffers. The candidate is
published only after the solver, cumulative inventory ledger and SI pressure/
temperature projection all validate. Rejection leaves accepted fields, time and
projections intact and latches the numerical cause. Iteration scratch is reset
at every solve and is not authoritative continuation state.

The same sealed `PhysicsWorker` adapter supports this session without adding a
thread per region or a second clock. Pause, single-step, 1x/2x/5x/10x and headless
execution preserve the physical timestep and full-state digest. Engineering
projections expose accepted pressure/temperature extrema and a compact retry/work
summary, not instrument readings. Accepted-attempt counts and the all-attempt
evaluation bound are labeled separately; rejected work is not reported as zero.
Shutdown returns exclusive session ownership. A restored failure remains latched;
Resume cannot clear it or restart equipment.

Snapshot schema `0.2.0` streams an NDJSON header, one fixed-size inventory record
per cell and a SHA-256 footer. The header pins model/numerical contracts, physical
time, step, cumulative exchange, last implicit-substep report, accepted retry
summary and any failure (including exhausted-retry detail). Rows are
bounded to 4 KiB before parsing. Loading checks cell order, complete input,
digest, initial-state equality at step zero, positive recoverable physical state,
clock/report consistency and aggregate inventory against the cumulative ledger.
A recorded numerical failure and its complete retry diagnostic must recur before
the restored session is returned. Version `0.1.0` remains readable for its original
non-retry model contract; writers only emit `0.2.0`. The
[previous snapshots](connected-v1/README.md) and their original digests are retained
as migration evidence, not regenerated under a new meaning.
The hash detects corruption; it is not authentication or proof that an arbitrary
rehashed intermediate state is reachable. Native re-execution provides that
evidence for the golden cases.

## Verification And Resources

Tests cover:

- Exact mass/energy conservation through unequal-volume connected cells.
- Exterior reservoir and wall-heat accounting, isolation and stationary fields.
- Linear shear gradients and viscous work on all three axes with unequal spacing.
- Analytical periodic contact-wave spatial refinement and implicit time refinement.
- Agreement with the preceding rectangular solver to declared numerical tolerance.
- Nonconvergence, invalid-state, overflow and malformed-topology rejection.
- Allocation-free execution on a 1 MiB worker stack.
- Named 3D region compilation, source/endpoint order invariance and field continuation.
- Measured compilation/field peak allocation within the declared memory plan.
- Full-state continuation from every step, including the initial/final boundaries.
- Rehashed invalid archive rejection and retained failure diagnostics after restart.
- Native recomputation at every pacing setting, with worker shutdown/restart.
- Whole-horizon recovery, explicit-substep equivalence, minimum-step exhaustion,
  fatal-error rejection, retry metadata validation and previous-schema migration.

The named 24-cell case joins supply, pipe and receiver regions, each with a 2x2x2
grid. Their normal cell widths are 0.37, 0.71 and 1.13 reference lengths, with unit
tangential widths. Initial normalized pressure/density are 2 in the supply and 1
elsewhere, velocity zero, gamma 1.4, viscosity 0.01, conductivity 0.02 and R=1.
All exterior walls are adiabatic slip walls. Twenty steps of 0.001 reference
time use at most 400 iterations and a density-form residual tolerance of 5e-8.
These are synthetic verification inputs, not proposed operating conditions.

The test pins the SHA-256 of topology digest plus the accepted raw field values:
`1c85d0ca8ceef5b02d4b4b9ae8c0ca0dccef2c486e92e55f97ccd934e4bbb3dd`.
It also continues from an eight-step in-memory field checkpoint. The enclosing
SI model uses unit reference scales and 1,000 microsecond physical steps.
[Current connected runtime goldens](connected-current.json) separately pin its model,
initial/eighth/final state and one deliberately nonconvergent state. Their final
full-state digest is
`6395eaf75393684d1898bf3973eed13021d98053a535697b055ef12a05eecea5`.
The [preceding schema's goldens](connected-golden.json) remain unchanged. The
new full-state digest reflects the versioned header; migration tests compare the
complete physical fields against native recomputation, not just updated hashes.
These cases re-solve natively, verify serialized continuation from all 21 step
boundaries and compare all pacing modes with a worker restart. They are fixture
contracts, not yet public `RunManifest`/`ReplayArtifact`. The source CLI above
executes the retry fixture through the same runtime. CI runs
the tests on all four supported architectures; local execution only establishes
the local result.

The [retry goldens](connected-retry.json) use the same geometry/initial gas with
three 1,001-microsecond horizons, 14 iterations, residual tolerance 1e-7, at most
64 substeps and a one-microsecond minimum. Each horizon converges on attempt four
with eight substeps. Separate one-iteration limits exhaust all seven attempts
without committing fields or time. Restart, all pacing modes and snapshots verify
these diagnostics and full-state results. A kernel case additionally checks an
odd Q9 time remainder against explicit integration of all 64 accepted substeps.

The mesh-only preflight counts cells, boundary/region/connection metadata, three
extensive field arrays and live compiler inputs. The enclosing gas model extends
this to four arrays, normalized initial states and SI compiler inputs; session
metadata and the bounded restore row are charged separately. Worker preflight
adds the mailbox and requested stack under the same budget. Each phase requires
25% unused reserve before mesh/field allocation. Measured tests check allocation
peaks; capture and accepted/rejected stepping allocate nothing. Restore preflight
assumes the prior session is retired, not retained alongside its replacement.
These are resource plans, not total application RSS. HTTP/rendering, allocator/OS
overhead and full-factory integration remain part of the release qualification gate.

```sh
cargo test --locked --manifest-path packages/Cargo.toml -p hearthline-physics --lib --test connected_flow
cargo test --locked --manifest-path packages/Cargo.toml -p hearthline-project --test physics_connections
cargo test --locked --manifest-path packages/Cargo.toml -p hearthline-sim --test physics_connected_runtime
```

## References And Limits

The integral finite-volume balance follows the public
[NIST FiPy discretization documentation](https://pages.nist.gov/fipy/en/master/numerical/discret.html).
The existing Rusanov flux is reused, not replaced with fixed inter-equipment
transfer rates. The [Clawpack finite-volume reference](https://www.clawpack.org/fvmhp_materials/sample.pdf)
provides conservation-law and finite-volume background. These references explain
the numerical method; they do not validate this implementation or its parameters.

The model remains first-order spatial gas transport with a single ideal-gas
mixture and constant transport coefficients. Turbulence, moisture/species,
condensate, equipment maps, valves/boosters, pipe-wall mechanics and experimental
calibration are still required. This work does not qualify factory geometry,
complete the memory gate, or establish real-plant prediction accuracy.

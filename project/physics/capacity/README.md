# Factory Physics Resource Study

This is a pre-migration resource study, not an executable factory configuration.
It deliberately exposes the cost of scaling the currently implemented proof
runtime without assuming future memory optimizations already work.

```sh
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics capacity project/physics/capacity/factory-baseline.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics capacity project/physics/capacity/factory-baseline.yaml --require-fit
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics capacity project/physics/capacity/factory-baseline.yaml --pooled
```

The first command emits the report even when limits fail. The second emits the
same report and exits unsuccessfully on any reviewed memory, population or
auxiliary-capacity violation. It currently **must fail** for this fixture.
Neither command allocates the reported spatial arrays. Neither authorizes
canonical migration, even when a smaller study fits its budget.

## Current Result

The checked-in study covers central, sector-local and boosted air, independent
water/return circuits, Slip and Glaze preparation, transport, four moulds,
multi-day drying, application, both kilns, inspections and bounded release to
logistics. Every count and assumption is visible in the YAML and report.

| Resource | Planned bytes |
| --- | ---: |
| Shared immutable geometry | 1,544,040 |
| Independent instance fields, rollback, coefficients and solver workspaces | 4,309,216,176 |
| Two raw field snapshot copies | 2,877,994,080 |
| Declared auxiliary storage envelopes | 184,549,376 |
| Total | **7,373,303,672** |
| Budget, including reserve | 1,610,612,736 |
| Usable budget after 25% reserve | **1,207,959,552** |

The total is approximately **6.87 GiB**, already above the laptop's full memory
capacity before browser/OS memory and several unimplemented fields. This is a
failure of the independent-workspace strategy under the declared study assumptions,
not proof that every possible implementation of the release is infeasible.
The profile resolutions are unvalidated verification-block envelopes, not
accepted mesh-convergence results or measured basin geometry.

### Implemented Pooled Comparison

The [pooled numerical runtime](../populations/README.md) now implements independent
state banks, shared per-profile workspaces and streamed capture/loading. The
`--pooled` comparison preserves all baseline profile shapes, population
capacities, residence envelopes and auxiliary allowances. The independent
baseline remains available as a regression and comparison fixture.

| Pooled resource | Planned bytes |
| --- | ---: |
| Independent authoritative state banks | 208,280,112 |
| Measured slot metadata, including optional nonlinear and thermal diagnostics | 817,080 |
| Shared profile workspaces, including working state copies | 3,937,848 |
| Shared immutable geometry | 1,544,040 |
| Largest bounded archive decoder envelope | 12,783,386 |
| Unchanged auxiliary envelopes | 184,549,376 |
| Total | **411,911,842** |

This is approximately **393 MiB**, below the **1.125 GiB** usable budget for the
implemented laws. `--pooled --require-fit` passes this accounting check, not
full-factory qualification. Previously, storing all observable fields in every
bank slot required about 1.53 GiB overall. The runtime now reconstructs gas
pressure/temperature from conserved fields, and linear-elastic stress from
displacement and thermal/moisture strain. Every observable field still appears
in snapshots with the same replay digest; no mesh or workload was reduced.
Irreversible constitutive history cannot be discarded this way and must be
budgeted when those models are implemented.
Population storage contract 4 measures 264 bytes of metadata per slot across
all 3,095 reserved slots. Optional thermal solver diagnostics add 32 bytes per
slot over contract 3, raising its estimate by 99,040 bytes. Contract 3 previously
added the same amount for nonlinear moisture diagnostics. Meshes, workloads and
physical fields are unchanged. Material-aware standalone profiles also charge
their extra solver workspace and shared diffusion/caloric tables; the baseline
profiles do not yet enable those laws.

The synthetic production envelope assumes four synchronized sources, a minimum
15-minute cycle and 72 hours in controlled drying, plus a four-piece burst.
That requires 1,156 concurrently resident drying pieces before reserve. The
compiler computes `sources * ceil(residence / minimum_cycle) + burst`, rounding
each source independently. It reserves 1,600 slots, including unused headroom;
it does not allocate for the nominal population alone. Both kiln pools assume
every piece can follow the second-firing route. Holds beyond the declared
residence envelope require upstream backpressure, not removal of existing
products or suspension of off-screen physics.

## Accounting Contract

The [source-backed basin studies](source-backed.md) now compile the original
geometry and wet-material law instead of relying on a filled-block surrogate.
The coarse case fits the implemented-field budget; halving spacing in every
direction produces a **1,556,311,074-byte pooled plan that fails the 25% reserve
requirement**. The earlier block baseline above is retained unchanged. This
new evidence means the full-factory memory gate still cannot be accepted.

- Profiles share immutable geometry only within the same profile. Distinct
  pieces, lots, return circuits and process volumes never share mutable state.
- Kernel-array sizes come from the same Rust layout compiler used to budget
  proof sessions. Runtime allocation tests check those actual array payloads.
- `state_bytes` counts independently stored variables; `checkpoint_field_bytes`
  includes all observable fields. The versioned capacity/storage contracts
  distinguish them without changing the existing checkpoint wire format.
- The baseline fully charges per-instance workspaces. The explicit pooled
  comparison credits the implemented and allocation-tested shared workspace;
  it does not assume arbitrary cross-profile scratch sharing.
- Snapshot field storage is separate from step rollback. JSON serialization,
  object metadata and allocator overhead are not part of raw array sizes.
- Transfer, controller/fault, historian, projection and metadata resources each
  require a typed declaration, positive workload, entry size, reserve, overflow
  policy and documented basis. Their current byte envelopes are assumptions,
  not measured future runtime objects. Material queues cannot drop contents.
- Source collection ordering cannot change the normalized report or digest.
  Invalid IDs, duplicates, unused/unknown profiles, missing evidence and checked
  arithmetic overflow are rejected. No mesh or workload is silently reduced.
- Source-backed profiles pin a typed proof document and record its compiled
  digest. Reports charge immutable material tables and the peak of sequential
  compilation versus runtime storage; a runtime-only fit is insufficient.
- At least 25% reserve applies independently to reviewed pool loads, auxiliary
  loads and aggregate allocation. A fitting memory report still carries explicit
  qualification gaps for physics, integration, measurements and validation.

This is the first capacity-study schema, `0.1.0`. Its generated Rust-derived
JSON Schema lives in `project/contracts/schemas/physics-capacity.schema.json`.
There is no preceding capacity-study version to migrate.

## Required Follow-through

Integrate the tested pooled workspace and streamed capture into the factory
worker and immutable model. The [original hollow basin](../geometry/README.md)
now compiles its solid/material domain rather than a filled verification block.
Its measured geometry and sparse assembly are now included in separate coarse
and refined studies. An opt-in [zero-start elastic study](../populations/reconstruction.md)
now fits the refined implemented-field budget without reducing mesh or workload;
normalized preconditioning also reaches the first-step elastic tolerance where
the legacy solve failed. Perform mesh convergence and physical validation before
choosing a production resolution. Count new
species, particles, gas fractions, damage, coating and combustion fields as
their constitutive models are implemented.

No further geometry or material-field savings are credited. Repeat the resource
study and measure full application resident memory
on the two-core, 6 GB baseline before migrating the canonical factory.

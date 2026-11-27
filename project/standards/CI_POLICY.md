# Continuous Integration Policy

Hearthline treats architecture, determinism, capacity, generated output, and
test evidence as versioned build requirements. Local verification and GitHub
Actions use the same repository-owned commands.

## Required Pull-Request Checks

The following named checks are required before merge. Jobs run independently
to keep the target wall time below 15 minutes.

| Check | Required evidence |
| --- | --- |
| Policy and generated contracts | Version alignment, Cargo-metadata dependency direction, AST runtime policy, generated OpenAPI/JSON Schema/TypeScript drift, current model lock, and clean generated catalogs |
| Rust quality (stable) | Formatting, Clippy with warnings denied, and `no_std` embedded-target compilation on pinned Rust `1.100.0` |
| Rust tests (four shards) | All model, engine, config, project, simulation, operator, CLI, API, and `xtask` tests compiled normally, then their binaries execute directly with a fixed 1.25 MiB worker stack and two test threads; the HMI construction/projection regression separately enforces 512 KiB |
| Capacity and deterministic budgets | Locked compilation, generated capacity report, saturation contracts, 75-cell acceptance model, 24-hour nominal/failure run, allocation counter, queue high-water marks, and structural size budgets |
| Regression corpus and fuzz smoke | Eight parallel checked-in corpora plus short identifier, appliance, connection, blueprint, compiler, scheduler, replay, and control-parser campaigns |
| Svelte and Chromium workflows | Exact npm install, SvelteKit synchronization, audit, type checks, Vitest, static production build, and desktop Chromium Playwright workflows |
| Golden replay matrix | All committed replay artifacts on Linux x64, Linux arm64, Windows x64, and macOS arm64, plus complete CLI replay verification on an explicit 1 MiB worker stack on each platform |

Required check names are repository policy. Renaming, removing, or making one
non-blocking requires a reviewed policy change in the same pull request.

The `0.4.0` numerical proof crate participates in deterministic-core tests,
`no_std` target compilation, architecture checks, and deterministic coverage.
The golden matrix additionally recomputes the standalone physics artifacts
and checks spatial snapshot continuation on a 1 MiB worker stack. This
qualifies only the recorded proof cases, not the future coupled factory.
Physics artifact changes require an explanation of changed equations,
coefficients, numerical resolution or solver behavior and updated analytical
tests; generating new hashes alone is not review evidence.
The basin artifact additionally pins its geometry document and compiled-domain
contract. Its gate checks nonzero mechanical response and the current source
configuration, and rejects allocation plans whose assembly peak cannot retain
25% reserve. Sparse topology, matrix validation and geometry parsing participate
in the numerical fuzz target. Native matrix evidence is still required; local
recomputation alone does not establish cross-platform qualification.
Heterogeneous interfaces and nonlinear constitutive transport also run analytical,
spatial/time-refinement, failure and latent-heat tests in this matrix. The numerical
fuzzer exercises property tables, wide flux arithmetic and transactional nonlinear
steps, and parses/compiles material documents. A separate material-bearing golden
checks source/digest agreement, validity bounds, transactional continuation,
allocation accounting and pooled-instance isolation. The five earlier numerical
artifacts remain unchanged. A separate enthalpy artifact adds dry-solid caloric
integration, bounded row solves and thermal diagnostics; its tests include
energy-based equilibrium, refinement and thermal-boundary resistance. The six
pre-enthalpy artifacts retain their prior digests. Both numerical and snapshot
fuzz targets include the caloric path. These cases have no calibrated
material-data or canonical factory-session claim.

The wet-energy artifact adds liquid sensible storage and conservative upwind
energy transfer from accepted moisture fields. Native tests cover analytic
evaporation/condensation, temporal/spatial refinement, basin coupling, rollback
and independent pooled continuation. Numerical and snapshot fuzz targets include
the liquid-caloric path. All seven preceding artifacts retain their digests;
the eighth is partial numerical evidence, not physical material validation.

The ninth artifact adds opt-in zero-start elastic equilibrium. Native checks
compare complete independent and pooled fields, archive continuation, pacing,
failure rollback and the new mode's model identity. Correctly rehashed but
noncanonical fields and diagnostics are rejected. The eight preceding artifacts
remain unchanged. Snapshot fuzzing includes zero-start reconstruction.

Dedicated-worker checks run on the same native matrix. They compare physical
digests across paced/headless execution, preserve full-state continuation,
exercise concurrent/stale clock revisions and full unread mailboxes, and check
shutdown wake-up races. The sealed worker-loop allocation counter must stay at
zero. Mailbox allocation and 25% burst reserve are measured independently of
proof-array budgets. Clock transitions join the numerical fuzz target; worker
clock/runner/mailbox mutations have their own nightly matrix entry. These tests
do not qualify canonical factory integration or total application RSS.

Multirate population checks additionally pin initial, partial-timestamp and
final aggregate field digests, check source-order-independent compilation, and
validate every snapshot prefix against the stable event order. Rehashed but
impossible frontiers and fabricated numerical failures must be rejected. The
same suite checks measured construction allocation, allocation-free execution
and capture, and a 1 MiB worker stack. Snapshot fuzzing includes generated and
mutated schedule archives and reordered registrations; nightly mutation targets
the schedule compiler, event loop, memory plan and frontier reader. This evidence
does not establish physical coupling between independent numerical populations.

The same native suite includes mixed connected-gas/drying populations with
different physical timesteps, independent reference sessions and a full snapshot
at every event boundary. Schedule schema `0.2.0` bounds typed child frames and
requires agreement between child failures and the aggregate failed event.
Archived `0.1.0` proof-only snapshots retain their original hashes; migration
checks compare every field, frontier and failure before current-schema capture.
Mixed source-order/snapshot fuzz cases and nightly mutations cover framing and
typed population dispatch. Budget regressions reject conflicting shared-profile
policies and runtime-only fits whose restore workspace exceeds aggregate or
profile limits. Diagnostic decoding must avoid arbitrary-map allocation.
Four-platform CI must recompute these goldens;
successful local tests alone do not establish cross-platform equivalence.

The worker suite now runs aggregate schedules as well as individual proofs.
It verifies native headless/all-rate equivalence, identity-qualified live field
updates, equal-time manual steps, restored failure latches and a full unread
mailbox on a 1 MiB thread with zero execution allocations. Event-clock fuzzing
includes empty/equal/regressing/maximum deadlines and checked state preservation.
Worker memory preflight adds mailbox and stack overhead to the aggregate budget;
it does not assign a new independent budget to each population.

Connected-flow tests add exact extensive-inventory balances across conforming
faces, unequal-volume shear/thermal checks, spatial/time refinement, atomic
failure and compiler peak-allocation measurements. The native matrix checks a
named 24-cell connected-domain field digest plus full-state connected runtime
goldens. Those fixtures verify native recomputation, streamed restart at all 21
step boundaries, paced/headless equivalence and retained failures after worker
restart. They are not yet public CLI replay artifacts. Numerical fuzzing
exercises reciprocal topology, SI compilation and failed-step rollback; snapshot
fuzzing includes generated/mutated connected archives. Native source/CLI
checks reproduce the existing retry fixture from YAML, including every pacing
mode, snapshot continuation, fault retention and no-clobber capture. Source
fuzzing exercises raw/mutated YAML, alias/depth budgets and compilation-order
invariance. Numerical replay remains distinct from checkpoint loading. Nightly
mutation targets the bounded source reader, SI/mesh/model compiler, connected
kernel, session and snapshot reader. The
preceding replay fixtures retain their original meanings.

Connected retry checks additionally compare recovered horizons with explicit
substeps, validate time remainders and conservative work bounds, reject forged
retry metadata, preserve exhausted failures through worker restart, and compare
all pacing modes. Snapshot `0.1.0` fixtures retain their original footer digests;
readers migrate them to `0.2.0` in memory and writers emit only the current schema.
The current header changes state hashes without changing the preceding case's
physical fields, which are independently compared after migration. Separate
retry goldens pin changed numerical paths rather than replacing old outcomes.

Connected-population checks compare every interleaved slot with an independent
session, pin normalized population digests, retain per-slot failures and verify
streamed continuation. Construction/restore peaks include the shared workspace,
accepted bank, metadata and bounded transactional decoder. Native tests cover
1 MiB stacks, allocation-free steps/capture, frame boundaries and output failure;
snapshot fuzzing and a dedicated mutation scope exercise the population layer.

Capacity evidence also includes the experimental factory physics resource
report. Its current independent-workspace baseline is deliberately over budget;
tests require that rejection and preserve the declared mesh/workload inputs.
An informational report is not permission to migrate the canonical model.
Changes to its accounting contract, geometry, residence envelopes or declared
auxiliary storage require corresponding runtime measurements and documentation.
The pooled comparison is retained alongside that baseline, not substituted for
it. Its implemented-field budget is enforced with `--pooled --require-fit`;
new independent fields must be charged before this gate can pass. Passing does
not fill the report's missing physical-validation or full-application evidence.
Native golden jobs also exercise interleaved populations and streamed
snapshot continuation; passing them does not qualify factory integration or
unimplemented constitutive models.

Capacity contract 3 resolves digest-pinned proof sources and accounts for actual
sparse geometry, material tables and compilation peaks. CI retains the original
block report, a coarse source-backed basin report and a refined basin rejection.
Tests preserve all workload declarations, measure construction of 2,778 coarse
product slots on a 1 MiB stack, and reject the refined bank before allocation.
The numerical fuzzer includes source mismatches, missing/duplicate/unused
documents, invalid proof semantics and population-count overflow. A report
collected without `--require-fit` is not acceptance of its failed limits.
An additional reconstructed-refinement report is enforced with `--require-fit`
without removing the previous-start rejection. It preserves the refined mesh,
all workload/reserve declarations and full snapshot fields. Its 2,778-slot
construction and selected-product step are allocation-checked on a 1 MiB stack.
Sparse SSOR preconditioning must retain the source residual tolerance and
iteration limit. All four refined source steps and midpoint continuation are
checked without allocations, including checkpoint digest verification; legacy
scaling failure remains explicit rollback evidence. Numerical fuzzing compares
prepared and reassembled loads across preconditioners and checked failures.
Passing these tests is not physical qualification, a residence-cycle run or
acceptance of unimplemented fields.

The stable toolchain is pinned to Rust `1.100.0`, Node `26.13.1`, npm `12.2.3`,
and SvelteKit `3.4.1`. The project tests this Rust pin and does not maintain a
separate older-Rust compatibility lane.
Repository policy requires the Rust toolchain file,
`.nvmrc`, npm engine metadata, package-manager declaration, lockfile, and
SvelteKit build integration to agree. Web CI jobs install the declared npm
version explicitly before dependency installation; Node's bundled npm is not
the project pin. Node follows the newest stable release line. Dated nightly
Rust remains separate for tools that require it.

## Nightly Evidence

The scheduled workflow produces evidence that is too expensive or
wall-clock-sensitive for pull requests:

- time-bounded fuzzing of identifiers, appliance and connection YAML,
  blueprints, model compilation, conduit scheduling, snapshots/replay, and
  control-program parsing;
- line and branch coverage with crate and critical-module gates;
- mutation testing for safety, capacity, scheduling, replay, blueprint, and
  control-boundary code; each selected scope must enumerate at least one
  mutant, and publishes the `packages/mutants.out` report. Each matrix job
  uses one in-place mutation worker in its disposable checkout so baseline
  and mutant tests have access to fixture files outside the Cargo workspace;
- bounded Miri-compatible tests for model quantities/contracts and the engine
  allocator-free core; long simulated-time cadence tests remain in the soak
  lane rather than being interpreted under Miri;
- Rust and npm advisory checks plus CycloneDX SBOM generation;
- a 24-hour nominal/concurrent-failure acceptance soak;
- explicit engine-runtime and scheduler Criterion targets, allocation counts,
  object sizes, stack sizes, and compiled capacity evidence;
- Chromium, Firefox, and WebKit workflows at desktop and mobile viewports.

Coverage and SBOM artifacts are retained for 90 days. Soak, performance, and
mutation evidence is retained for 90 days. Browser traces are retained for 30
days. Pull-request Playwright and capacity evidence is retained for 14 and 30
days respectively. Full simulation traces are CI artifacts; compact normalized
goldens remain in the repository.

The Rust SBOM command emits one `*.cdx.json` document per workspace package;
the web command emits `packages/web/web-sbom.json`. `cargo xtask verify` parses
both workflow files as YAML and repository policy checks the pinned tool
versions, bounded test scopes, evidence paths, and retention periods.

## Coverage Policy

`project/standards/coverage-baseline.json` is monotonic. A measured decrease of
more than 0.5 percentage points fails even when the absolute threshold is met.
Baseline updates may only increase a recorded value.

| Scope | Lines | Branches |
| --- | ---: | ---: |
| Model, engine, project, and simulation | 90.45% | 80.17% |
| Safety, capacity, scheduler, replay, blueprint compiler, and control boundary | 98.31% | 90.67% |
| Config, operator, API, and CLI | 85.02% | 75.08% |
| Web application | 89.96% | 71.55% |

## Capacity Changes

The compiler, not a manually edited observation table, measures topology,
workload burst, queue reserve, largest runtime object, loader stack, and total
sealed memory. Every bounded resource declares an overflow policy and retains
at least 25% reserve at reviewed load.

A structural maximum may change only when the same pull request contains:

1. an ADR under `project/docs/decisions/` naming the owner, rationale,
   alternatives, memory/stack effect, saturation behavior, and review date;
2. an updated capacity policy and generated `model.lock.json`;
3. focused saturation and recovery tests;
4. updated acceptance, allocation, object/stack-size, and Criterion evidence;
5. a lock update reason that references the ADR.

`project/scripts/check-capacity-change.mjs` compares the pull request against
its base revision and rejects a registry change unless those evidence files
changed together. `cargo xtask verify` separately proves that the reviewed
registry exactly matches the AST-parsed engine constants.

An oversized process area is partitioned into more cells. Increasing a
project-wide factory, site, cell, component, or connection ceiling is not an
accepted capacity response.

## Golden Replay Changes

Golden replay changes require a deterministic rerun on all four replay-matrix
platforms. The reviewer checks the model digest, schema migration, ordered
inputs/faults, embedded full checkpoint state, checkpoint field digests, final
outcome, and first-divergence diagnostics. The pull request must explain every
changed digest. A golden may not be updated solely to make a failing test pass.

## Exceptions

Temporary exceptions require all of the following in an ADR or tracked issue:

- one named maintainer as owner;
- the exact check, path, threshold, or platform affected;
- technical justification and bounded risk;
- compensating evidence;
- an expiration date no more than 30 days away;
- a removal issue linked from the exception.

Expired exceptions fail repository policy. Security, safety interlocks,
post-seal allocation, model-lock integrity, stale-revision protection, and
non-loopback unauthenticated writes cannot be excepted.

## Local Commands

```text
node project/scripts/check-version.mjs
node project/scripts/repository-policy.mjs
cd packages && cargo xtask verify
cd packages && cargo xtask contracts --check
cd packages && cargo test --workspace --all-features
cd packages && cargo run --bin hearthline -- model compile --locked
cd packages/web && npm run check && npm run test:unit && npm run build
```

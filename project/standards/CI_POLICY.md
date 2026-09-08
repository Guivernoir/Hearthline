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
| Rust quality (MSRV) | Formatting, Clippy with warnings denied, and `no_std` embedded-target compilation on Rust `1.92.0` |
| Rust quality (stable) | The same checks on pinned Rust `1.98.1` |
| Rust tests (four shards) | All model, engine, config, project, simulation, operator, CLI, API, and `xtask` tests compiled normally, then their binaries execute directly with a fixed 1.25 MiB worker stack and two test threads; the HMI construction/projection regression separately enforces 512 KiB |
| Capacity and deterministic budgets | Locked compilation, generated capacity report, saturation contracts, 75-cell acceptance model, 24-hour nominal/failure run, allocation counter, queue high-water marks, and structural size budgets |
| Regression corpus and fuzz smoke | Eight parallel checked-in corpora plus short identifier, appliance, connection, blueprint, compiler, scheduler, replay, and control-parser campaigns |
| Svelte and Chromium workflows | Exact npm install, SvelteKit synchronization, audit, type checks, Vitest, static production build, and desktop Chromium Playwright workflows |
| Golden replay matrix | All committed replay artifacts on Linux x64, Linux arm64, Windows x64, and macOS arm64, plus complete CLI replay verification on an explicit 1 MiB worker stack on each platform |

Required check names are repository policy. Renaming, removing, or making one
non-blocking requires a reviewed policy change in the same pull request.

The stable toolchain is pinned to Rust `1.98.1`, Node `26.8.1`, npm `12.0.2`,
and SvelteKit `2.70.3`. Repository policy requires the Rust toolchain file,
`.nvmrc`, npm engine metadata, package-manager declaration, lockfile, and
SvelteKit build integration to agree. Web CI jobs install the declared npm
version explicitly before dependency installation; Node's bundled npm is not
the project pin. Node follows the stable Current release, not the LTS line.
The Rust `1.92.0` MSRV and dated nightly toolchains remain separate test lanes.

## Nightly Evidence

The scheduled workflow produces evidence that is too expensive or
wall-clock-sensitive for pull requests:

- time-bounded fuzzing of identifiers, appliance and connection YAML,
  blueprints, model compilation, conduit scheduling, snapshots/replay, and
  control-program parsing;
- line and branch coverage with crate and critical-module gates;
- mutation testing for safety, capacity, scheduling, replay, blueprint, and
  control-boundary code; each selected scope must enumerate at least one
  mutant, runs without copying the full target tree, and publishes the
  `packages/mutants.out` report;
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

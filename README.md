# Hearthline

**Current development release:** `0.3.2`

Hearthline is a vendor-neutral industrial architecture and deterministic
simulation project. It connects a public customer path, enterprise services,
governed IT/OT exchange, and a segmented ceramics process in one reviewable
model. The project combines a SvelteKit operator and architecture application,
typed YAML source documents, IEC 61131-3 control sources, and Rust simulation.

Hearthline is being built from code because no evaluated simulator covered the
required combination of hierarchical physical and logical navigation,
configuration-owned topology, network and security decisions, virtual control,
process dynamics, deterministic replay, and generated documentation. This is a
scope decision, not a claim that Hearthline replaces network emulators,
controller engineering environments, hardware laboratories, or commissioning.

![Hearthline regional physical architecture](project/docs/screenshot.png)

![Hearthline regional logical architecture](project/docs/logical-screenshot.png)

## Release 0.3.2

Release `0.3.2` is a clean foundation break. It introduces an immutable,
content-addressed project model and separates deterministic execution from
configuration, project compilation, replay, and operator sessions.

The canonical model currently validates:

| Evidence | Current result |
| --- | ---: |
| Appliance documents | 394 |
| Connection documents | 450 |
| Scenarios | 30 |
| Canonical runtime cells | 29 |
| Blueprint instances | 2 |
| Normalized compiled objects | 936 |
| Capacity assessments | 255 accepted |
| Minimum reviewed capacity reserve | 25% |
| Golden replay classes | 6 |

The canonical project digest is recorded in
[`model.lock.json`](project/config/model.lock.json). Compilation is independent
of source traversal order. The lock records source and object digests,
compiler/schema versions, partition assignments, generated catalogs, and
capacity evidence; expanded per-instance YAML is deliberately not committed.

## Architecture

Dependencies flow downward through these Rust crates:

| Crate | Responsibility |
| --- | --- |
| `hearthline-model` | `no_std` identifiers, events, units, fixed-point quantities, and stable contracts |
| `hearthline-engine` | allocator-free component, network, process, safety, and cell behavior |
| `hearthline-config` | versioned YAML contracts, validation, and immediately previous schema migrations |
| `hearthline-project` | blueprint expansion, graph compilation, capacity planning, model locking, catalogs, and draft transactions |
| `hearthline-sim` | deterministic site/cell scheduling, snapshots, scenarios, recording, and replay |
| `hearthline-operator` | typed commands, permissions, projections, model-revision pinning, and operator sessions |
| `hearthline-cli` | command-line adapter |
| `hearthline-api` | loopback-first HTTP adapter and generated host contracts |

`xtask` verifies dependency direction from Cargo metadata and inspects Rust
syntax trees for runtime policy. Svelte owns presentation and interaction; it
does not decide identity, routing, policy, control, or process state.

### Deterministic Runtime

Projects grow by composing sites and isolated cells during loading. There is no
project-wide factory, site, or cell ceiling. Each cell retains reviewed,
allocator-free engine limits, and an oversized area must be partitioned rather
than accommodated by enlarging a global constant.

Cross-cell and cross-site traffic uses preallocated bounded conduits. The
single-threaded scheduler orders work by time, site, cell, conduit, and
sequence. Every bounded resource declares reject, backpressure, coalescing,
drop, stop, or recovery behavior. Tests reject post-seal allocation and exercise
all conduit saturation policies.

State-affecting deterministic values use typed fixed-point quantities for
pressure, temperature, flow, mass, concentration, position, angle, percentage,
and time. Quantization occurs before transitions, comparisons, snapshots, and
hashing. The x86 build disables fused multiply-add as an additional host
guardrail.

### Blueprints And Capacity

Typed blueprints support bounded repetition, nested instances, namespaced local
IDs, typed parameters with units and ranges, and exported ports, signals,
material handoffs, and network conduits. Imports are acyclic and pinned by
schema version and digest. Blueprints intentionally provide no script or
arbitrary string-template language.

Direct appliance and connection YAML remains supported for unique assets.
Repeated cells and process trains can migrate incrementally to blueprints.
Compiler-generated capacity evidence covers topology demand, workload and
burst envelopes, object and stack size, queue demand, and aggregate preallocated
memory. Missing evidence, missing overflow behavior, or less than 25% reviewed
reserve rejects compilation.

### Recording And Replay

Run manifests bind the model digest, simulation version, scenario, actual
initial-runtime digest, quantization contract, clock policy, ordered commands
and faults, limits, and expected outcomes. Replay checkpoints retain full
normalized component and cell snapshots together with their component and field
digests. Verification checks that the digest evidence matches the embedded
state, then reports the first divergent event, component, and field.

Current and immediately previous blueprint, lock, snapshot, and replay schemas
are readable. Readers migrate in memory; writers emit only the current schema.
Compact golden artifacts cover network, safety, Forming, Body Preparation,
overload, and recovery behavior. Full traces are retained as CI artifacts.

### Transactional Editing

The configuration interface edits drafts against an immutable model revision.
A draft may overlay blueprint, instance, appliance, connection, or scenario
documents without modifying tracked files. Preview compilation returns
source-located diagnostics, topology changes, capacity deltas, scenario impact,
and generated catalog previews.

Commit requires an unchanged base revision and a valid complete overlay. A
write-ahead journal, temporary files, `fsync`, atomic renames, rollback, and
startup recovery protect multi-file source, lock, and catalog updates. The host
rejects path traversal, unknown roots, oversized or deeply nested YAML,
duplicate IDs, and stale revisions. Write APIs bind to loopback by default and
refuse non-loopback access unless authentication is configured.

Active simulations remain pinned to the immutable revision on which they were
started. A committed model affects only new or explicitly restarted sessions;
older sessions are reported as stale.

## Modeled Environment

The SvelteKit application provides physical and logical views for three sites:

| Site | Scope | Documentation |
| --- | --- | --- |
| Customer Network | Residential LAN, customer edge, and public web path | [Customer Network](project/docs/customer-network/README.md) |
| Central Office | IT DMZ, Business IT, governance, monitoring, analytics, and approved exchange | [Central Office](project/docs/central-office/README.md) |
| Factory | Factory-local OT DMZ, Level 3 services, vPLC platform, and process cells | [Factory](project/docs/factory/README.md) |

The factory process is:

```text
Body Preparation
  -> Forming
  -> Controlled Drying
  -> Industrial Dryer
  -> Color and Glaze
  -> Kiln 1
  -> Intermediate Inspection
  -> Kiln 2
  -> Final Inspection
  -> Logistics
```

Body Preparation and Forming are the deepest current process models. Body
Preparation includes separate slip, industrial-water, return-water, and glaze
trains with local HMIs, supervised pumps, quality instruments, material
handoffs, and pipeline-loss effects. Forming includes four equal moulds,
mould-local HMIs and I/O, guarded robot and handoff behavior, machine-level
supervision, historian replication, and source-bound controller behavior.

The 30 canonical scenarios preserve the existing customer, enterprise,
availability, HA, OT exchange, local-autonomy, historian, and controlled web
security outcomes. This is selected-path evidence, not proof that every possible
pair of endpoints or every production condition has been modeled.

## Current Limits

- Appliance values and architecture definitions are provisional engineering
  placeholders. They exercise contracts and behavior but are not deployment
  configurations or final equipment selections.
- The complete graph compiles and is capacity-checked, but only declared
  scenarios establish end-to-end communication outcomes.
- Forming and the Body Preparation slip train execute a bounded Structured Text
  subset. This is not a general IEC 61131-3 runtime.
- Process physics are deterministic development models based on public
  information. They are not controller, robot, supervisory-platform, or
  production-process equivalence claims.
- Deployment and standards conformance are not claimed. The project documents
  alignment decisions and unresolved qualification work separately.

## Acceptance Evidence

The generated scale project contains a Central Office, customer edge, and three
ten-area factories with 1,200 or more components, 1,400 or more connections,
and at least 75 isolated cells. It runs 24 simulated hours plus a deterministic
concurrent-failure schedule without unreported saturation or post-seal
allocation. Construction, snapshots, operator projection, and replay are also
tested on constrained worker stacks to prevent recurrence of stack overflow.

Pull requests run architecture, version, schema, lock, generated-output,
formatting, Clippy, MSRV, stable, `no_std`, sharded tests, capacity, regression
corpus, fuzz smoke, SvelteKit, Vitest, Chromium, and four-platform golden-replay
gates. Nightly evidence adds deep fuzzing, branch coverage, mutation testing,
Miri, dependency and license policy, SBOMs, soak runs, benchmarks, allocation
and size reports, and Chromium/Firefox/WebKit desktop and mobile workflows.

See the [CI policy](project/standards/CI_POLICY.md) for thresholds, retention,
capacity-review rules, replay review, and exception ownership.

## Command Line

From the repository root:

```bash
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- model validate
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- model compile --locked
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- model lock --update --reason "review reference"
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- model expand --output /tmp/hearthline-expanded
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- capacity report --format text
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- run foundation-conduit-overload --record /tmp/run.json
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- replay verify project/replays/conduit-overload.json
```

Run the API and web application in separate terminals:

```bash
cargo run --manifest-path packages/Cargo.toml -p hearthline-api
```

```bash
cd packages/web
npm ci
npm run dev
```

Primary local gates:

```bash
node project/scripts/check-version.mjs
node project/scripts/repository-policy.mjs
cd packages && cargo xtask verify && cargo xtask contracts --check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd web && npm run check && npm test && npm run build && npm run test:e2e
```

## Roadmap

1. Complete migration of repeated process trains and cells to reviewed
   blueprints while retaining direct YAML for unique assets.
2. Extend formal communication and fault behavior to additional declared
   component families and policy paths.
3. Add finite cross-area inventory, receiving capacity, interrupted material
   transfer, recipe deployment, and deterministic recovery.
4. Broaden selected IEC 61131-3 compatibility only after the language,
   scheduling, and interchange contracts are explicitly versioned.
5. Replace provisional configuration and architecture values with
   scenario-derived, cross-validated, reviewed engineering definitions.
6. Continue controlled security, failover, local-autonomy, and recovery cases
   without turning simulated evidence into a deployment claim.

## Documentation

- [Documentation index](project/docs/README.md)
- [Implementation direction](project/docs/reference/project-direction.md)
- [Simulation engine](project/docs/reference/simulation-engine.md)
- [SvelteKit application](project/docs/reference/svelte-application.md)
- [Configuration model](project/config/README.md)
- [Deployment conformance](project/docs/reference/deployment-conformance.md)
- [Versioning](project/docs/reference/versioning.md)
- [Changelog](CHANGELOG.md)

## License

Hearthline is licensed under the [MIT License](LICENSE).

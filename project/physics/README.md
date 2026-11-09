# Coupled Factory Physics: 0.4.0 Work In Progress

The release target is a live, vendor-neutral 3D continuum simulation from Body
Preparation through final inspection, including plant air and sector-local
supplies. The existing `0.3.2` model is intentionally unchanged while numerical
qualification is in progress. This directory is a verification workspace, not
a substitute canonical plant or a production recipe collection.

## Implemented Numerical Proofs

| Capability | Evidence and boundary |
| --- | --- |
| Checked numerical arithmetic | Signed decimal billionths, wide intermediates, nearest-even rounding, explicit overflow/division errors |
| Spatial transport | Cell-centered Cartesian 3D finite volumes, implicit backward Euler, deterministic lexicographic Gauss-Seidel |
| Isothermal receiver equilibration | Finite ideal-gas mass storage and linear pressure-driven conductances; no compression work, momentum equation, or compressor equipment yet |
| Compressible flow kernel | Conservative density, three momentum components and total energy in 3D; implicit Rusanov fluxes, Newtonian viscous stress, Fourier heat conduction, periodic/slip/no-slip boundaries and bounded whole-horizon timestep retries; turbulence and plant integration remain pending |
| Compressible receiver proof | Two finite chambers on a fixed Cartesian mesh, SI inputs with checked reference-scale conversion, pressure-driven charging and reflected waves; conserved-field snapshots include accepted-step retry metadata |
| Mechanical kernel | Compiled linear tetrahedra, matrix-free or immutable sparse elasticity, imposed expansion/shrinkage strain and checked iterative equilibrium; verified free/restrained expansion and rigid motion, not fracture or a calibrated product model |
| Drying specimen | Spatial moisture/heat diffusion, surface exchange, evaporation/condensation heat; optional tetrahedral equilibrium driven by local thermal/moisture eigenstrain, with displacement and stress snapshots; constant properties, no fracture or calibrated ceramic law yet |
| Constitutive transport | Per-cell harmonic interfaces, bounded property tables, implicit nonlinear diffusion and latent-heat coupling; optional solid/liquid sensible enthalpy and conservative upwind liquid heat transport with bounded root solves; digest-pinned material YAML, standalone-session/population replay and analytical/refinement tests; synthetic inputs, no calibrated ceramic predictions |
| Original basin | Digest-pinned geometry, active-volume transport without fictitious cavity inventory, compiled mechanical sparsity and complete geometry-aware replay; coarse orthogonal development mesh, not qualified product geometry |
| Memory | Actual field-buffer allocations checked against compiled measurements; zero allocations during successful stepping |
| Factory resource study | Typed population/workload compiler, shared geometry, independent fields/snapshots and declared auxiliary envelopes; the current full-factory baseline exceeds the memory budget |
| Pooled proof runtime | Independent state banks with one serial workspace per profile, allocation-free stepping and streamed capture, bounded validated loading, opt-in zero-start elastic reconstruction; not yet integrated with the factory worker |
| Transactions | Failed coupled steps preserve physical state, time and conservation ledgers |
| Replay | Complete initial/intermediate/final fields, model and numerical-contract digests, exact re-execution and checkpoint continuation |
| Pacing | Tested pause/single-step and 1x/2x/5x/10x wall-time accumulator; not yet connected to the API or HMI |
| Configuration | Strict standalone proof YAML and generated JSON Schema; not yet included in canonical drafts or `model.lock.json` |

Analytical tests cover two-volume exchange, constant fields, manufactured
spatial gradients, quadratic mesh convergence, timestep refinement, closed-gas
mass/energy conservation, and elastic patch cases. Three-dimensionality here refers
to the numerical mesh, not a claim of complete CFD. No material or equipment
parameter in these fixtures has been experimentally calibrated. Passing their
tests demonstrates implementation properties, not real-world predictive accuracy.
The [constitutive transport work](materials/README.md) records equations, units,
workspace requirements, standalone integration and remaining validation work.

## Run The Proofs

Run from the repository root:

```sh
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/receiver-equilibration.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/drying-specimen.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/drying-deformation.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/compressible-receiver.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/variable-drying.yaml --material project/physics/materials/synthetic-moisture-diffusion.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/enthalpy-drying.yaml --material project/physics/materials/synthetic-drying-enthalpy.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics budget project/physics/proofs/drying-specimen.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics replay project/physics/replays/drying-specimen.json
```

`physics prove <yaml> --record <path>` writes a numerical replay artifact.
Ordinary proof and replay commands do not modify source files. Replays embed
their configuration; they do not depend on checkout paths, YAML traversal
order, platform newline conventions, or a running web application. Unknown
schema/numerical versions and altered fields are rejected. This is the first
proof schema, `0.1.0`; no earlier proof schema exists to migrate.
The standalone adapter limits source/replay documents to 16 MiB. Recording and
verification check a conservative artifact-size estimate before constructing
meshes or snapshot fields. Larger runs require the still-pending streaming
factory archive; the adapter does not silently discard fields or checkpoints.

The proof snapshot stores the accumulated signed conservation residual instead
of hiding it through clamping. A receiver's first field is absolute pressure
in Pa. A drying specimen's first field is kg water per kg dry solid. Temperature
is always K. Serialized field integers use the declared billionth resolution.
Mechanical displacements are node-major xyz in reference-length units; stress
is element-major xx, yy, zz, yz, xz, xy in reference-stress units. The coupled
fixture specifies both scales. Its one-way small-strain coupling does not feed
deformation back into transport geometry or account for thermoelastic heat.
These omissions are explicit proof assumptions, not accepted factory defaults.

The compressible fixture has an 8 L upstream region initially at 200 kPa
absolute and a 16 L receiver region initially at 100 kPa absolute, both at
300 K. Their interface is open from time zero; external walls are stationary
and adiabatic. The pressure discontinuity is an idealized initial condition,
not a model of a real valve-opening transient. This is a coupled finite-volume
case, not a compressor station or a qualified pressure-vessel design.

Its conserved fields are normalized density, momentum xyz and total energy
density. Reference length L, velocity U, density rho and gas constant R define
the scales L/U, rho U^2 and U^2/R for time, pressure and temperature. Physical
session time remains in SI microseconds. Retry metadata labels its separate
normalized solver time explicitly. A positive transport coefficient that would
round to zero is rejected during compilation. Checkpoints occur at accepted
step boundaries; iterative scratch buffers restart from committed fields.

The gas proof has eight cells and allocates 768 field-buffer bytes. The drying
proof has 27 cells and allocates 2,592 field-buffer bytes. Eight scalar fields
include actual state, material storage, workspaces and rollback storage; two
boundary fields complete the count. The default 1.5 GiB budget is a ceiling,
not an eagerly allocated arena. At least 25% of the budget remains reserved.
Host objects, configuration strings, checkpoint serialization, allocator
overhead, the browser, and compiler/build memory are not included in this
numerical-buffer report. It is **not a full-factory memory estimate**.
The deformation proof additionally budgets immutable node/tetrahedron/material
data, restraints, nine displacement-sized buffers and thirteen element-sized
scalar buffers, including outer rollback fields. Its geometry has 64 nodes and
162 tetrahedra over the same 27 transport cells. Distinct sessions share the
immutable model but allocate independent physical fields.
The compressible fixture allocates 5,568 numerical-buffer bytes: five
conserved-state buffers and four scalar projection/trial buffers for 24 cells.
It does not allocate diffusion-specific storage or boundary arrays. The host
retains signed mass, momentum and energy residuals rather than hiding them.

The separate [factory resource study](capacity/README.md) now measures how these
layouts scale with simultaneous equipment and multi-day product inventory.
Its current per-instance-workspace baseline needs about 6.87 GiB and is rejected
against the 1.5 GiB budget with reserve. This result is retained as evidence;
it is not hidden by reducing residence time or coarsening a mesh. The study
is not yet a qualified factory layout or total application RSS measurement.
The implemented [pooled runtime and archive](populations/README.md) reduce the
unchanged study to about 393 MiB for implemented fields, below the usable budget
after reserve. Each instance retains independent state; history-free elastic
stress and gas readings are reconstructed exactly and remain in snapshots.
Unimplemented constitutive fields are not included. The [original basin proof](geometry/README.md)
now exercises a sparse solid domain, including cavity surfaces and deformation.
Its geometry, wet-material law and temporary assembly storage are now included
in separate [source-backed studies](capacity/source-backed.md), retaining the
block baseline. The coarse case fits, but the first mesh refinement requires
1,556,311,074 bytes and fails the reviewed budget. A separate
[zero-start elastic study](populations/reconstruction.md) retains that refined
mesh and workload, using deterministic equilibrium reconstruction to reduce
the implemented-field plan to 681,641,106 bytes. Its solver contract is opt-in;
the old study and replay meanings remain unchanged. Its sparse SSOR
preconditioner completes all four refined source steps within the original
1,000-iteration limit, where legacy scaling failed even with 2,000. Factory integration, further
storage work and production mesh qualification remain necessary; this is not a
completed memory gate.

The PR matrix recomputes all nine committed artifacts on Linux x64, Linux arm64,
Windows x64, and macOS arm64. Tests also execute replay and continuation on an
explicit 1 MiB worker stack. Native matrix results must be collected before
cross-platform qualification is declared.

## Release Work Remaining

The accepted release remains one integrated `0.4.0` release, with internal
gates in this order:

1. Complete the numerical gate: qualify and couple the new flow/momentum and
   solid-mechanics kernels, extend turbulent and material constitutive
   behavior, robust checked scaling across operating envelopes, public
   experimental comparisons, and an accepted full-factory memory/resource plan.
   Establish exact four-platform replay and validate operation on two CPU
   cores with 6 GB RAM. No automatic fidelity reduction or real-time guarantee.
2. Build an enterable central compressed-air station, independent local
   compressors and receivers in each sector, and separate inlet-dependent
   boosters. Include cooling, separation, drying, filtration, condensate,
   regulators, non-return valves, pressure-dependent leakage, storage and
   electrical dependencies. Complete spatial water and segregated returns.
3. Replace Body Preparation phase-generated measurements with material
   balances and equipment evolution. Include dosing, mixing, particle-size
   distributions, rheology, treatment loading, waste, sampling, laboratory
   release and authoritative bounded Structured Text controllers.
4. Track individual lots and products through the four existing moulds,
   controlled drying, industrial drying, glazing, both recipe-routed kilns and
   inspection. Use an original basin plus validation coupons. Add moisture,
   thermal, shrinkage, stress and regularized damage fields; inspection sees
   measurable effects rather than hidden solver truth. Account for repair,
   refiring, rejection and release to the existing logistics boundary.
5. Integrate the dedicated simulation worker, session-wide clock, instructor
   fault schedules, sensor dynamics, material/geometry/recipe drafts, generated
   host API contracts, Three.js field inspection and bounded historian.
   Preserve the existing HMI, PC, robot and safety authority boundaries.

Future material contracts include `MaterialLot`, `ProductInstance`,
`PhysicalPort`, `PhysicalConnection`, `PhysicsProfile` and `FaultSchedule`.
These are planned contracts, not empty types added to imply completion.
Meshes, properties, physical connections and solver settings must join the
immutable compiled project and lock before canonical model migration.

Faults must alter physical causes, not assign a defect outcome: a pipe opening
admits air only when pressure permits ingress; a healthy heartbeat does not
prove useful pumping; clearing an injection does not undo contamination or
damage. Process Hold and emergency actions do not freeze stored heat,
pressure, inertia or continuing material evolution. Software pause is separate.

## Public Engineering Basis

- [DOE compressed-air references](https://www.energy.gov/cmei/ito/compressed-air-systems): utility topology and equipment phenomena to implement.
- [EPA hydraulic benchmarks](https://www.epa.gov/water-research/epanet): compatible water-network comparisons, not a slip/air solver.
- [Moisture-dependent ceramic drying study](https://doi.org/10.1016/j.jeurceramsoc.2021.07.042): experimental-property requirements and coupled drying physics.
- [NIST verification and validation guidance](https://doi.org/10.6028/NIST.IR.8298): separating numerical verification, physical validation and uncertainty.
- [Published finite-element elasticity formulation](https://mfem.org/examples/): weak-form stress/displacement reference for the independent mechanical kernel.
- [NASA compressible-flow equations](https://www1.grc.nasa.gov/beginners-guide-to-aeronautics/navier-strokes-equation/): conservation, viscous stress and heat-flux formulation; not validation of the implemented discretization.

These references do not validate the current proof fixtures. The project makes
no commissioning, functional-safety, burner-management or real-plant defect
prediction claim. Project code remains MIT licensed; reference material and
any future imported datasets retain their own licenses.

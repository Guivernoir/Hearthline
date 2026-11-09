# Zero-Start Elastic Reconstruction

Status: experimental numerical storage, not a qualified factory material model.
See the [architecture decision](../../docs/decisions/0003-reconstructible-elastic-equilibrium.md)
for assumptions and restrictions.

`case.mechanics.initial_guess: zero` makes the current linear-elastic solve a
deterministic projection of a product's independent moisture and temperature.
The default `previous-step` mode retains its displacement history unchanged.
Zero-start mode uses normalized symmetric forward/backward (SSOR, omega=1)
preconditioning for compiled sparse mechanics, or normalized Jacobi for the
matrix-free path. The numerical contracts are separately versioned; equilibrium
tolerances are unchanged.
Both solve the same discrete equation, but their rounded iterates and solver
reports need not be identical. They are different compiled model identities.
No existing golden artifact is replaced.

## Memory Evidence

`factory-basin-reconstructed.yaml` changes only the refined study's product
initial-guess contract and source identity. All 21 populations, 3,095 total
slots, 2,778 product slots, production/residence envelopes, auxiliary storage and
reserve remain unchanged. Each product still has 8,576 material cells, 13,119
mechanical nodes and 51,456 tetrahedra.

The old per-product state is `16 * cells + 24 * nodes` bytes. Zero-start mode
retains `16 * cells` bytes; the shared worker still has the entire displacement
and stress fields. No integer precision, spatial resolution or constitutive
parameter changes. These formulas count array payloads, not allocator overhead.

| Resource | Previous-step refinement | Zero-start refinement |
| --- | ---: | ---: |
| Product state per slot, bytes | 452,072 | 137,216 |
| All product state slots, bytes | 1,255,856,016 | 381,186,048 |
| All population state banks, bytes | 1,263,253,392 | 388,583,424 |
| Shared product worker, bytes | 9,282,856 | 9,282,856 |
| Full checkpoint fields per product, bytes | 2,921,960 | 2,921,960 |
| Largest decoder envelope, bytes | 72,363,566 | 72,363,566 |
| Factory study runtime plan, bytes | 1,556,311,074 | **681,641,106** |
| Compilation peak plan, bytes | 386,076,288 | 386,076,288 |
| Usable budget after 25% reserve, bytes | 1,207,959,552 | 1,207,959,552 |
| Implemented-field result | Rejected | Fits |

The array reduction is 874,669,968 bytes, not an estimated compression ratio.
The new runtime plan is about 650 MiB. The allocation test constructs all 2,778
refined slots on a 1 MiB worker stack: requested constructor bytes must equal
391,202,296, excluding the separately compiled immutable model. It advances a
selected product without allocations, reconstructs every displacement/stress
value, and validates the complete checkpoint independently. Other products
retain their initial state.

This is not a complete population/residence soak or total RSS measurement.
Firing, damage, glaze, chemistry, particle populations and other required fields
remain absent from these estimates. The synthetic wet material is not valid at
kiln temperatures; using its layout for a reserved kiln slot does not implement
kiln physics. The full-factory memory gate has not passed.

## Refined Solver Evidence

Executing the actual refined model exposed a fixed-point preconditioning
problem in the legacy solve. Its first elastic step requires a normalized
maximum force residual of `0.00001`. The old path misses that tolerance at
both 1,000 and 2,000 iterations. Normalizing Jacobi alone accepted the first
step but failed a later step; recomputing every residual also failed. Neither
experiment was sufficient to qualify reconstruction. The selected sparse SSOR
path completes every step in the four-step source within the original limit.

| Preconditioning | Iteration limit | Used iterations | Final true residual | Accepted |
| --- | ---: | ---: | ---: | --- |
| Legacy | 1,000 | 1,000 | 0.000084533 | No |
| Legacy | 2,000 | 2,000 | 0.000131667 | No |
| SSOR, step 1 | 1,000 | 191 | 0.000008229 | Yes |
| SSOR, step 2 | 1,000 | 240 | 0.000008767 | Yes |
| SSOR, step 3 | 1,000 | 317 | 0.000007981 | Yes |
| SSOR, step 4 | 1,000 | 359 | 0.000009233 | Yes |

For `A = D + L + U` and `s = max(D)` over free DOFs, solve `(D+L)y = s*r`,
then `(D+U)z = D*y`. The same vector holds `y` and then `z`; constrained correction
DOFs remain zero. Each row accumulates checked wide products before one rounded
division. The symmetric factorization is suitable for conjugate gradients;
it changes conditioning, not the equilibrium equation. See the public
[PETSc SSOR reference](https://petsc.org/main/manual/ksp/#sor-and-ssor-preconditioners).
The matrix-free fallback computes `z = r*s/D` with one wide quotient.

The load vector is assembled once per solve in an existing buffer, never reused
across products or changed fields. This avoids repeated element load assembly
without additional allocation. True assembled force residuals still decide
acceptance; recurrence drift restarts the search. Mesh, tolerance, stiffness and
integer precision remain unchanged. The algorithms enter the new model digest.

Analytical sparse/matrix-free tests check equilibrium and failure rollback at
two stiffness scales, prescribed nonzero displacements and exact prepared-load
equivalence. A two-by-two factor inverse independently checks the sweep.
A legacy 2,000-iteration regression retains the observed
nonconvergence and rollback; the source limit remains 1,000. These numerical
checks do not establish basin mesh convergence or material validity. A separate
unit-cube manufactured solution, `u = (x*(1-x), 0, 0)` with constant body force,
uses 2, 4 and 8 subdivisions per axis. It checks second-order centroid
displacement error and first-order centroid strain error with allocation-free
solves on a 1 MiB stack. This verifies the linear kernel against an analytic
solution, not the basin's ceramic material or drying residence cycle. Native replay,
physical validation and the complete workload remain qualification gates.

## Replay And Failure Evidence

The ninth numerical artifact, `wet-elastic-zero.json`, combines wet sensible
energy, moisture-dependent transport and zero-start mechanics. Its final digest
is `8c581d4232237a2016d599ec3c3feb3d8f4f5c34973b00b1ec3c64346ed472b6`.
Tests re-execute it and compare pooled continuation, pause/step, all four pacing
rates, archive restoration and independent sessions exactly. Native CI must
still provide four-platform evidence; a local pass does not establish that.
The refined four-step fixture separately pins full-state digest
`ea31345903c06a614208d12ccfdc63e06a281e0d8de61aa1612a7f40eb85740e`
and restores its midpoint on a 1 MiB stack without allocating during restore
or subsequent steps. Digest validation compares fixed-size bytes; it does not
allocate a temporary string. These short runs are not a full drying cycle.

Corruption tests rehash altered fields before attempting restore, so they
exercise reconstruction validation rather than only the checksum guard.
Changed displacement, stress, solver diagnostics, and non-reference initial
fields are rejected without changing the live state. A previous-start result
cannot be relabeled as a zero-start checkpoint. Snapshot fuzzing includes both
valid and mutated zero-start population archives.

## Reproduce

From the repository root:

```sh
cargo run --locked --manifest-path packages/Cargo.toml --bin hearthline -- physics capacity project/physics/capacity/factory-basin-reconstructed.yaml --proof project/physics/proofs/basin-wet-reconstructed.yaml --geometry project/physics/geometry/original-basin.yaml --material project/physics/materials/synthetic-wet-drying.yaml --pooled --require-fit
cargo run --locked --manifest-path packages/Cargo.toml --bin hearthline -- physics replay project/physics/replays/wet-elastic-zero.json
cargo test --locked --manifest-path packages/Cargo.toml -p hearthline-sim --test physics_reconstruction --test physics_capacity --test physics_golden
```

The refined full-field proof exceeds the standalone whole-replay adapter's
16 MiB bound. Its state bank uses the existing bounded per-product archive
interface; the capacity test does not silently omit fields to make a compact
whole-replay file fit. Factory archival publication remains separate work.

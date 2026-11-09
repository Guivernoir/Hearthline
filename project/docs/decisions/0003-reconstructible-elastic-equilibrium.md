# 0003: Opt-In Reconstructible Elastic Equilibrium

Status: accepted for isolated numerical proofs; not canonical factory migration.

## Context

The refined basin resource study retains 2,778 product slots. Persisting each
slot's displacement vector requires 874,669,968 bytes. The existing elastic
law is quasistatic, linear and history-free, but its iterative solver starts
from the previous displacement. That numerical history affects rounded solver
results and iteration counts even though the physical equation has no history.
Discarding it without a different numerical contract would break exact replay.

## Decision

Add `case.mechanics.initial_guess: zero` as an explicit opt-in. The omitted
value remains `previous-step`, and serialization omits that default so existing
source identities and all previous numerical replay artifacts remain valid.
Zero mode enters a versioned reconstruction contract into the compiled digest.

Every accepted zero-mode mechanical step solves the same assembled equilibrium
equation from zero, with the same fixed-point arithmetic, restraints, residual
criterion and bounded iteration count. Compiled sparse mechanics uses one
symmetric forward/backward SSOR sweep (omega=1); matrix-free mechanics uses
Jacobi. Both normalize by the largest unconstrained diagonal and use checked
wide arithmetic. The sparse sweep reuses an existing vector; load assembly is
cached only within a single solve. The numerical contracts are separately
pinned. The result is a deterministic function
of the immutable model and the current moisture and temperature fields. The
initial checkpoint is the undeformed reference state; it is not a previous
equilibrium iteration. Restore checks those reference fields at step zero.

The population bank retains independent moisture, temperature, time, conservation
ledgers and solver diagnostics. Its shared worker reconstructs displacement and
stress before exposing them. Reconstruction must reproduce the stored solver
report exactly. Checkpoints still contain every displacement and stress value;
restore recomputes and compares them rather than accepting a merely
self-consistent displacement/stress pair. Failed solves or restore validation
do not publish any state to the bank.

The worker still preallocates all displacement, stress, rollback and solve
buffers. Moving displacement from persistent state to shared workspace must not
reduce the standalone worker or full-checkpoint accounting. Population reporting
advances to storage contract 5; the existing archive shape is unchanged.

## Scope And Consequences

This is a memory/computation tradeoff, not compression or a lower-resolution
model. Loading an advanced product currently repeats an equilibrium solve;
stepping it then solves its next equilibrium. Snapshot capture and validation
also perform bounded solves. No real-time or residence-cycle performance claim
is made. Numerical iteration paths differ from previous-step mode, so the new
proofs have their own identities and replay artifacts. Existing proofs are not
rewritten to make their hashes match.

This decision applies only to the implemented history-free, one-way coupled
elastic law. Plastic strain, fracture/damage history, evolving geometry,
viscoelasticity, inertia, velocity, changing restraints and other future state
cannot be discarded under this contract. Their own complete persistent state,
capacity measurements and reconstruction analysis are required. The release
still needs those physical models; this proof does not replace them.

The old refined study remains a tested rejection. A separate reconstructed study
retains its geometry, material laws, solver tolerances, residence envelopes,
population counts and 25% reserve. The full-factory resource gate remains open
until all required physics and application memory are accounted for.
The legacy refined solve misses its tolerance at both 1,000 and 2,000 iterations.
Normalized Jacobi alone passed the first step but failed later. The selected
sparse SSOR path completes all four source steps in 191, 240, 317 and 359
iterations, retaining the original 1,000 limit and 0.00001 force tolerance.
The old scaling failure remains regression evidence; it is not
silently fixed by changing the old replay algorithms or increasing source limits.

## Verification Basis

The stateless equilibrium premise follows the linear-elastic weak-form problem,
not a claim that all mechanics are stateless. [MFEM's linear-elasticity example](https://mfem.org/examples/#example-2-linear-elasticity)
provides the public equation reference; its separate time-dependent nonlinear
example illustrates why dynamic state is outside this decision. The exact
fixed-point reconstruction property is established by Hearthline tests, not by
that reference.
The [PETSc SSOR documentation](https://petsc.org/main/manual/ksp/#sor-and-ssor-preconditioners)
provides the public preconditioning reference. Hearthline implements its own
checked fixed-point sweep; this is not an integration with PETSc.

Tests cover independent/interleaved execution, full snapshot equality, pacing
and restart, correctly hashed but noncanonical projections, failure atomicity,
constrained stacks and allocation counts. Separate refined-bank construction and
capacity checks preserve the old failure evidence. This is conditionally
verified numerical behavior, not validated ceramic material prediction.

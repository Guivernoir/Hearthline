# ADR 0002: Qualify Numerical Physics Before Factory Migration

Status: accepted architecture; numerical proof implementation in progress.

## Decision

Add `hearthline-physics` below `hearthline-engine`. It remains `no_std` and
allocator-free and receives buffers owned and preallocated by host code.
Checked numerical arithmetic has its own explicit resolution and rounding
contract rather than reusing saturating control-signal arithmetic.

Proof YAML lives outside canonical `project/config` until the numerical gate
passes. The compiler creates an immutable proof model with a digest covering
configuration and numerical contract. Simulation owns state and replay; CLI
is an adapter. The running factory, old replay artifacts and model lock are
not migrated merely because a transport demonstration runs.

Exact re-execution on all four CI platforms is required. Recording solver
outputs and playing them back does not satisfy this requirement. Numerical
failure must not publish partially advanced fields or hide conservation loss.

The requested full-3D continuum release must respect the two-core, 6 GB baseline.
When compute cannot meet requested pacing, simulation runs slower without
silently changing equations, mesh, iteration limits or event order. Runtime
memory demand is measured and checked before allocation; increasing structural
engine limits remains subject to ADR and capacity-policy review.

## Consequences

Scalar transport and isothermal receiver storage are useful numerical proofs.
Momentum/energy transport includes optional viscous stress and conduction.
Linear tetrahedral elasticity is coupled to the drying-deformation replay case;
a separate receiver replay covers conserved gas fields and timestep retries.
Neither case is integrated into the plant. Full factory migration remains
gated on remaining constitutive and
equipment models, physical validation and aggregate resource evidence.

The 1.5 GiB physics-buffer budget is configurable, with at least 25% reserve.
It must not be misreported as total application RSS. The first proof schema
has no invented predecessor; later schema revisions must supply migrations.

The [initial aggregate resource study](../../physics/capacity/README.md) rejects
the existing independent-workspace strategy at factory scale. This failed
baseline remains a test fixture. Reusable per-profile scratch and bounded
archival capture now have allocation and equivalence tests. Gas readings and
history-free elastic stress are reconstructed exactly from independently stored
state and still appear in snapshots. The implemented-field comparison fits the
budget. Original basin geometry now has sparse active-volume transport and
constant-law mechanical sparsity; assembly scratch and retained arrays are
preflighted independently, with reserve applied to the larger construction or
runtime peak. The previous block-profile capacity study is not implicitly
qualified by this addition. Product geometry qualification, irreversible material history and
full factory integration must be implemented and measured before an accepted
resource plan is claimed.

Digest-pinned moisture-dependent material tables now compile into standalone
profiles with explicit validity bounds and synthetic/public-source provenance.
Mutable transport coefficients are reconstructed from each instance's field;
only the law/table is shared. Additional scratch, immutable coefficients and
per-instance nonlinear diagnostics are charged in capacity accounting. The
new material-bearing replay does not replace previous constant-law evidence.
Physical calibration, enthalpy-based storage and canonical material integration
remain part of the numerical and release gates.

The integrated release target remains `0.4.0`. The current release metadata
and canonical artifacts stay at `0.3.2` until the release migration is actually
ready. The [physics work record](../../physics/README.md) separates implemented
proofs from the remaining accepted release scope.

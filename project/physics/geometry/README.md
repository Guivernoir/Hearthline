# Original Basin Geometry

This directory contains an independently authored, rectangular open basin for
numerical development. It is not a vendor drawing or a calibrated ceramic
product. The dimensions are synthetic inputs, not manufacturing recommendations.

## Compiled Domain

`original-basin.yaml` declares a 0.48 x 0.36 x 0.18 m outer envelope, 0.02 m
walls and a 0.02 m floor. The proof uses 0.02 m Cartesian spacing:

| Quantity | Compiled value |
| --- | --- |
| Bounding lattice | 24 x 18 x 9 cells |
| Active material volumes | 1,072 |
| Solid volume | 0.008576 m3 |
| Exposed area, including cavity and rim | 0.8912 m2 |
| Mechanical nodes | 2,230 |
| Linear tetrahedra | 6,432 |

The cavity owns no material, transported field or mechanical node unless that
node belongs to an adjoining solid element. Sorted active cell/node identifiers
and reciprocal face adjacency are compiled once. Interior and exterior surfaces
both exchange moisture and heat with the fixture's declared ambient conditions.
All faces must align with lattice planes; unresolved dimensions are rejected,
not rounded or silently voxelized. Refinement tests preserve solid volume.
One cell through a wall is a starting mesh, not a mesh-converged product result.

The `0.1.0` geometry document is referenced by ID and SHA-256 of its typed JSON
representation. Mapping order, comments and line endings do not affect the
digest. Decimal strings are exact configuration values; changing their spelling
changes the digest. A replay embeds the geometry document and checks its reference
before compiling. Missing documents and digest mismatches are rejected.

## Mechanics And Resource Evidence

The constant linear-elastic law permits an immutable symmetric sparse stiffness
matrix. Assembly uses a total `(row, column, element)` order and checked sums.
Runtime matrix products accumulate in wide integers before rounding each row.
Preconditioned conjugate gradients verify the true force residual before
accepting a solution; recurrence drift triggers a bounded restart. This matrix
cannot be reused unchanged for future damage or evolving stiffness laws.
Existing proofs retain Jacobi. Opt-in [zero-start proofs](../populations/reconstruction.md)
use a versioned symmetric forward/backward preconditioner for sparse mechanics.

The basin fixture uses reference length 0.02 m and reference stress 1,000 Pa.
Its normalized force-residual tolerance of 0.00001 corresponds to 0.000004 N
per unconstrained degree of freedom. The iteration limit remains 1,000. All four
10 s steps converge with nonzero displacement; the final residual is 0.000007
normalized (0.0000028 N). The final maximum displacement component is about
0.00000843 m. These values demonstrate the fixture's numerical response, not
validated drying shrinkage or a complete residence cycle.

The current compiled numerical payload is 1,253,520 bytes of runtime buffers and
3,388,748 bytes of immutable geometry/matrix arrays. Temporary assembly has a
22,237,912-byte upper bound. Preflight checks conservative matrix and scratch
bounds before allocation; the final report uses retained matrix counts and
charges mesh plus the larger of runtime buffers or compiler scratch. The 25%
reserve applies to that peak. Host metadata, allocator overhead and total
application RSS are separate. The [source-backed capacity studies](../capacity/source-backed.md)
now include this geometry and a two-cell-through-thickness refinement. The
coarse case fits implemented fields, but the previous-step refined study fails
the reviewed memory budget. [Zero-start reconstruction](../populations/reconstruction.md)
fits that mesh's implemented-field storage, with normalized preconditioning
reaching the first-step elastic tolerance. Neither geometry is a qualified
production resolution.

## Reproduce

From the repository root:

```sh
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics geometry project/physics/geometry/original-basin.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics budget project/physics/proofs/basin-drying.yaml --geometry project/physics/geometry/original-basin.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/basin-drying.yaml --geometry project/physics/geometry/original-basin.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics replay project/physics/replays/basin-drying.json
```

The geometry-specific numerical contract is
`hearthline-solid-domain/2:orthogonal-basin:sparse-elasticity`. Earlier rectangular
proofs retain their original numerical path and digests. Debug builds optimize
only the numerical dependency, retaining checked arithmetic and debug assertions.

Methods follow the public [NIST finite-volume discretization description](https://pages.nist.gov/fipy/en/3.4.5/numerical/discret.html)
and [Templates for the Solution of Linear Systems](https://netlib.org/templates/templates.pdf)
for sparse storage, preconditioned iteration and residual considerations.
These references do not validate material coefficients. Moisture-dependent laws,
fracture, gas content, deformation feedback, experimental comparison, refinement
studies and factory coupling remain required by the release plan.

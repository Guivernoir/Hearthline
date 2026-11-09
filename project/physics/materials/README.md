# Constitutive Transport Numerics

Status: **Conditionally verified** for the analytical and manufactured cases
described below. This directory does not contain calibrated material data.
The kernels are exposed through standalone proof YAML, compiled material tables,
the proof-session runtime and its pooled storage. Canonical model compilation,
drafts and the factory worker remain unchanged. Existing basin/proof artifacts
retain their original laws; `variable-drying`, `enthalpy-drying` and `wet-drying` are separate
experimental cases.

## Equations And Units

The new operator supports spatially varying, principal-axis conductances on the
existing dense or sparse orthogonal 3D mesh. For moisture, the state `u` is kg
water per kg dry solid, storage `C` is kg dry solid, conductance `G` is kg/s,
source is kg water/s, and time is seconds. For heat diffusion the analogous
units are K, J/K, W/K and W. Nondimensional inputs must preserve those relations.

The implicit equation is:

```text
C * (u_new - u_old) = dt * [sum_faces G_face * (u_neighbor - u_new)
                           + H * (ambient - u_new) + source]
```

For equal half-cell distances, the shared interface coefficient is
`G_face = 2*G_left*G_right/(G_left+G_right)`. Both rows compute the same rounded
value. Zero conductivity isolates an interface. For an exposed surface, film
and half-cell conductances are in series, not interchangeable:
`H = G_film*G_half/(G_film+G_half)`. A prescribed surface value uses `G_half`.
Sparse cavity surfaces follow the same rule. Cross-axis tensor terms and
nonorthogonal or moving meshes are not supported by this operator.

An immutable `PropertyTable` supplies a nonnegative multiplier `f(u)` for the
reference conductance. Piecewise linear interpolation uses a single rounded
wide multiply/divide. Unsorted/duplicate knots, invalid values and queries
outside the table's declared state interval are rejected; no extrapolation or
silent clamping is applied.

Bounded, relaxed Picard iteration freezes coefficients for each linear solve,
then rebuilds them at the candidate state and checks the original nonlinear
equation. A small change between iterates is not sufficient for acceptance.
The previous-time field remains unchanged throughout all nonlinear iterations.
Failure does not publish a candidate state. A stalled quantized linear iterate
above the requested residual produces `InsufficientResolution` explicitly.

The nonlinear drying trial applies the converged boundary water flux as a
thermal source multiplied by latent heat. Outflow cools; condensation heats.
Both input fields remain immutable until the host commits the completed trials.
The original `variable-drying` case retains constant thermal capacity. The
separate [enthalpy case](caloric.md) integrates tabulated dry-solid heat capacity
over temperature instead of changing `C` in this equation. The [wet-energy
extension](wet-energy.md) adds liquid sensible storage and conservative liquid
enthalpy transport. Vapor storage, water activity, shrinkage feedback, damage and calibrated
temperature/moisture-dependent properties remain outstanding.

## Evidence And Bounds

- Heterogeneous two-volume transfer matches the independently derived implicit
  solution and preserves inventory within the reported numerical residual.
- Uniform per-cell coefficients reproduce the existing uniform operator exactly.
- A symmetric variable-conductivity problem has the discrete solution
  `[0.25, 0.75]`. Timestep refinement is also checked against its independently
  integrated continuous differential equation, not another invocation of the
  discrete solver.
- The 3D manufactured field `u=16*x(1-x)*y(1-y)*z(1-z)`, with `k(u)=1+u`, is
  tested on 2, 4 and 8 cells per direction. The independently differentiated
  forcing is `-((1+u)*laplacian(u) + |gradient(u)|^2)`. The test requires shrinking
  spatial error and an absolute fine-grid bound, and runs on a 1 MiB stack.
- In that normalized test, a conservative coarse-grid diagonal bound is 12.125.
  A billionth-resolution state can therefore have several billionths of equation
  residual after rounding. The linear tolerance is `1e-8` and the nonlinear
  tolerance is `1e-7`, well below the allowed spatial error. These are test-specific
  scaling choices, not universal factory tolerances.
- Allocation counters cover successful nonlinear stepping and coupled drying.
  Failure, isolation, interpolation, flux signs, latent heat and invalid inputs
  have focused tests. Signed conservation residuals remain visible.

The nonlinear workspace requires two scalar vectors, one three-scalar coefficient
vector and one two-scalar boundary vector: **56 bytes per cell**, excluding
inputs, immutable tables and host metadata. The coupled drying workspace uses
**80 bytes per cell**, including its four existing scalar vectors and the
additional nonlinear trial/coefficient/boundary arrays. The host reuses the
existing boundary vector, so enabling this law adds **32 bytes per cell** to
the existing session, plus **16 bytes per table knot** of immutable coefficients.
The 27-cell proof allocates **3,456 field-buffer bytes** and **64 material-table
bytes**. Allocation-counter tests match these counts. Config strings, parsed
document metadata and serialized artifacts are outside this numerical payload.
The baseline factory study still uses constant-law profiles; it is not updated
by silently applying this material to every product.

## Material Documents And Replay

[`synthetic-moisture-diffusion.yaml`](synthetic-moisture-diffusion.yaml) is the
first `0.1.0` drying-material document. It declares a reference diffusivity in
m2/s, moisture/multiplier knots, a closed temperature validity interval and
explicit synthetic provenance. The reference diffusivity must agree with the
proof profile. Density and mechanical properties remain separate profile inputs.
An optional dry-solid heat-capacity table must match the profile's reference
capacity at its initial temperature. This is a partial constitutive contract,
not a complete body formulation. Public-measurement provenance requires a valid HTTPS URL without
embedded credentials and a data locator. The compiler does not fetch that URL,
certify the data, confirm public availability or validate the model.
The synthetic case restricts its temperature domain to 273.15-350 K and uses an
80-second horizon with surface transfer of 3e-6 m/s. This avoids passing through
an unmodeled freezing regime; freeze/thaw and phase-change storage remain absent.
Conservation-ledger tests use bounds derived from the cell equation tolerances
and accumulated step count, not thresholds fitted to the recorded result.

From the repository root:

```sh
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics material project/physics/materials/synthetic-moisture-diffusion.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/variable-drying.yaml --material project/physics/materials/synthetic-moisture-diffusion.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics replay project/physics/replays/variable-drying.json
```

`physics material` computes a formatting-independent typed-document reference;
`physics prove` and `physics budget` perform semantic and reference checks.
The replay embeds the material document and verifies its digest before execution.
Changing the law requires a new reference and reviewed replay evidence. The new
law includes half-cell moisture resistance, unlike the retained legacy proofs.
This distinction is included in the compiled model contract.

Each checkpoint records accepted fields, conservation ledgers and nonlinear
iteration count, linear sweep count and final true residual. Iterative scratch
is history-free and rebuilt from committed state. Invalid material domains,
nonconvergence and failed thermal/mechanical trials cannot partially commit.
Restart, shared-workspace independence, archive capture, four pacing settings
and zero-allocation steps have local tests. The native four-platform matrix must
still run before identical cross-platform reexecution is qualified.

## Public Basis And Remaining Work

The spatial formulation follows the public [NIST finite-volume description](https://pages.nist.gov/fipy/en/3.4.5/numerical/discret.html).
The harmonic interface treatment is consistent with the diffusion options in
[INL's finite-volume implementation](https://mooseframework.inl.gov/source/fvkernels/FVDiffusion.html).
These references support numerical methods, not this project's material inputs.

The [ceramic drying study](https://www.sciencedirect.com/science/article/pii/S0955221921005124)
identifies moisture-dependent properties as necessary model inputs. Its abstract
does not supply enough data for calibration here; no values have been inferred
from it. Experimental comparisons and property uncertainty remain required.

Next work requires public measured property data, independent experimental
comparisons, uncertainty analysis, complete multiphase energy, water activity and coupled
constitutive laws, followed by canonical compilation/draft integration and a
reviewed factory resource study. Existing proofs are preserved as separate
evidence rather than overwritten. Numerical verification and material validation
remain distinct under the [NIST V&V framework](https://nvlpubs.nist.gov/nistpubs/ir/2020/NIST.IR.8298.pdf).

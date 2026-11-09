# Dry-Solid Enthalpy Verification

Status: **Conditionally verified** for synthetic analytical and manufactured
cases, not validated against a ceramic material. The optional caloric law is
available to standalone drying proofs and independent pooled specimens. It is
not yet part of the canonical factory model.

## Storage And Discretization

Specific heat capacity `cp(T)` has units J/(kg K), specific enthalpy `h(T)` has
units J/kg, and each cell carries constant dry mass `m` in kg. The material
compiler integrates a positive piecewise-linear capacity table exactly before
fixed-point rounding. Within a segment, its primitive is quadratic:

```text
h(T) = h(T0) + cp0*(T-T0) + (cp1-cp0)*(T-T0)^2/(2*(T1-T0))
m*(h(T_new)-h(T_old)) = dt*(sum G*(T_neighbor-T_new) + source)
```

Here `G` is W/K and `source` is W. Using `cp(T_new)*(T_new-T_old)` instead of
the enthalpy difference would not conserve the declared stored energy.
The reference enthalpy cancels in the local equation. A single rounded
primitive, including compiled knot values, is used by both the solver and
inventory ledger; no independently rounded integral is substituted.

The implicit finite-volume operator uses deterministic nonlinear Gauss-Seidel
sweeps. Each scalar row uses bracketed Newton iteration over the declared
temperature interval, falling back to bisection. Acceptance requires the true
cell energy residual, not a small temperature update. Iteration limits,
out-of-domain energy demand, arithmetic overflow and insufficient quantized
resolution fail explicitly without publishing either coupled field. Very
large variable-property scales can exceed the checked wide intermediate;
rescaling is required, not dropping the quadratic term.

Interior face conductances use harmonic resistance. In `enthalpy-drying`,
external film and half-cell thermal conduction are in series, including basin
cavity faces. The earlier constant-capacity fixtures retain their original
lumped thermal boundary convention and unchanged replay artifacts. The new
convention is included in the compiled model digest.

## Configuration And Runtime

[`synthetic-drying-enthalpy.yaml`](synthetic-drying-enthalpy.yaml) specifies
800, 900 and 1,100 J/(kg K) at 273.15, 300 and 350 K. These are deliberately
synthetic inputs, not claimed measurements. Its temperature endpoints must
equal the validity interval, capacities must be positive, and capacity at the
initial temperature must match the proof profile. No extrapolation is allowed.
Both the table and explicit `caloric_solver` limits must be present; a partial
configuration is rejected rather than silently reverting to constant capacity.

The law is an additive optional field in the initial, unreleased `0.1.0`
material schema. Documents without it retain their previous normalized bytes
and digests. Compiled models include the caloric and enthalpy numerical contracts.
The new replay is a separate artifact; no earlier golden fixture was replaced.

The thermal solve reuses moisture-stage trial and linear scratch after that
stage completes. The 27-cell fixture still allocates **3,456 field-buffer bytes**.
Four diffusion knots plus three caloric knots consume **136 immutable bytes**;
caloric compilation needs **48 temporary bytes** for parsed knot pairs.
These are numerical payloads, not total application memory or parsed YAML size.
Successful stepping and capture allocate nothing after construction.

Checkpoints include thermal sweeps, total local iterations, final cell energy
residual and the signed discrepancy between integrated latent sources and
latent heat times the rounded water exchange. The two product orders can differ
at fixed-point resolution; the diagnostic does not silently force agreement.
The accumulated energy conservation ledger remains separately visible.

## Verification Evidence

- Independent polynomial integration, continuity at knots, increasing enthalpy
  with decreasing positive capacity, and reference-enthalpy invariance.
- Unit mass with `cp=1+T` stores 1.5 energy units when heated from 0 to 1;
  closed equal-mass volumes initially at 0 and 2 equilibrate to `sqrt(5)-1`,
  not the arithmetic temperature average.
- Endpoint forcing `q=1+t` converges to the independently integrated temperature
  as the timestep shrinks. Constant-capacity heterogeneous transfer recovers
  its analytic backward-Euler limit.
- The 3D manufactured field `16*x*(1-x)*y*(1-y)*z*(1-z)` is tested on 2, 4 and
  8 cells per direction with independently differentiated forcing. The test
  checks reducing spatial error on a 1 MiB worker stack.
- Host tests check a one-cell film/half-cell series resistance, failed-step
  rollback, sparse basin/mechanical coupling, strict checkpoint validation,
  pacing invariance, no step allocations and independent streamed restart.
- The native four-platform CI matrix includes the new golden artifact and
  these numerical tests. Local Linux results alone do not qualify that matrix.

## Physical Limits And References

This retained proof stores **dry-solid sensible energy only**. It does not account
for liquid sensible heat, transported sensible enthalpy, evolving composition,
freezing, bound-water thermodynamics, radiation or thermoelastic heat. Latent
surface cooling remains the existing partial coupling. It must not be described
as a complete wet-body energy model or used for real defect predictions.
The separate [wet-energy extension](wet-energy.md) now includes liquid sensible
storage and transport without changing this artifact's equations or results.

The distinction between constant capacity and a material enthalpy relation is
also explicit in [INL's finite-volume enthalpy material](https://mooseframework.inl.gov/source/functormaterials/INSFVEnthalpyFunctorMaterial.html).
Its [heat-transfer formulation](https://mooseframework.inl.gov/source/physics/WCNSLinearFVFluidHeatTransferPhysics.html)
provides a public reference for energy transport; neither source validates the
synthetic properties or this implementation. Complete multiphase conservation,
experimental comparisons, uncertainty analysis and the factory memory gate
remain required before canonical migration.

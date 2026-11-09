# Wet Sensible-Energy Verification

Status: **Conditionally verified** for synthetic analytical and manufactured
cases, not physically validated. This extension runs in standalone proofs and
independent pooled specimens. It does not migrate the canonical factory model.
The retained [dry-solid proof](caloric.md) remains separate evidence.

## Energy And Mass Contracts

Each finite volume has fixed dry mass `md` (kg), moisture `w` (kg liquid/kg dry
solid), temperature `T` (K), and liquid mass `ml = md*w` (kg). Both phases share
the local temperature. Positive tabulated capacities are integrated into
specific sensible enthalpies `hd` and `hl` (J/kg):

```text
E = md*hd(T) + ml*hl(T)
E_new - E_old = dt*sum(thermal face fluxes)
              + sum(signed transferred liquid mass * hl(T_upstream_new))
              + dt*latent boundary source
```

Thermal conductance is W/K, liquid conductance is kg/s per unit moisture,
and time is seconds. Signed liquid transfer is positive into a cell. The
completed moisture solve supplies its actual accepted fields, conductances
and boundary coefficients to the thermal solve; no nominal flow is substituted.
Interior transfers use upstream temperature at the new time. Outgoing energy
is implicit in the local row; incoming energy uses the neighbor's current
iterate. Both rows round each face transfer identically with opposite signs.
Lumping outgoing masses before multiplying by enthalpy would break that exact
face cancellation at fixed-point resolution and is deliberately avoided.

Boundary sensible energy uses local temperature for outflow and the declared
ambient temperature for inflow. The existing latent source remains separate:
`L * boundary mass rate`, with constant `L` in J/kg. Evaporation therefore removes
liquid sensible energy plus latent energy; condensation adds both. This is an
effective boundary-vapor enthalpy approximation, not a solved vapor phase.

Storage and flux use the same rounded caloric primitive and common reference.
Positive liquid inventory that rounds to zero mass is rejected. The reported
water and energy residuals must both be reviewed: a finite mass residual can
produce a reference-dependent energy discrepancy. The kernel accepts an
already completed moisture stage; callers cannot supply arbitrary old/new
inventories and claim a coupled conservation result. The host uses its checked
nonlinear moisture solver and retains both conservation ledgers.

The thermal operator uses bounded nonlinear Gauss-Seidel with bracketed scalar
root solves and true energy-residual acceptance. Failure rolls back the coupled
trial. The mixture is stationary and approximately incompressible, with no
pressure-work contribution. Moisture transport is not yet temperature-dependent;
this sequential coupling is not a fully coupled multiphase solver.

## Configuration And Storage

[`synthetic-wet-drying.yaml`](synthetic-wet-drying.yaml) adds
`liquid_heat_capacity_j_kg_k_by_temperature` alongside the dry-solid caloric
table. A liquid table without the dry table and caloric solver is rejected.
Both tables must span the same declared temperature interval and remain
positive. No extrapolation is allowed. The liquid values 4,100, 4,180 and
4,300 J/(kg K) at 273.15, 300 and 350 K are synthetic test inputs, not measured
water properties or a calibrated body formulation.

The formatting-independent material digest is
`8a86e1765f553c92e05c647a24ee91f454dfa094efec9057d9f34511b3e356c6`.
The compiled model includes
`hearthline-wet-enthalpy/1:solid-liquid-storage:implicit-upwind-sensible-flux`.
Documents without the optional liquid table retain their old normalized bytes.
The initial `0.1.0` material schema is still unreleased.

The 27-cell wet proof uses **3,456 field-buffer bytes**, **208 immutable material
bytes**, and **48 temporary caloric-compilation bytes**. It reuses existing
thermal/moisture scratch; no new runtime field or checkpoint array is added.
Liquid energy is derived from independently stored moisture and temperature
and the immutable law. Wet energy itself leaves pooled metadata unchanged.
Population storage contract 5 subsequently adds opt-in zero-start elasticity;
it does not remove any wet-energy state.
These counts are numerical payloads, not complete application memory.
The original factory capacity baseline retains its constant-law inputs. Separate
[source-backed studies](../capacity/source-backed.md) now explicitly charge this
law in basin profiles; the first previous-step refined geometry fails the
reviewed budget. A [separate reconstructed study](../populations/reconstruction.md)
reduces elastic storage, not the moisture or thermal model.

From the repository root:

```sh
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics material project/physics/materials/synthetic-wet-drying.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics prove project/physics/proofs/wet-drying.yaml --material project/physics/materials/synthetic-wet-drying.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics replay project/physics/replays/wet-drying.json
```

## Numerical Evidence

- Two isolated dry masses with transferred liquid balance total sensible energy
  while heating the receiving cell; a successful step allocates nothing.
- A separate quantized face test checks exact energy antisymmetry when summing
  transferred masses before enthalpy multiplication would round differently.
- Independent one-cell evaporation and condensation balances check the signs
  and amounts of storage, boundary sensible flux and latent heat.
- The normalized continuous case `w'=-w`, `E=(1+2*w)*T`, `L=4`, initially
  `w=1, T=10`, has `T(1)=8.9056648505`. Timesteps from 1 to 1/64 approach this
  independently integrated result; no replay digest is used as its oracle.
- A 3D manufactured temperature field on 2, 4 and 8 cells per direction checks
  spatial convergence with stationary wet storage on a 1 MiB worker stack.
- Host tests cover sparse basin mechanics, independent instances, streamed
  restart, pacing invariance, strict material-digest checks, invalid domains,
  failed-step rollback and post-construction allocation counts.
- The eighth golden proof has final digest
  `e1d4685d0586e30f4a36867041bd4b5d04922c041ada1bd2c63af77e00cf9a07`.
  All seven older proof artifacts remain unchanged. The native four-platform
  CI matrix includes this case; local results alone do not qualify that matrix.

## Public Basis And Limits

[INL's porous-flow governing equations](https://mooseframework.inl.gov/releases/moose/2024-11-11/modules/porous_flow/governing_equations.html)
distinguish solid/fluid energy storage from conductive and fluid-enthalpy flux
under local thermal equilibrium. That is a methodological reference, not a
validation of this simplified law or its synthetic inputs.

Independent vapor inventory and transport, pressure-dependent phase energy,
sorption, water activity, freezing, composition, shrinkage feedback, radiation,
and thermoelastic heat remain absent. Material experiments, uncertainty analysis,
full-factory resource qualification and canonical integration are still required.
This proof cannot establish real drying time, crack formation or defect risk.

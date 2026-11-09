# Source-Backed Capacity Studies

Status: **Conditionally verified** numerical storage accounting, not a qualified
production mesh or full-application memory measurement. The factory memory gate
is still open: the first basin refinement fails the reviewed budget. The
separate [zero-start reconstruction study](../populations/reconstruction.md)
addresses that implemented-field storage problem without replacing this
rejection evidence or qualifying the remaining release physics.

## Sources And Identity

A capacity profile may declare `model: {id, sha256}` referencing a typed numerical
proof document. The command receives proof, geometry and material files explicitly;
paths are not part of source identity. Referenced geometry and material documents
retain their own digests. The profile's bounding `shape` and `kernel` must agree
with the proof. Actual active cells, tetrahedra, sparse matrix, nonlinear buffers
and material tables come from the same compiler used by the runtime.

`physics proof-reference <proof.yaml>` computes the typed source reference. It
does not prove numerical validity. Capacity compilation separately validates
the complete proof and records its compiled model digest, including numerical
contracts. YAML key order, comments and source collection order do not change
identity. Changing a material, geometry, solver or model reference changes the
reviewed evidence. Missing, duplicate, unused, stale or incompatible sources are
errors; a digest match cannot bypass semantic validation.

The CLI bounds each document and the combined additional source documents to
16 MiB. The library accepts typed documents and performs no filesystem lookup.
Capacity contract **3** adds source-backed layouts, material storage and compilation
peaks. It changes report digests, not physical replay results. Population
storage contract 5 separately adds opt-in reconstructible elastic displacement;
the default previous-step layouts below remain unchanged. Existing `0.1.0`
study documents without a `model` remain valid;
this initial schema is still unreleased.

## Resource Results

Both new studies retain the block baseline's 21 populations, 3,095 reserved
slots, 2,778 product slots, production/residence assumptions, overflow policies,
auxiliary envelopes and 25% reserve. Only the product profile changes.

| Resource | Coarse basin | Refined basin |
| --- | ---: | ---: |
| Bounding lattice | 24 x 18 x 9 | 48 x 36 x 18 |
| Active material volumes per piece | 1,072 | 8,576 |
| Mechanical nodes per piece | 2,230 | 13,119 |
| Tetrahedra per piece | 6,432 | 51,456 |
| All independent state banks, bytes | 203,724,192 | 1,263,253,392 |
| Slot metadata, bytes | 817,080 | 817,080 |
| Shared profile workers, bytes | 3,690,128 | 11,685,160 |
| Shared immutable geometry, bytes | 3,388,748 | 23,642,292 |
| Shared material tables, bytes | 208 | 208 |
| Largest serial archive decoder, bytes | 11,399,168 | 72,363,566 |
| Declared auxiliary envelopes, bytes | 184,549,376 | 184,549,376 |
| Pooled runtime plan, bytes | **407,568,900** | **1,556,311,074** |
| Compilation peak plan, bytes | 210,176,244 | 386,076,288 |
| Reviewed usable budget, bytes | 1,207,959,552 | 1,207,959,552 |
| Pooled result | Fits implemented fields | **Rejected** |

The coarse result is about 389 MiB. The refined result is about 1.45 GiB: below
the gross 1.5 GiB ceiling but above the 1.125 GiB usable limit after reserve.
Its product state bank alone is already over that usable limit. No smaller
population, shorter residence, coarser replacement mesh or larger budget is
substituted when rejection occurs.

The coarse basin has one-cell-thick walls/floor; the refinement has two. Neither
is a demonstrated converged production mesh. This comparison is resource
sensitivity, not a spatial-convergence result or an optimization benchmark.
The product laws are synthetic wet drying plus constant linear elasticity.
Assigning these array layouts to kiln-resident slots does not implement firing
physics or extend the material's declared temperature validity interval.
Damage, coating, chemistry, turbulence and several other release fields remain
unbudgeted. The unchanged independent-workspace strategies require approximately
5.53 GiB and 39.39 GiB respectively and both fail.

## Peak And Allocation Checks

Profiles are compiled serially before their mutable field banks are constructed.
The report charges all retained immutable models, the largest compiler workspace
and the declared auxiliary envelope for the compilation stage. It separately
charges all state banks, workers and the largest bounded decoder for runtime.
The larger stage is compared with the reviewed limit. A fitting runtime with an
oversized compiler stage is rejected by both reporting strategies.

The current capacity adapter compiles each referenced immutable mesh to obtain
its actual sparse layout; it never constructs the reported factory's mutable
field banks. Factory-worker integration must preserve the staged loading contract.
Concurrent compilation, old/new model coexistence during drafts, multiple active
sessions and full browser/server RSS require separate aggregate budgeting.

The coarse product bank is also actually constructed in a 1 MiB-stack test:
2,778 independent slots request **198,348,032 bytes**, matching the allocation
counter. This excludes its separately compiled immutable model. Selected slots
are stepped and restored independently without new step allocations. That is
not a full-population residence-cycle soak. The refined bank is rejected before
its arrays are allocated. Native CI runs these tests; local Linux results do
not establish four-platform qualification.

## Reproduce

From the repository root:

```sh
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics proof-reference project/physics/proofs/basin-wet-drying.yaml
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics capacity project/physics/capacity/factory-basin-study.yaml --proof project/physics/proofs/basin-wet-drying.yaml --geometry project/physics/geometry/original-basin.yaml --material project/physics/materials/synthetic-wet-drying.yaml --pooled --require-fit
cargo run --manifest-path packages/Cargo.toml --bin hearthline -- physics capacity project/physics/capacity/factory-basin-refinement.yaml --proof project/physics/proofs/basin-wet-refined.yaml --geometry project/physics/geometry/original-basin.yaml --material project/physics/materials/synthetic-wet-drying.yaml --pooled --require-fit
```

The final command must currently fail after printing its report. Omit
`--require-fit` only to collect informational evidence, not to accept the model.
CI retains both study reports and tests the expected refinement rejection.
Further storage work must preserve physical fields, fidelity, workload and
exact continuation; validated geometry and constitutive fields must then be
included before the canonical migration gate can pass.

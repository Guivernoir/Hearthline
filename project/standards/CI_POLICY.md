# Continuous Integration Policy

Hearthline treats its repository constraints as build requirements. Pull
requests and branch updates must pass the same checks available locally through
`node project/scripts/repository-policy.mjs`.

## Repository Shape

- Repository-owned text files are limited to 500 physical lines.
- Every checked folder is limited to seven direct files or subfolders.
- Build output, dependency installations, and version-control internals are not
  part of the repository tree.
- Package-manager lockfiles are generated dependency metadata and are excluded
  only from the line limit. They still count toward folder fan-out.
- Binary assets are excluded only from the line limit.

## Rust Boundaries

`hearthline-model` and `hearthline-engine` are the deterministic runtime. They
must compile without `std`, must not import `alloc`, and must use fixed-capacity
storage. Filesystem access, YAML deserialization, HTTP handling, and command-line
presentation belong to host adapter crates and are not linked into the runtime.

Tests must live in a crate's `tests/` folder. Production source files cannot
contain test modules or test functions.

## Required Verification

CI runs repository and version policy checks, Rust formatting and linting, all
Rust tests, canonical configuration validation, generated-catalog drift
detection, Svelte type and build checks, bounded fuzz campaigns, and
release-mode benchmarks. A required suite cannot be replaced by a
presence-only check.

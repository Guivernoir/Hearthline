# Versioning And Releases

Hearthline follows Semantic Versioning using `MAJOR.MINOR.PATCH`.

The current development release is `0.3.1`. The repository-root
[`VERSION`](../../VERSION) file is the human-readable authority for the release
number. The same value must be synchronized into the Rust workspace and Svelte
package metadata.

## Compatibility Policy

- `MAJOR` identifies an incompatible stable application, API, or simulation
  contract after Hearthline reaches `1.0.0`.
- `MINOR` identifies a development milestone or backward-compatible stable
  capability.
- `PATCH` identifies compatible fixes, documentation corrections, and
  nonfunctional maintenance.

While the project remains below `1.0.0`, a minor release may include breaking
changes. Every such change must be identified in
[`CHANGELOG.md`](../../../CHANGELOG.md), include required migrations, and update
affected generated data and documentation.

Prereleases use SemVer suffixes such as `0.3.0-alpha.1`. Build metadata may be
used for local artifacts but does not change compatibility.

## Versioned Surfaces

| Surface | Current version | Authority |
| --- | --- | --- |
| Hearthline application release | `0.3.1` | `project/VERSION` |
| Rust workspace packages | `0.3.1` | `packages/Cargo.toml` workspace package |
| Svelte package | `0.3.1` | `packages/web/package.json` |
| Appliance YAML schema | `0.10.0` | `APPLIANCE_SCHEMA_VERSION` |
| Connection YAML schema | `0.2.0` | `CONNECTION_SCHEMA_VERSION` |
| Generated appliance catalog schema | `0.9.0` | `FRONTEND_CATALOG_SCHEMA_VERSION` |
| Bootstrap process view schema | `0.2.0` | `packages/web/src/generated/process-view.json` |
| Scenario YAML schema | `0.12.0` | `SCENARIO_SCHEMA_VERSION` |
| Scenario report schema | `0.15.0` | `SCENARIO_REPORT_SCHEMA_VERSION` |
| Workstation API schema | `0.10.0` | `WORKSTATION_SCHEMA_VERSION` |
| HMI API schema | `0.8.0` | `HMI_SCHEMA_VERSION` |
| Security-console session schema | `0.1.0` | `SECURITY_CONSOLE_SCHEMA_VERSION` |

Application and schema versions are independent. A release may change no
schemas, one schema, or several schemas. Schema changes are validated by their
Rust parser and must not be inferred from the application release number.

## Release Procedure

1. Select the next SemVer value and update `project/VERSION`.
2. Synchronize the Rust workspace, Cargo lockfile, npm package, and npm
   lockfile.
3. Run `node project/scripts/check-version.mjs` and resolve every mismatch.
4. Move completed entries from `Unreleased` into a dated changelog section.
5. Run repository policy, standalone `no_std` checks, Rust formatting, tests,
   strict Clippy, configuration validation, deterministic catalog generation,
   bounded fuzzing, and benchmarks.
6. Run Svelte diagnostics, the production build, and desktop/mobile route
   checks.
7. Refresh documentation screenshots for every changed route.
8. Commit the complete release state and create an annotated `vMAJOR.MINOR.PATCH`
   tag.

Release tags must identify a commit that contains matching metadata,
documentation, generated files, and changelog content.

# Generated Frontend Data

Files in this directory are derivative inputs for the Svelte application. They
must never become the source of truth for network, policy, controller, I/O, or
process behavior.

`process-view.json` is currently marked `bootstrap` because it was created
before the Rust generator and canonical YAML model. It establishes the first
versioned frontend contract and removes process inventory from Svelte
components. When the Rust pipeline exists, the file will be regenerated from
validated YAML and IEC 61131-3 references and its status will change to
`generated`.

Svelte may use presentation-only coordinates and interaction state. It must not
invent missing devices, links, policy decisions, simulation values, or parser
results.

Schema `0.2.0` distinguishes the physical vPLC host cluster from logical
area-controller workloads and includes a local distributed-I/O station in every
process area.

## Current and Planned State

At present, `process-view.json` is the only bootstrap view model in this
directory. It is manually maintained and validated only by the frontend schema
guard and build checks. Future Rust generation must add source validation,
cross-reference diagnostics, deterministic output, and an atomic replacement
step before the file can be labeled `generated`.

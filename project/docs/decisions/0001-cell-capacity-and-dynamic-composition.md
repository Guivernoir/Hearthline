# ADR 0001: Cell Capacity and Dynamic Composition

- Status: Accepted
- Decision date: 2026-08-27
- Owner: Hearthline maintainers
- Review date: 2026-11-27

## Context

Early runtime growth was handled by increasing shared fixed-capacity constants.
That made unrelated sites compete for one global ceiling and turned new process
areas into pressure to enlarge every simulation object. It also obscured the
memory, queue, and stack effect of a model change.

## Decision

Projects compose an unbounded host-side collection of sites and cells during
model loading. Each cell executes with the engine's fixed structural limits.
Cross-cell and cross-site traffic uses separately preallocated, logically
bounded conduits. The sealed single-threaded scheduler orders envelopes by
time, destination site, destination cell, conduit, and sequence.

The compiler rejects a cell that exceeds its component or internal-link
budget. The model must partition that area instead of raising a project-wide
factory, site, or cell count. Every compiled boundary receives two directional
queue assessments, an explicit saturation policy, workload evidence, and at
least 25% reviewed reserve.

The compiler also records the largest registered runtime object, estimated
loader stack, aggregate sealed-session memory, event envelopes, and normalized
partition assignments in `model.lock.json`. Construction runs on a dedicated
2 MiB loader stack; constrained caller-stack tests prevent recurrence of the
previous HMI construction overflow.

## Current Structural Limits

Engine limits remain local to one cell or one component family. Their source
of truth is `hearthline-engine/src/capacity.rs`; compiler demand and policy are
recorded in `project/config/runtime/capacity.yaml`. The current canonical model
and generated acceptance model both compile with at least 25% reserve.

Overflow is never implicit. Resources use reject, backpressure, coalescing,
drop-oldest, stop-simulation, or explicit recovery semantics according to
their contract. Saturation tests cover each conduit policy.

## Consequences

- Project growth adds cells and conduits without resizing every runtime.
- A cell that becomes too large must be split along a reviewed process or
  control boundary.
- Model compilation performs more host-side allocation, but sealed execution
  remains allocator-free.
- Every structural-limit change requires this ADR process, updated lock and
  capacity reports, saturation/recovery tests, and benchmark plus stack/object
  evidence.

## Rejected Alternatives

- One global partition coordinator: unrelated model growth consumes shared
  capacity and imposes a project-wide ceiling.
- Automatically growing queues after seal: this breaks deterministic memory
  bounds and makes overload behavior platform-dependent.
- Silently dropping at every boundary: this hides invalid design loads and is
  unsuitable for control and safety traffic.

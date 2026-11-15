# Previous Connected Snapshots

These schema `0.1.0` snapshots were captured before adding connected-flow retries
and schema `0.2.0`. They retain the initial, eighth-step, final and failed states
of the original 24-cell synthetic case described in [connections](../connections.md).
Their footer digests are pinned by [the original golden fixture](../connected-golden.json).

Tests read the original bytes, verify integrity and model identity, migrate in
memory, and compare complete fields and diagnostics with native recomputation.
Writers emit only the current schema. These files must not be regenerated to
accommodate changed equations or control outcomes. They are numerical regression
evidence, not a calibrated air system or a canonical factory model.

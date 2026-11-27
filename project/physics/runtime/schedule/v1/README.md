# Previous Schedule Evidence

These snapshots were produced by the unmodified `0.1.0` scheduler writer before
introducing typed, length-delimited children in `0.2.0`. The initial, fourth-event
and final records correspond to [the original nominal hashes](../previous-golden.json).
The failed record corresponds to [the original failure hash](../previous-failure-golden.json).
Their bytes and numerical meaning are retained, not regenerated using the new writer.

Migration tests check each complete archive's original SHA-256, load it with the
current reader, and compare every checkpoint, failure and frontier against fresh
execution. Rewriting produces current framing and therefore a new aggregate
digest. This format migration does not change the previous cases' timesteps,
physical coefficients, mesh, accepted fields or numerical tolerances.

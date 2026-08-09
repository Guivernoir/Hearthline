# Hearthline Control Sources

This directory contains executable control-source contracts used by the Hearthline simulator. Each supported controller references one program under `programs/` and one explicit process-I/O map under `bindings/` from its appliance YAML.

The current Structured Text implementation is a deliberately bounded Hearthline subset. It supports deterministic sequence steps, integer output-state assignments, `TON` delays, start/reset conditions, and fault-state handling. It is not presented as a complete IEC 61131-3 runtime or as a substitute for vendor engineering and safety-validation tools.

Rust owns plant dynamics, signal evolution, independent fault detection, and runtime bounds. Structured Text owns the normal Forming sequence and requested actuator states. YAML binds program variables to simulated field tags and validates those states against the appliance configuration.

Only the Forming cell has an executable source contract at this stage. References used by the other process-area placeholders remain non-executable until their plant models and I/O contracts are implemented.

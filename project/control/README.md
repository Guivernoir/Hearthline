# Hearthline Control Sources

This directory contains executable control-source contracts used by the Hearthline simulator. Each supported controller references one program under `programs/` and one explicit process-I/O map under `bindings/` from its appliance YAML.

The current Structured Text implementation is a deliberately bounded Hearthline subset. It supports deterministic sequence steps, integer output-state assignments, `TON` delays, start/reset conditions, and fault-state handling. The Forming robot also references a bounded `.g` motion source supporting the explicitly documented movement, dwell, positioning, gripper, and termination words. Neither parser is presented as a complete controller runtime or a substitute for engineering and safety-validation tools.

Rust owns plant dynamics, signal evolution, independent fault detection, robot motion interpolation, source parsing, and runtime bounds. Structured Text owns the normal Forming sequence and requested actuator states. The Forming `.g` file is the authoritative executable source for automatic robot movement and gripper sequencing; it contains one complete routine for each mould and its assigned operator handoff. YAML binds program variables to simulated field tags and defines robot limits, physical reference geometry, tolerances, mould-to-routine assignments, and the source reference. Rust compares the pose reached by the `.g` routine with that configured geometry and faults the cell when a pickup or handoff command is outside tolerance.

Only the Forming cell has an executable source contract at this stage. References used by the other process-area placeholders remain non-executable until their plant models and I/O contracts are implemented.

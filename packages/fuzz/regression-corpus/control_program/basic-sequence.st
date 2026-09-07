PROGRAM FuzzSequence
VAR_INPUT
    StartCycle : BOOL;
    SafetyReady : BOOL;
END_VAR
VAR_OUTPUT
    PhaseCode : DINT;
END_VAR
VAR
    Step : DINT := 0;
    StepTimer : TON;
END_VAR
CASE Step OF
0:
    PhaseCode := 0;
    IF StartCycle AND SafetyReady THEN Step := 10; END_IF;
10:
    PhaseCode := 10;
    StepTimer(IN := TRUE, PT := T#100ms);
    IF StepTimer.Q THEN Step := 0; END_IF;
END_CASE;
END_PROGRAM

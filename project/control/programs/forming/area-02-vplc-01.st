PROGRAM FormingSequence
VAR_INPUT
    StartCycle : BOOL;
    SafetyReady : BOOL;
    ResetCycle : BOOL;
    TripActive : BOOL;
    SlipTankLevel : REAL;
    SlipDensity : REAL;
    SlipViscosity : REAL;
    SlipTemperature : REAL;
    SlipFeedFlow : REAL;
    SlipFeedPressure : REAL;
    MouldPressure : REAL;
    MouldTemperature : REAL;
    FillHeadPosition : REAL;
    MouldPosition : REAL;
    WaterFlow : REAL;
    ExcessSlipDrainFlow : REAL;
    MouldMoisture : REAL;
    CompressedAirPressure : REAL;
    VacuumPressure : REAL;
    RobotPosition : REAL;
    PieceGripped : BOOL;
END_VAR
VAR_OUTPUT
    PhaseCode : DINT;
    SlipCommand : INT;
    MouldCommand : INT;
    WaterCommand : INT;
    AirCommand : INT;
    VacuumCommand : INT;
    RobotCommand : INT;
END_VAR
VAR
    Step : DINT := 0;
    StepTimer : TON;
END_VAR

CASE Step OF
0:
    PhaseCode := 0;
    SlipCommand := 1;
    MouldCommand := 1;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 1;
    IF StartCycle AND SafetyReady THEN Step := 10; END_IF;
10:
    PhaseCode := 10;
    SlipCommand := 2;
    MouldCommand := 1;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#1500ms);
    IF StepTimer.Q THEN Step := 20; END_IF;
20:
    PhaseCode := 20;
    SlipCommand := 0;
    MouldCommand := 1;
    WaterCommand := 0;
    AirCommand := 1;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#750ms);
    IF StepTimer.Q THEN Step := 30; END_IF;
30:
    PhaseCode := 30;
    SlipCommand := 0;
    MouldCommand := 1;
    WaterCommand := 0;
    AirCommand := 1;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#2500ms);
    IF StepTimer.Q THEN Step := 40; END_IF;
40:
    PhaseCode := 40;
    SlipCommand := 0;
    MouldCommand := 1;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#500ms);
    IF StepTimer.Q THEN Step := 60; END_IF;
50:
    PhaseCode := 50;
    SlipCommand := 3;
    MouldCommand := 1;
    WaterCommand := 0;
    AirCommand := 1;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#1000ms);
    IF StepTimer.Q THEN Step := 50; END_IF;
60:
    PhaseCode := 60;
    SlipCommand := 0;
    MouldCommand := 1;
    WaterCommand := 1;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#400ms);
    IF StepTimer.Q THEN Step := 70; END_IF;
70:
    PhaseCode := 70;
    SlipCommand := 0;
    MouldCommand := 1;
    WaterCommand := 0;
    AirCommand := 2;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#400ms);
    IF StepTimer.Q THEN Step := 80; END_IF;
80:
    PhaseCode := 80;
    SlipCommand := 0;
    MouldCommand := 2;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#750ms);
    IF StepTimer.Q THEN Step := 90; END_IF;
90:
    PhaseCode := 90;
    SlipCommand := 0;
    MouldCommand := 3;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 3;
    StepTimer(IN := TRUE, PT := T#1000ms);
    IF StepTimer.Q THEN Step := 100; END_IF;
100:
    PhaseCode := 100;
    SlipCommand := 0;
    MouldCommand := 3;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 4;
    StepTimer(IN := TRUE, PT := T#1200ms);
    IF StepTimer.Q THEN Step := 110; END_IF;
110:
    PhaseCode := 110;
    SlipCommand := 0;
    MouldCommand := 3;
    WaterCommand := 2;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 6;
    StepTimer(IN := TRUE, PT := T#1000ms);
    IF StepTimer.Q THEN Step := 120; END_IF;
120:
    PhaseCode := 120;
    SlipCommand := 0;
    MouldCommand := 3;
    WaterCommand := 0;
    AirCommand := 3;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#750ms);
    IF StepTimer.Q THEN Step := 130; END_IF;
130:
    PhaseCode := 130;
    SlipCommand := 0;
    MouldCommand := 3;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 1;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#1500ms);
    IF StepTimer.Q THEN Step := 140; END_IF;
140:
    PhaseCode := 140;
    SlipCommand := 0;
    MouldCommand := 4;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 1;
    StepTimer(IN := TRUE, PT := T#750ms);
    IF StepTimer.Q THEN Step := 0; END_IF;
900:
    PhaseCode := 900;
    SlipCommand := 0;
    MouldCommand := 0;
    WaterCommand := 0;
    AirCommand := 0;
    VacuumCommand := 0;
    RobotCommand := 0;
    IF ResetCycle AND SafetyReady AND NOT TripActive THEN Step := 0; END_IF;
END_CASE;
END_PROGRAM

use hearthline_engine::{
    ROBOT_PROGRAM_CAPACITY, RobotInstruction, RobotMotionKind, RobotPose, RobotProgram,
    RobotProgramLine,
};

use crate::ConfigError;

pub(super) const MAX_ROBOT_SOURCE_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug)]
pub(in crate::hmi) struct ParsedRobotProgram {
    pub(in crate::hmi) name: String,
    pub(in crate::hmi) program: RobotProgram,
    pub(in crate::hmi) routines: Vec<ParsedRobotRoutine>,
    pub(in crate::hmi) source_lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub(in crate::hmi) struct ParsedRobotRoutine {
    pub(in crate::hmi) name: String,
    pub(in crate::hmi) program: RobotProgram,
}

pub(in crate::hmi) fn parse(
    source: &str,
    home: RobotPose,
) -> Result<ParsedRobotProgram, ConfigError> {
    if source.len() > MAX_ROBOT_SOURCE_BYTES {
        return Err(ConfigError::new(format!(
            "robot program exceeds the {MAX_ROBOT_SOURCE_BYTES}-byte source limit"
        )));
    }
    let source_lines = source.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut routines = Vec::new();
    let mut program = RobotProgram::default();
    let mut name = "ROBOT_PROGRAM".to_owned();
    let mut named_routine = false;
    let mut current = home;
    let mut absolute = true;
    for (index, source_line) in source_lines.iter().enumerate() {
        let source_line_number = u16::try_from(index + 1)
            .map_err(|_| ConfigError::new("robot source contains too many lines"))?;
        let cleaned = strip_comments(source_line)?;
        if cleaned.trim().is_empty() || cleaned.trim() == "%" {
            continue;
        }
        let words = words(&cleaned, source_line_number)?;
        if words.is_empty() {
            continue;
        }
        if let Some((_, value)) = words.iter().find(|(letter, _)| *letter == 'O') {
            require_integer_code(*value, 'O', source_line_number)?;
            if *value > 9_999.0 {
                return Err(line_error(
                    source_line_number,
                    "O program number must be between 0000 and 9999".into(),
                ));
            }
            if !program.is_empty() {
                finish_routine(&mut routines, name, program)?;
                program = RobotProgram::default();
            }
            name = format!("O{:04}", *value as u32);
            named_routine = true;
            current = home;
            absolute = true;
        }
        let mut motion_code = None;
        let mut machine_code = None;
        for (letter, value) in &words {
            if *letter == 'G' {
                require_integer_code(*value, 'G', source_line_number)?;
                match *value as i32 {
                    90 => absolute = true,
                    91 => absolute = false,
                    code => motion_code = Some(code),
                }
            } else if *letter == 'M' {
                require_integer_code(*value, 'M', source_line_number)?;
                machine_code = Some(*value as i32);
            }
        }

        if let Some(code) = motion_code {
            let instruction = match code {
                0 | 1 => {
                    let mut target = if absolute {
                        current
                    } else {
                        RobotPose::default()
                    };
                    for (letter, value) in &words {
                        match *letter {
                            'X' => target.x = coordinate(current.x, *value, absolute),
                            'Y' => target.y = coordinate(current.y, *value, absolute),
                            'Z' => target.z = coordinate(current.z, *value, absolute),
                            'A' => target.w = coordinate(current.w, *value, absolute),
                            'B' => target.p = coordinate(current.p, *value, absolute),
                            'C' => target.r = coordinate(current.r, *value, absolute),
                            _ => {}
                        }
                    }
                    let speed_percent = parameter(&words, 'F').unwrap_or(20.0);
                    current = target;
                    RobotInstruction::Move {
                        target,
                        kind: if code == 0 {
                            RobotMotionKind::Rapid
                        } else {
                            RobotMotionKind::Linear
                        },
                        speed_percent,
                    }
                }
                4 => RobotInstruction::Dwell {
                    duration_ms: parameter(&words, 'P').unwrap_or(0.0).max(0.0) as u64,
                },
                28 => {
                    current = home;
                    RobotInstruction::Move {
                        target: home,
                        kind: RobotMotionKind::Rapid,
                        speed_percent: parameter(&words, 'F').unwrap_or(20.0),
                    }
                }
                _ => {
                    return Err(line_error(
                        source_line_number,
                        format!("unsupported G code G{code}"),
                    ));
                }
            };
            ensure_open_routine(&program, source_line_number)?;
            push(&mut program, source_line_number, instruction)?;
        }

        if let Some(code) = machine_code {
            let instruction = match code {
                64 => RobotInstruction::Gripper { closed: true },
                65 => RobotInstruction::Gripper { closed: false },
                30 => RobotInstruction::End,
                _ => {
                    return Err(line_error(
                        source_line_number,
                        format!("unsupported M code M{code}"),
                    ));
                }
            };
            ensure_open_routine(&program, source_line_number)?;
            push(&mut program, source_line_number, instruction)?;
        }
    }
    if program.is_empty() {
        return Err(ConfigError::new(
            "robot program contains no executable instructions",
        ));
    }
    finish_routine(&mut routines, name, program)?;
    if named_routine
        && routines
            .iter()
            .any(|routine| routine.name == "ROBOT_PROGRAM")
    {
        return Err(ConfigError::new(
            "robot source cannot mix unnamed instructions with O programs",
        ));
    }
    let first = routines
        .first()
        .ok_or_else(|| ConfigError::new("robot program contains no routines"))?;
    Ok(ParsedRobotProgram {
        name: first.name.clone(),
        program: first.program.clone(),
        routines,
        source_lines,
    })
}

fn finish_routine(
    routines: &mut Vec<ParsedRobotRoutine>,
    name: String,
    program: RobotProgram,
) -> Result<(), ConfigError> {
    if !matches!(
        program.lines().last().map(|line| line.instruction),
        Some(RobotInstruction::End)
    ) {
        return Err(ConfigError::new(format!(
            "robot routine {name} must terminate with M30"
        )));
    }
    if routines.iter().any(|routine| routine.name == name) {
        return Err(ConfigError::new(format!(
            "robot source repeats routine {name}"
        )));
    }
    routines.push(ParsedRobotRoutine { name, program });
    Ok(())
}

fn ensure_open_routine(program: &RobotProgram, line: u16) -> Result<(), ConfigError> {
    if matches!(
        program.lines().last().map(|entry| entry.instruction),
        Some(RobotInstruction::End)
    ) {
        Err(line_error(
            line,
            "executable instruction follows M30 without a new O program".into(),
        ))
    } else {
        Ok(())
    }
}

fn push(
    program: &mut RobotProgram,
    source_line: u16,
    instruction: RobotInstruction,
) -> Result<(), ConfigError> {
    program
        .push(RobotProgramLine {
            source_line,
            instruction,
        })
        .map_err(|_| {
            ConfigError::new(format!(
                "robot program exceeds the {ROBOT_PROGRAM_CAPACITY}-instruction limit"
            ))
        })
}

fn strip_comments(line: &str) -> Result<String, ConfigError> {
    let before_semicolon = line.split(';').next().unwrap_or_default();
    let mut cleaned = String::with_capacity(before_semicolon.len());
    let mut comment = false;
    for character in before_semicolon.chars() {
        match character {
            '(' if comment => return Err(ConfigError::new("nested robot program comment")),
            '(' => comment = true,
            ')' if comment => comment = false,
            ')' => {
                return Err(ConfigError::new(
                    "unmatched robot program comment terminator",
                ));
            }
            _ if !comment => cleaned.push(character),
            _ => {}
        }
    }
    if comment {
        return Err(ConfigError::new("unterminated robot program comment"));
    }
    Ok(cleaned)
}

fn words(line: &str, source_line: u16) -> Result<Vec<(char, f64)>, ConfigError> {
    let bytes = line.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() || bytes[index] == b'%' {
            index += 1;
            continue;
        }
        if !bytes[index].is_ascii_alphabetic() {
            return Err(line_error(
                source_line,
                format!("expected an address letter near byte {}", index + 1),
            ));
        }
        let letter = (bytes[index] as char).to_ascii_uppercase();
        index += 1;
        let start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_digit()
                || bytes[index] == b'.'
                || bytes[index] == b'-'
                || bytes[index] == b'+')
        {
            index += 1;
        }
        if start == index {
            return Err(line_error(
                source_line,
                format!("address {letter} requires a numeric value"),
            ));
        }
        let value = line[start..index].parse::<f64>().map_err(|_| {
            line_error(
                source_line,
                format!("address {letter} has an invalid number"),
            )
        })?;
        if !value.is_finite() {
            return Err(line_error(
                source_line,
                format!("address {letter} must be finite"),
            ));
        }
        output.push((letter, value));
    }
    Ok(output)
}

fn parameter(words: &[(char, f64)], letter: char) -> Option<f64> {
    words
        .iter()
        .find(|(candidate, _)| *candidate == letter)
        .map(|(_, value)| *value)
}

fn coordinate(current: f64, requested: f64, absolute: bool) -> f64 {
    if absolute {
        requested
    } else {
        current + requested
    }
}

fn require_integer_code(value: f64, letter: char, line: u16) -> Result<(), ConfigError> {
    if value.fract() == 0.0 && value >= 0.0 {
        Ok(())
    } else {
        Err(line_error(
            line,
            format!("address {letter} requires a non-negative integer code"),
        ))
    }
}

fn line_error(line: u16, message: String) -> ConfigError {
    ConfigError::new(format!("robot program line {line}: {message}"))
}

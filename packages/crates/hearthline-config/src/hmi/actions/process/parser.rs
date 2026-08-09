use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::ConfigError;

#[derive(Parser)]
#[grammar = "hmi/actions/process/structured_text.pest"]
struct StructuredTextParser;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VariableSection {
    Input,
    Output,
    Local,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DataType {
    Bool,
    Int,
    Dint,
    Real,
    Ton,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Literal {
    Bool(bool),
    Integer(i64),
    Real(f64),
    Variable(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Variable {
    pub name: String,
    pub section: VariableSection,
    pub data_type: DataType,
    pub initial: Option<Literal>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConditionTerm {
    pub variable: String,
    pub negated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Assignment {
    pub variable: String,
    pub value: Literal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TimerCall {
    pub variable: String,
    pub input: Vec<ConditionTerm>,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ConditionalAssignment {
    pub condition: Vec<ConditionTerm>,
    pub assignment: Assignment,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Branch {
    pub number: i64,
    pub assignments: Vec<Assignment>,
    pub timer: Option<TimerCall>,
    pub transition: Option<ConditionalAssignment>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Program {
    pub name: String,
    pub variables: Vec<Variable>,
    pub case_variable: String,
    pub branches: Vec<Branch>,
}

pub(super) fn parse(source: &str) -> Result<Program, ConfigError> {
    let mut parsed = StructuredTextParser::parse(Rule::program, source)
        .map_err(|error| ConfigError::new(format!("Structured Text syntax error: {error}")))?;
    let program = parsed
        .next()
        .ok_or_else(|| ConfigError::new("Structured Text source contains no PROGRAM"))?;
    parse_program(program)
}

fn parse_program(pair: Pair<'_, Rule>) -> Result<Program, ConfigError> {
    let mut pairs = pair.into_inner();
    let name = text(next_rule(&mut pairs, Rule::identifier, "program name")?);
    let mut variables = Vec::new();
    let mut case = None;
    for pair in pairs {
        match pair.as_rule() {
            Rule::variable_block => variables.extend(parse_variable_block(pair)?),
            Rule::case_statement => case = Some(parse_case(pair)?),
            Rule::EOI => {}
            rule => return Err(unexpected(rule, "PROGRAM")),
        }
    }
    let (case_variable, branches) =
        case.ok_or_else(|| ConfigError::new("Structured Text PROGRAM requires one CASE"))?;
    Ok(Program {
        name,
        variables,
        case_variable,
        branches,
    })
}

fn parse_variable_block(pair: Pair<'_, Rule>) -> Result<Vec<Variable>, ConfigError> {
    let mut pairs = pair.into_inner();
    let section = parse_section(next_rule(
        &mut pairs,
        Rule::variable_section,
        "variable section",
    )?)?;
    pairs
        .map(|pair| {
            if pair.as_rule() != Rule::variable_declaration {
                return Err(unexpected(pair.as_rule(), "variable block"));
            }
            parse_variable(pair, section)
        })
        .collect()
}

fn parse_section(pair: Pair<'_, Rule>) -> Result<VariableSection, ConfigError> {
    match pair.into_inner().next().map(|pair| pair.as_rule()) {
        Some(Rule::var_input_kw) => Ok(VariableSection::Input),
        Some(Rule::var_output_kw) => Ok(VariableSection::Output),
        Some(Rule::var_kw) => Ok(VariableSection::Local),
        rule => Err(ConfigError::new(format!(
            "unsupported Structured Text variable section {rule:?}"
        ))),
    }
}

fn parse_variable(pair: Pair<'_, Rule>, section: VariableSection) -> Result<Variable, ConfigError> {
    let mut pairs = pair.into_inner();
    let name = text(next_rule(&mut pairs, Rule::identifier, "variable name")?);
    let data_type = parse_data_type(next_rule(&mut pairs, Rule::data_type, "variable type")?)?;
    let initial = pairs.next().map(parse_initializer).transpose()?;
    Ok(Variable {
        name,
        section,
        data_type,
        initial,
    })
}

fn parse_data_type(pair: Pair<'_, Rule>) -> Result<DataType, ConfigError> {
    match pair.into_inner().next().map(|pair| pair.as_rule()) {
        Some(Rule::bool_type) => Ok(DataType::Bool),
        Some(Rule::int_type) => Ok(DataType::Int),
        Some(Rule::dint_type) => Ok(DataType::Dint),
        Some(Rule::real_type) => Ok(DataType::Real),
        Some(Rule::ton_type) => Ok(DataType::Ton),
        rule => Err(ConfigError::new(format!(
            "unsupported Structured Text data type {rule:?}"
        ))),
    }
}

fn parse_initializer(pair: Pair<'_, Rule>) -> Result<Literal, ConfigError> {
    parse_literal(next_rule(
        &mut pair.into_inner(),
        Rule::literal,
        "initializer",
    )?)
}

fn parse_case(pair: Pair<'_, Rule>) -> Result<(String, Vec<Branch>), ConfigError> {
    let mut pairs = pair.into_inner();
    let variable = text(next_rule(&mut pairs, Rule::identifier, "CASE variable")?);
    let branches = pairs
        .map(|pair| {
            if pair.as_rule() != Rule::case_branch {
                return Err(unexpected(pair.as_rule(), "CASE"));
            }
            parse_branch(pair)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((variable, branches))
}

fn parse_branch(pair: Pair<'_, Rule>) -> Result<Branch, ConfigError> {
    let mut pairs = pair.into_inner();
    let number = parse_integer(next_rule(&mut pairs, Rule::integer, "CASE branch")?)?;
    let mut branch = Branch {
        number,
        assignments: Vec::new(),
        timer: None,
        transition: None,
    };
    for pair in pairs {
        match pair.as_rule() {
            Rule::assignment => branch.assignments.push(parse_assignment(pair)?),
            Rule::timer_call if branch.timer.is_none() => branch.timer = Some(parse_timer(pair)?),
            Rule::if_statement if branch.transition.is_none() => {
                branch.transition = Some(parse_if(pair)?)
            }
            Rule::timer_call => {
                return Err(ConfigError::new(format!(
                    "CASE branch {number} declares more than one timer call"
                )));
            }
            Rule::if_statement => {
                return Err(ConfigError::new(format!(
                    "CASE branch {number} declares more than one transition"
                )));
            }
            rule => return Err(unexpected(rule, "CASE branch")),
        }
    }
    Ok(branch)
}

fn parse_assignment(pair: Pair<'_, Rule>) -> Result<Assignment, ConfigError> {
    let mut pairs = pair.into_inner();
    let variable = text(next_rule(
        &mut pairs,
        Rule::identifier,
        "assignment variable",
    )?);
    let value = parse_assignment_value(next_rule(
        &mut pairs,
        Rule::assignment_value,
        "assignment value",
    )?)?;
    Ok(Assignment { variable, value })
}

fn parse_assignment_value(pair: Pair<'_, Rule>) -> Result<Literal, ConfigError> {
    let value = pair
        .into_inner()
        .next()
        .ok_or_else(|| ConfigError::new("assignment has no value"))?;
    match value.as_rule() {
        Rule::literal => parse_literal(value),
        Rule::identifier => Ok(Literal::Variable(text(value))),
        rule => Err(unexpected(rule, "assignment")),
    }
}

fn parse_timer(pair: Pair<'_, Rule>) -> Result<TimerCall, ConfigError> {
    let mut pairs = pair.into_inner();
    let variable = text(next_rule(&mut pairs, Rule::identifier, "timer variable")?);
    let input = parse_condition(next_rule(&mut pairs, Rule::bool_expression, "timer input")?)?;
    let duration_ms = parse_duration(next_rule(&mut pairs, Rule::duration, "timer duration")?)?;
    Ok(TimerCall {
        variable,
        input,
        duration_ms,
    })
}

fn parse_if(pair: Pair<'_, Rule>) -> Result<ConditionalAssignment, ConfigError> {
    let mut pairs = pair.into_inner();
    let condition = parse_condition(next_rule(
        &mut pairs,
        Rule::bool_expression,
        "IF condition",
    )?)?;
    let assignment = parse_assignment(next_rule(&mut pairs, Rule::assignment, "IF assignment")?)?;
    Ok(ConditionalAssignment {
        condition,
        assignment,
    })
}

fn parse_condition(pair: Pair<'_, Rule>) -> Result<Vec<ConditionTerm>, ConfigError> {
    pair.into_inner()
        .map(|pair| {
            let mut terms = pair.into_inner();
            let first = terms
                .next()
                .ok_or_else(|| ConfigError::new("boolean condition has no term"))?;
            let (negated, atom) = if first.as_rule() == Rule::not_kw {
                (
                    true,
                    terms.next().ok_or_else(|| {
                        ConfigError::new("NOT condition has no following variable")
                    })?,
                )
            } else {
                (false, first)
            };
            let atom = atom
                .into_inner()
                .next()
                .ok_or_else(|| ConfigError::new("boolean term has no atom"))?;
            let variable = match atom.as_rule() {
                Rule::variable_reference => text(atom),
                Rule::bool_literal => text(atom).to_ascii_uppercase(),
                rule => return Err(unexpected(rule, "boolean expression")),
            };
            Ok(ConditionTerm { variable, negated })
        })
        .collect()
}

fn parse_literal(pair: Pair<'_, Rule>) -> Result<Literal, ConfigError> {
    let value = pair
        .into_inner()
        .next()
        .ok_or_else(|| ConfigError::new("literal has no value"))?;
    match value.as_rule() {
        Rule::bool_literal => Ok(Literal::Bool(value.as_str().eq_ignore_ascii_case("TRUE"))),
        Rule::integer => Ok(Literal::Integer(parse_integer(value)?)),
        Rule::real => value
            .as_str()
            .parse::<f64>()
            .map(Literal::Real)
            .map_err(|error| ConfigError::new(format!("invalid REAL literal: {error}"))),
        rule => Err(unexpected(rule, "literal")),
    }
}

fn parse_integer(pair: Pair<'_, Rule>) -> Result<i64, ConfigError> {
    pair.as_str()
        .parse::<i64>()
        .map_err(|error| ConfigError::new(format!("invalid integer literal: {error}")))
}

fn parse_duration(pair: Pair<'_, Rule>) -> Result<u64, ConfigError> {
    let value = pair.as_str();
    let body = value
        .strip_prefix("T#")
        .or_else(|| value.strip_prefix("t#"))
        .ok_or_else(|| ConfigError::new(format!("invalid duration literal {value}")))?;
    let (amount, multiplier) =
        if let Some(amount) = body.strip_suffix("ms").or_else(|| body.strip_suffix("MS")) {
            (amount, 1)
        } else if let Some(amount) = body.strip_suffix('s').or_else(|| body.strip_suffix('S')) {
            (amount, 1_000)
        } else {
            return Err(ConfigError::new(format!(
                "duration {value} must use ms or s"
            )));
        };
    amount
        .parse::<u64>()
        .map_err(|error| ConfigError::new(format!("invalid duration {value}: {error}")))?
        .checked_mul(multiplier)
        .ok_or_else(|| ConfigError::new(format!("duration {value} exceeds runtime limits")))
}

fn next_rule<'a>(
    pairs: &mut impl Iterator<Item = Pair<'a, Rule>>,
    expected: Rule,
    context: &str,
) -> Result<Pair<'a, Rule>, ConfigError> {
    let pair = pairs
        .next()
        .ok_or_else(|| ConfigError::new(format!("{context} is missing")))?;
    if pair.as_rule() != expected {
        return Err(ConfigError::new(format!(
            "{context} expected {expected:?}, found {:?}",
            pair.as_rule()
        )));
    }
    Ok(pair)
}

fn text(pair: Pair<'_, Rule>) -> String {
    pair.as_str().to_owned()
}

fn unexpected(rule: Rule, context: &str) -> ConfigError {
    ConfigError::new(format!(
        "unsupported Structured Text construct {rule:?} in {context}"
    ))
}

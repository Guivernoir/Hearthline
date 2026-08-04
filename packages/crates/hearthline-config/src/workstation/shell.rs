const MAX_ARGUMENTS: usize = 32;

pub(super) fn split_command_line(command: &str) -> Result<Vec<String>, String> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut started = false;

    for character in command.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            started = true;
            continue;
        }
        match (quote, character) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('\''), value) => {
                current.push(value);
                started = true;
            }
            (Some('"'), '\\') | (None, '\\') => {
                escaped = true;
                started = true;
            }
            (Some('"'), value) => {
                current.push(value);
                started = true;
            }
            (None, '\'' | '"') => {
                quote = Some(character);
                started = true;
            }
            (None, value) if value.is_whitespace() => {
                if started {
                    push_argument(&mut arguments, core::mem::take(&mut current))?;
                    started = false;
                }
            }
            (None, value) => {
                current.push(value);
                started = true;
            }
            _ => unreachable!("supported quote states are exhaustive"),
        }
    }

    if escaped {
        return Err("terminal: trailing escape is incomplete".into());
    }
    if quote.is_some() {
        return Err("terminal: quoted argument is not terminated".into());
    }
    if started {
        push_argument(&mut arguments, current)?;
    }
    Ok(arguments)
}

fn push_argument(arguments: &mut Vec<String>, argument: String) -> Result<(), String> {
    if arguments.len() == MAX_ARGUMENTS {
        return Err(format!(
            "terminal: command exceeds {MAX_ARGUMENTS} arguments"
        ));
    }
    arguments.push(argument);
    Ok(())
}

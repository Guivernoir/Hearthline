use crate::ScenarioHttpMethod;

pub(super) struct CurlRequest<'a> {
    pub method: ScenarioHttpMethod,
    pub url: &'a str,
    pub body: Option<&'a str>,
}

pub(super) fn parse_curl(arguments: &[String]) -> Result<CurlRequest<'_>, String> {
    let mut method = ScenarioHttpMethod::Get;
    let mut method_explicit = false;
    let mut url = None;
    let mut body = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        match argument {
            "-I" | "--head" => {
                method = ScenarioHttpMethod::Head;
                method_explicit = true;
            }
            "-X" | "--request" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| format!("curl: option {argument} requires a method"))?;
                method = parse_method(value)?;
                method_explicit = true;
            }
            value if value.starts_with("--request=") => {
                method = parse_method(&value["--request=".len()..])?;
                method_explicit = true;
            }
            value if value.starts_with("-X") && value.len() > 2 => {
                method = parse_method(&value[2..])?;
                method_explicit = true;
            }
            "-d" | "--data" | "--data-raw" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| format!("curl: option {argument} requires data"))?;
                set_body(&mut body, value)?;
            }
            value if value.starts_with("--data=") => {
                set_body(&mut body, &value["--data=".len()..])?;
            }
            value if value.starts_with("--data-raw=") => {
                set_body(&mut body, &value["--data-raw=".len()..])?;
            }
            value if value.starts_with("-d") && value.len() > 2 => {
                set_body(&mut body, &value[2..])?;
            }
            value if value.starts_with('-') => {
                return Err(format!("curl: unsupported option {value}"));
            }
            value => {
                if url.replace(value).is_some() {
                    return Err("curl: exactly one URL is supported".into());
                }
            }
        }
        index += 1;
    }
    if body.is_some() && !method_explicit {
        method = ScenarioHttpMethod::Post;
    }
    if body.is_some() && method == ScenarioHttpMethod::Head {
        return Err("curl: HEAD cannot include request data".into());
    }
    let url =
        url.ok_or_else(|| "Usage: curl [-I] [-X METHOD] [-d DATA] <https-url>".to_string())?;
    Ok(CurlRequest { method, url, body })
}

fn set_body<'a>(body: &mut Option<&'a str>, value: &'a str) -> Result<(), String> {
    if body.replace(value).is_some() {
        return Err("curl: exactly one request body is supported".into());
    }
    if value.len() > 256 {
        return Err("curl: request body exceeds 256 bytes".into());
    }
    Ok(())
}

fn parse_method(value: &str) -> Result<ScenarioHttpMethod, String> {
    match value.to_ascii_uppercase().as_str() {
        "GET" => Ok(ScenarioHttpMethod::Get),
        "HEAD" => Ok(ScenarioHttpMethod::Head),
        "POST" => Ok(ScenarioHttpMethod::Post),
        "PUT" => Ok(ScenarioHttpMethod::Put),
        "PATCH" => Ok(ScenarioHttpMethod::Patch),
        "DELETE" => Ok(ScenarioHttpMethod::Delete),
        "OPTIONS" => Ok(ScenarioHttpMethod::Options),
        _ => Err(format!("curl: unsupported HTTP method {value}")),
    }
}

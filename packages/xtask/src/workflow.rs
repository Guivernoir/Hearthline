use std::error::Error;
use std::fs;
use std::path::PathBuf;

pub fn verify() -> Result<(), Box<dyn Error>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for relative in [".github/workflows/ci.yml", ".github/workflows/nightly.yml"] {
        let source = fs::read_to_string(repository.join(relative))?;
        parse(relative, &source)?;
    }
    Ok(())
}

fn parse(path: &str, source: &str) -> Result<(), Box<dyn Error>> {
    serde_yaml_ng::from_str::<serde_yaml_ng::Value>(source)
        .map(|_| ())
        .map_err(|error| format!("invalid workflow YAML in {path}: {error}").into())
}

#[cfg(test)]
mod tests {
    use super::{parse, verify};

    #[test]
    fn repository_workflows_are_valid_yaml() {
        verify().expect("repository workflows");
    }

    #[test]
    fn workflow_parser_rejects_unquoted_expressions_in_flow_mappings() {
        let malformed = "steps:\n  - with: { shared-key: cache-${{ matrix.target }} }\n";
        assert!(parse("invalid.yml", malformed).is_err());
    }

    #[test]
    fn workflow_parser_accepts_quoted_expressions() {
        let valid = "steps:\n  - with:\n      shared-key: \"cache-${{ matrix.target }}\"\n";
        assert!(parse("valid.yml", valid).is_ok());
    }
}

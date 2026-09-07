use std::collections::BTreeMap;

use hearthline_api::contracts::{MODEL_API_CONTRACT_VERSION, generated_contract_files};
use serde_json::Value;

#[test]
fn generated_contract_bundle_exposes_revisioned_model_workflows() {
    let files = generated_contract_files().expect("contracts generate");
    let by_path = files
        .iter()
        .map(|(path, source)| (path.to_string_lossy().into_owned(), source.as_str()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(files.len(), 7);
    let openapi: Value = serde_json::from_str(
        by_path
            .get("project/contracts/openapi.json")
            .expect("OpenAPI contract"),
    )
    .expect("valid OpenAPI JSON");
    assert_eq!(openapi["openapi"], "3.1.0");
    assert_eq!(openapi["info"]["version"], MODEL_API_CONTRACT_VERSION);

    let paths = openapi["paths"].as_object().expect("OpenAPI paths");
    for path in [
        "/api/model",
        "/api/model/documents",
        "/api/model/drafts",
        "/api/model/drafts/{id}/preview",
        "/api/model/drafts/{id}/commit",
        "/api/model/sessions",
        "/api/model/sessions/{id}",
    ] {
        assert!(paths.contains_key(path), "OpenAPI omits {path}");
    }

    let client = by_path
        .get("packages/web/src/generated/model-api.ts")
        .expect("TypeScript client contract");
    for dto in [
        "ModelSummary",
        "DraftChange",
        "DraftPreview",
        "DraftCommitResponse",
        "SimulationSessionResponse",
    ] {
        assert!(client.contains(dto), "TypeScript contract omits {dto}");
    }
    for method in [
        "export class ModelApiClient",
        "createDraft()",
        "updateDraft(id: number",
        "commitDraft(id: number",
        "createSession()",
        "restartSession(id: number)",
    ] {
        assert!(client.contains(method), "TypeScript client omits {method}");
    }

    assert_openapi_references_resolve(&openapi, &openapi, "#");
    for (path, item) in paths {
        for method in ["get", "put", "post", "delete", "patch"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            for parameter in path_parameters(path) {
                let declared = operation["parameters"]
                    .as_array()
                    .is_some_and(|parameters| {
                        parameters.iter().any(|candidate| {
                            candidate["in"] == "path"
                                && candidate["name"] == parameter
                                && candidate["required"] == true
                        })
                    });
                assert!(declared, "{method} {path} omits path parameter {parameter}");
            }
        }
    }
}

#[test]
fn every_generated_schema_is_valid_json() {
    for (path, source) in generated_contract_files().expect("contracts generate") {
        if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            serde_json::from_str::<Value>(&source)
                .unwrap_or_else(|error| panic!("{} is invalid JSON: {error}", path.display()));
        }
    }
}

fn assert_openapi_references_resolve(document: &Value, value: &Value, location: &str) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str)
                && let Some(pointer) = reference.strip_prefix('#')
            {
                assert!(
                    document.pointer(pointer).is_some(),
                    "unresolved OpenAPI reference {reference} at {location}"
                );
            }
            for (key, nested) in object {
                assert_openapi_references_resolve(document, nested, &format!("{location}/{key}"));
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                assert_openapi_references_resolve(document, item, &format!("{location}/{index}"));
            }
        }
        _ => {}
    }
}

fn path_parameters(path: &str) -> Vec<&str> {
    path.split('{')
        .skip(1)
        .filter_map(|tail| tail.split_once('}').map(|(parameter, _)| parameter))
        .collect()
}

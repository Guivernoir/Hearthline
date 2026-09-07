use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::path::PathBuf;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use hearthline_api::{
    require_safe_write_binding, router_for_repository, router_for_repository_with_process_clock,
};
use serde_json::Value;
use tower::ServiceExt;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

async fn request(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    token: Option<&str>,
) -> (StatusCode, Option<Value>) {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let body = body
        .map(|value| Body::from(serde_json::to_vec(&value).expect("request JSON")))
        .unwrap_or_else(Body::empty);
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("router response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .expect("response body");
    let body = (!bytes.is_empty()).then(|| serde_json::from_slice(&bytes).expect("response JSON"));
    (status, body)
}

#[tokio::test]
async fn router_exposes_model_runtime_and_operator_boundaries() {
    let app = router_for_repository(repository_root(), Some("test-token".into()))
        .expect("canonical API router");

    for (uri, field) in [
        ("/api/health", "modelRevision"),
        ("/api/model", "revision"),
        ("/api/model/documents", "documents"),
        ("/api/config/catalog", "appliances"),
        ("/api/simulations", "scenarios"),
        ("/api/hmis/area-01-hmi-01", "interfaceKind"),
        ("/api/hmis/area-01-hmi-01/program", "source"),
        ("/api/workstations/customer-pc-01", "id"),
        ("/api/security/consoles/operations-soc-console-01", "events"),
    ] {
        let (status, body) = request(&app, Method::GET, uri, None, None).await;
        assert_eq!(status, StatusCode::OK, "GET {uri}: {body:?}");
        assert!(
            body.as_ref().is_some_and(|body| body.get(field).is_some()),
            "GET {uri} omits {field}: {body:?}"
        );
    }

    let (status, _) = request(&app, Method::POST, "/api/model/drafts", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, draft) = request(
        &app,
        Method::POST,
        "/api/model/drafts",
        None,
        Some("test-token"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let draft_id = draft
        .as_ref()
        .and_then(|body| body["id"].as_u64())
        .expect("draft ID");
    let (_, documents) = request(&app, Method::GET, "/api/model/documents", None, None).await;
    let document = documents
        .as_ref()
        .and_then(|body| body["documents"].as_array())
        .and_then(|documents| documents.first())
        .expect("editable document");
    let (status, update) = request(
        &app,
        Method::PUT,
        &format!("/api/model/drafts/{draft_id}/documents"),
        Some(serde_json::json!({
            "path": document["path"],
            "source": document["source"],
        })),
        Some("test-token"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "draft update: {update:?}");
    let (status, preview) = request(
        &app,
        Method::POST,
        &format!("/api/model/drafts/{draft_id}/preview"),
        None,
        Some("test-token"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "draft preview: {preview:?}");
    assert!(preview.expect("draft preview")["diagnostics"].is_array());
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/model/drafts/{draft_id}"),
        None,
        Some("test-token"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, session) = request(&app, Method::POST, "/api/model/sessions", None, None).await;
    assert_eq!(status, StatusCode::OK, "session create: {session:?}");
    let session_id = session
        .as_ref()
        .and_then(|body| body["id"].as_u64())
        .expect("simulation session ID");
    let (status, current) = request(
        &app,
        Method::GET,
        &format!("/api/model/sessions/{session_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(current.expect("session response")["status"], "current");
    let (status, restarted) = request(
        &app,
        Method::POST,
        &format!("/api/model/sessions/{session_id}/restart"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "session restart: {restarted:?}");

    let (status, scenarios) = request(&app, Method::GET, "/api/simulations", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let scenario_id = scenarios
        .as_ref()
        .and_then(|body| body["scenarios"].as_array())
        .and_then(|scenarios| scenarios.first())
        .and_then(|scenario| scenario["id"].as_str())
        .expect("scenario ID");
    let (status, report) = request(
        &app,
        Method::POST,
        &format!("/api/simulations/{scenario_id}/run"),
        Some(serde_json::json!({})),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "scenario run: {report:?}");

    let (status, action) = request(
        &app,
        Method::POST,
        "/api/hmis/area-01-hmi-01/actions",
        Some(serde_json::json!({ "kind": "reset-process" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "HMI action: {action:?}");
    for (method, uri, expected) in [
        (
            Method::GET,
            "/api/hmis/area-02-machine-pc-01/historian",
            StatusCode::OK,
        ),
        (
            Method::POST,
            "/api/hmis/area-02-machine-pc-01/telemetry",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let (status, body) = request(&app, method, uri, None, None).await;
        assert_eq!(status, expected, "{uri}: {body:?}");
    }

    let (status, workstation) = request(
        &app,
        Method::POST,
        "/api/workstations/customer-pc-01/actions",
        Some(serde_json::json!({ "kind": "terminal", "command": "help" })),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "workstation action: {workstation:?}"
    );
    let (status, security_action) = request(
        &app,
        Method::POST,
        "/api/workstations/customer-pc-01/actions",
        Some(serde_json::json!({
            "kind": "terminal",
            "command": "curl https://shop.hearthline.test/shop?file=../../etc/passwd"
        })),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "security action: {security_action:?}"
    );
    let (status, console) = request(
        &app,
        Method::GET,
        "/api/security/consoles/operations-soc-console-01",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "security console: {console:?}");
    let event_id = console
        .as_ref()
        .and_then(|body| body["events"].as_array())
        .and_then(|events| events.first())
        .and_then(|event| event["id"].as_u64())
        .expect("recorded security event");
    let (status, acknowledged) = request(
        &app,
        Method::POST,
        &format!("/api/security/events/{event_id}/acknowledge"),
        None,
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "acknowledge event: {acknowledged:?}"
    );
    assert_eq!(
        acknowledged.expect("acknowledged event")["acknowledged"],
        true
    );
    let (status, _) = request(
        &app,
        Method::POST,
        "/api/security/events/999999/acknowledge",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, cleared) = request(
        &app,
        Method::POST,
        "/api/security/consoles/operations-soc-console-01/clear",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "security clear: {cleared:?}");

    for uri in [
        "/api/hmis/missing-hmi",
        "/api/workstations/missing-workstation",
        "/api/security/consoles/missing-console",
        "/api/model/sessions/999999",
    ] {
        let (status, _) = request(&app, Method::GET, uri, None, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "GET {uri}");
    }
    let (status, _) = request(
        &app,
        Method::GET,
        "/api/security/consoles/customer-pc-01",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    for (method, uri, body) in [
        (
            Method::PUT,
            "/api/model/drafts/999999/documents",
            Some(serde_json::json!({
                "path": "project/config/appliances/missing.yaml",
                "source": "schema_version: 0.1.0"
            })),
        ),
        (Method::POST, "/api/model/drafts/999999/preview", None),
        (
            Method::POST,
            "/api/model/drafts/999999/commit",
            Some(serde_json::json!({
                "expectedRevision": "missing",
                "reason": "negative route contract"
            })),
        ),
        (Method::DELETE, "/api/model/drafts/999999", None),
    ] {
        let (status, _) = request(&app, method, uri, body, Some("test-token")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    for (method, uri, body) in [
        (
            Method::POST,
            "/api/simulations/missing-scenario/run",
            Some(serde_json::json!({})),
        ),
        (Method::POST, "/api/model/sessions/999999/restart", None),
    ] {
        let (status, _) = request(&app, method, uri, body, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}

#[tokio::test]
async fn loopback_router_without_authentication_allows_draft_lifecycle() {
    let app = router_for_repository(repository_root(), None).expect("unauthenticated loopback API");
    let (status, draft) = request(&app, Method::POST, "/api/model/drafts", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let id = draft
        .as_ref()
        .and_then(|body| body["id"].as_u64())
        .expect("draft ID");
    let (status, _) = request(
        &app,
        Method::DELETE,
        &format!("/api/model/drafts/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn model_commit_preserves_live_state_and_stales_pinned_sessions() {
    let temporary = tempfile::tempdir().expect("temporary repository");
    copy_tree(
        &repository_root().join("project"),
        &temporary.path().join("project"),
    );
    copy_tree(
        &repository_root().join("packages/web/src/generated"),
        &temporary.path().join("packages/web/src/generated"),
    );
    let app = router_for_repository(temporary.path(), None).expect("temporary API router");

    let (_, created_session) = request(&app, Method::POST, "/api/model/sessions", None, None).await;
    let session_id = created_session.unwrap()["id"].as_u64().unwrap();
    let (status, action) = request(
        &app,
        Method::POST,
        "/api/hmis/area-01-hmi-01/actions",
        Some(serde_json::json!({
            "kind": "set-water-pump-failure",
            "pumpId": "area-01-wd-pmp-01a",
            "failed": true
        })),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "state-changing HMI action: {action:?}"
    );
    let expected_snapshot = action.unwrap()["snapshot"].clone();

    let (_, draft) = request(&app, Method::POST, "/api/model/drafts", None, None).await;
    let draft = draft.unwrap();
    let draft_id = draft["id"].as_u64().unwrap();
    let base_revision = draft["baseRevision"].as_str().unwrap();
    let (_, documents) = request(&app, Method::GET, "/api/model/documents", None, None).await;
    let document = documents.unwrap()["documents"][0].clone();
    let source = format!(
        "{}\n# revision-pinning regression\n",
        document["source"].as_str().unwrap().trim_end()
    );
    let (status, update) = request(
        &app,
        Method::PUT,
        &format!("/api/model/drafts/{draft_id}/documents"),
        Some(serde_json::json!({ "path": document["path"], "source": source })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "draft update: {update:?}");
    let (status, commit) = request(
        &app,
        Method::POST,
        &format!("/api/model/drafts/{draft_id}/commit"),
        Some(serde_json::json!({
            "expectedRevision": base_revision,
            "reason": "verify live revision preservation"
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "draft commit: {commit:?}");
    assert_eq!(commit.unwrap()["staleSessionCount"], 1);

    let (status, session) = request(
        &app,
        Method::GET,
        &format!("/api/model/sessions/{session_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session.unwrap()["status"], "stale");
    let (status, profile) =
        request(&app, Method::GET, "/api/hmis/area-01-hmi-01", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(profile.unwrap(), expected_snapshot);
}

#[tokio::test]
async fn process_clock_collects_replicates_and_publishes_historian_records() {
    let app = router_for_repository_with_process_clock(repository_root(), None)
        .expect("clocked canonical API router");

    for (uri, expected) in [
        ("/api/hmis/area-02-hmi-01/historian", StatusCode::FORBIDDEN),
        ("/api/hmis/missing-hmi/historian", StatusCode::NOT_FOUND),
    ] {
        let (status, _) = request(&app, Method::GET, uri, None, None).await;
        assert_eq!(status, expected, "GET {uri}");
    }

    tokio::time::sleep(std::time::Duration::from_millis(1_400)).await;
    let (status, body) = request(
        &app,
        Method::GET,
        "/api/hmis/area-02-machine-pc-01/historian",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "historian status: {body:?}");
    let body = body.expect("historian status body");
    assert!(
        body["local"]["storedRecords"]
            .as_u64()
            .is_some_and(|count| count > 0)
    );
    assert!(
        body["replica"]["storedRecords"]
            .as_u64()
            .is_some_and(|count| count > 0)
    );
    assert_eq!(body["pendingRecords"], 0);
    assert!(
        body["replicationAttempts"]
            .as_u64()
            .is_some_and(|count| count > 0)
    );
    assert!(body["lastCollection"].is_object());
    assert!(body["lastReplication"].is_object());

    let (status, report) = request(
        &app,
        Method::POST,
        "/api/hmis/area-02-machine-pc-01/telemetry",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "historian publication: {report:?}");
    assert_eq!(report.expect("publication report")["expectation_met"], true);
}

#[test]
fn non_loopback_write_binding_requires_authentication() {
    let public = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    assert!(require_safe_write_binding(public, None).is_err());
    assert!(require_safe_write_binding(public, Some(" ")).is_err());
    assert!(require_safe_write_binding(public, Some("token")).is_ok());
    assert!(require_safe_write_binding(IpAddr::V4(Ipv4Addr::LOCALHOST), None).is_ok());
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create copied directory");
    for entry in fs::read_dir(source).expect("read source directory") {
        let entry = entry.expect("source entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("source file type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy source file");
        }
    }
}

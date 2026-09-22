use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::json;
use tower::ServiceExt;
mod common;
async fn send(method: &str, path: &str, headers: &[(&str, &str)], body: Body) -> Response {
    let mut request = Request::builder().method(method).uri(path);
    for (key, value) in headers {
        request = request.header(*key, *value);
    }
    txsignx_api::app()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
}
#[tokio::test]
async fn strict_json_rejects_duplicates_unknown_fields_and_trailing_values() {
    for body in [
        r#"{"psbt":"a","psbt":"b"}"#,
        r#"{"psbt":"a","path":"PRIVATE_MARKER"}"#,
        r#"{"psbt":"a"}{}"#,
        r#"{"psbt":false}"#,
        r#"{"psbt":"a","policy":{"max_fee_ratio_bps":1,"max_fee_ratio_bps":2}}"#,
    ] {
        let response = send(
            "POST",
            "/api/v1/psbt/preflight",
            &[
                ("host", "127.0.0.1:8080"),
                ("content-type", "application/json"),
            ],
            Body::from(body),
        )
        .await;
        assert_eq!(response.status(), 400);
        let text = to_bytes(response.into_body(), 1024).await.unwrap();
        assert!(!String::from_utf8_lossy(&text).contains("PRIVATE_MARKER"));
    }
}
#[tokio::test]
async fn text_and_transport_sizes_are_bounded() {
    for body in [
        json!({"psbt":"A".repeat(txsignx_api::config::TEXT_BYTES+1)}).to_string(),
        " ".repeat(txsignx_api::config::BODY_BYTES + 1),
    ] {
        let response = send(
            "POST",
            "/api/v1/psbt/inspect",
            &[
                ("host", "127.0.0.1:8080"),
                ("content-type", "application/json"),
            ],
            Body::from(body),
        )
        .await;
        assert_eq!(response.status(), 413);
    }
}
#[tokio::test]
async fn rejected_browser_origins_and_rebinding_hosts_never_receive_cors_permission() {
    for headers in [
        vec![("host", "attacker.example:8080")],
        vec![("host", "127.0.0.1:8080"), ("origin", "null")],
        vec![
            ("host", "127.0.0.1:8080"),
            ("origin", "https://attacker.example"),
        ],
        vec![("host", "127.0.0.1:8080"), ("host", "attacker.example")],
        vec![],
        vec![
            ("host", "127.0.0.1:8080"),
            ("origin", "http://localhost:5173"),
            ("origin", "https://attacker.example"),
        ],
    ] {
        for method in ["POST", "OPTIONS"] {
            let response = send(
                method,
                "/api/v1/psbt/preflight",
                &headers,
                Body::from("PRIVATE_MARKER"),
            )
            .await;
            assert_eq!(response.status(), 403);
            assert!(
                !response
                    .headers()
                    .contains_key("access-control-allow-origin")
            );
            assert_eq!(response.headers()["cache-control"], "no-store");
        }
    }
}
#[tokio::test]
async fn cors_preflight_is_explicit_and_has_no_credentials() {
    let response = send(
        "OPTIONS",
        "/api/v1/psbt/preflight",
        &[
            ("host", "127.0.0.1:8080"),
            ("origin", "http://localhost:5173"),
            ("access-control-request-method", "POST"),
            ("access-control-request-headers", "content-type"),
        ],
        Body::empty(),
    )
    .await;
    assert_eq!(response.status(), 204);
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        "http://localhost:5173"
    );
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-credentials")
    );
    let response = send(
        "OPTIONS",
        "/api/v1/psbt/preflight",
        &[
            ("host", "127.0.0.1:8080"),
            ("origin", "http://localhost:5173"),
            ("access-control-request-method", "DELETE"),
        ],
        Body::empty(),
    )
    .await;
    assert_eq!(response.status(), 403);
}
#[tokio::test]
async fn media_query_method_and_missing_route_errors_are_json_and_hardened() {
    for (method, path, media, status) in [
        ("POST", "/api/v1/psbt/inspect", "text/plain", 415),
        (
            "POST",
            "/api/v1/psbt/inspect?psbt=PRIVATE_MARKER",
            "application/json",
            400,
        ),
        ("GET", "/api/v1/psbt/inspect", "application/json", 405),
        ("POST", "/api/v1/health", "application/json", 405),
        ("POST", "/api/v1/psbt/broadcast", "application/json", 404),
    ] {
        let response = send(
            method,
            path,
            &[("host", "127.0.0.1:8080"), ("content-type", media)],
            Body::empty(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::from_u16(status).unwrap());
        for (key, value) in [
            ("content-type", "application/json"),
            ("cache-control", "no-store"),
            ("x-content-type-options", "nosniff"),
            ("referrer-policy", "no-referrer"),
        ] {
            assert_eq!(response.headers()[key], value);
        }
        assert!(response.headers().contains_key("content-security-policy"));
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
        assert!(value["error"]["code"].is_string());
    }
}

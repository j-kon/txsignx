use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;

#[tokio::test]
async fn health_has_only_public_service_metadata() {
    let response = txsignx_api::app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .header("host", "127.0.0.1:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"status":"ok","service":"txsignx-api","version":"0.1.0"})
    );
}

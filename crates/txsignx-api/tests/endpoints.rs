use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;
mod common;
use common::request;
use serde_json::json;

#[tokio::test]
async fn capabilities_do_not_claim_node_or_broadcast() {
    let (s, v) = request(
        txsignx_api::app(),
        "GET",
        "/api/v1/capabilities",
        json!(null),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["node_context_available"], false);
    assert_eq!(v["broadcast_via_api"], false);
    assert_eq!(v["signing"], false);
    assert_eq!(v["finalization"], false);
    assert_eq!(v["active_rules"], 15);
    assert_eq!(v["deferred_rules"], 2);
}

#[tokio::test]
async fn catalog_and_detail_use_actual_engine_registry() {
    let (s, v) = request(txsignx_api::app(), "GET", "/api/v1/policies", json!(null)).await;
    assert_eq!(s, 200);
    assert_eq!(v["active_rules"].as_array().unwrap().len(), 15);
    assert_eq!(v["deferred_rules"].as_array().unwrap().len(), 2);
    for code in ["TG001", "TG007", "TG017"] {
        let (s, v) = request(
            txsignx_api::app(),
            "GET",
            &format!("/api/v1/policies/{code}"),
            json!(null),
        )
        .await;
        assert_eq!(s, 200);
        assert_eq!(v["code"], code);
        assert_eq!(v["active"], code != "TG007");
    }
    let (s, v) = request(
        txsignx_api::app(),
        "GET",
        "/api/v1/policies/PRIVATE_MARKER",
        json!(null),
    )
    .await;
    assert_eq!(s, 404);
    assert!(!v.to_string().contains("PRIVATE_MARKER"));
}

#[tokio::test]
async fn inspection_reuses_core_and_never_invents_network_or_fee() {
    let (s, v) = request(
        txsignx_api::app(),
        "POST",
        "/api/v1/psbt/inspect",
        json!({"psbt":common::PASS}),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(
        v,
        serde_json::to_value(txsignx_core::analyze_psbt(common::PASS).unwrap()).unwrap()
    );
    assert!(v.get("network").is_none());
    let tx = txsignx_core::psbt::decode_psbt(common::PASS)
        .unwrap()
        .unsigned_tx;
    // Read public fixture hex via the engine's existing Bitcoin dependency in the fixture below.
    assert_eq!(v["unsigned_txid"], tx.compute_txid().to_string());
}

#[tokio::test]
async fn invalid_inputs_do_not_echo_user_text() {
    for (path, key) in [
        ("psbt/inspect", "psbt"),
        ("transactions/inspect", "raw_transaction"),
    ] {
        let (s, v) = request(
            txsignx_api::app(),
            "POST",
            &format!("/api/v1/{path}"),
            json!({key:"PRIVATE_MARKER"}),
        )
        .await;
        assert_eq!(s, 422);
        assert!(v["error"]["code"].is_string());
        assert!(!v.to_string().contains("PRIVATE_MARKER"));
    }
}

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

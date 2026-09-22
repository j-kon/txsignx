mod common;
use common::{PASS, request};
use serde_json::json;
fn wallet() -> serde_json::Value {
    json!({"network":"regtest","external_descriptor":include_str!("../../../fixtures/wallet/external.desc").trim(),"internal_descriptor":include_str!("../../../fixtures/wallet/internal.desc").trim(),"derivation_window":8})
}
#[tokio::test]
async fn offline_decisions_and_coverage_are_engine_reports() {
    for (psbt, decision) in [
        (PASS, "pass"),
        (
            include_str!("../../../fixtures/policy/missing-utxo.b64"),
            "review",
        ),
        (
            include_str!("../../../fixtures/policy/absolute-fee.b64"),
            "block",
        ),
    ] {
        let (status, value) = request(
            txsignx_api::app(),
            "POST",
            "/api/v1/psbt/preflight",
            json!({"psbt":psbt}),
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(value["policy"]["decision"], decision);
        assert!(value["wallet_context"].is_null());
        assert!(value["node_context"].is_null());
        let inspection = txsignx_core::analyze_psbt(psbt).unwrap();
        let policy = txsignx_policy::PolicyEngine::development()
            .unwrap()
            .evaluate_with_context(&inspection, &Default::default(), None, None)
            .unwrap();
        assert_eq!(value["policy"], serde_json::to_value(policy).unwrap());
    }
}
#[tokio::test]
async fn wallet_context_is_classified_without_descriptor_disclosure() {
    let request_body =
        json!({"psbt":include_str!("../../../fixtures/wallet/payment.b64"),"wallet":wallet()});
    let (status, value) = request(
        txsignx_api::app(),
        "POST",
        "/api/v1/psbt/preflight",
        request_body,
    )
    .await;
    assert_eq!(status, 200);
    assert!(value["wallet_context"].is_object());
    assert!(!value.to_string().contains("tpub"));
    assert!(
        !value
            .to_string()
            .contains(include_str!("../../../fixtures/wallet/external.desc").trim())
    );
}
#[tokio::test]
async fn invalid_context_and_node_requests_fail_closed() {
    let mut invalid_wallet = wallet();
    invalid_wallet["derivation_window"] = json!(0);
    for (body, status) in [
        (
            json!({"psbt":PASS,"policy":{"max_fee_ratio_bps":10001}}),
            422,
        ),
        (json!({"psbt":PASS,"wallet":invalid_wallet}), 422),
        (json!({"psbt":PASS,"wallet":{"network":"regtest"}}), 400),
        (
            json!({"psbt":PASS,"node":{"use_configured_node":true}}),
            422,
        ),
        (
            json!({"psbt":PASS,"wallet":wallet(),"node":{"use_configured_node":true}}),
            503,
        ),
        (
            json!({"psbt":PASS,"node":{"use_configured_node":false,"rpc_url":"PRIVATE_MARKER"}}),
            400,
        ),
        (
            json!({"psbt":PASS,"wallet":{"network":"regtest","external_descriptor_file":"PRIVATE_MARKER"}}),
            400,
        ),
    ] {
        let (actual, value) =
            request(txsignx_api::app(), "POST", "/api/v1/psbt/preflight", body).await;
        assert_eq!(actual, status);
        assert!(!value.to_string().contains("PRIVATE_MARKER"));
    }
}
#[tokio::test]
async fn policy_overrides_are_evaluated_only_by_rust() {
    let (status, value) = request(
        txsignx_api::app(),
        "POST",
        "/api/v1/psbt/preflight",
        json!({"psbt":PASS,"policy":{"max_absolute_fee_sats":0}}),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(value["policy"]["decision"], "block");
}

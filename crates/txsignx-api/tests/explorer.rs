use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use bitcoin::{
    Amount, BlockHash, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    hashes::Hash,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use tower::ServiceExt;
use txsignx_api::{Config, ConfiguredNode, app, app_with_config};
use txsignx_node::{
    BlockchainInfo, MempoolAcceptance, NodeError, NodeRpc, NodeTransaction, NodeTxOut,
};
use txsignx_wallet::ConfiguredNetwork;

mod common;
use common::request;

const SEGWIT_HEX: &str = include_str!("../../../crates/txsignx-core/tests/fixtures/segwit.hex");

struct MockExplorerRpc {
    info: BlockchainInfo,
    transactions: HashMap<Txid, NodeTransaction>,
}

impl MockExplorerRpc {
    fn new(network: Network) -> Self {
        Self {
            info: BlockchainInfo {
                network,
                blocks: 100,
                headers: 100,
                best_block_hash: BlockHash::from_byte_array([1; 32]),
                initial_block_download: false,
                verification_progress: Some(1.0),
            },
            transactions: HashMap::new(),
        }
    }
}

impl NodeRpc for MockExplorerRpc {
    fn blockchain_info(&self) -> Result<BlockchainInfo, NodeError> {
        Ok(self.info.clone())
    }
    fn best_block_hash(&self) -> Result<BlockHash, NodeError> {
        Ok(self.info.best_block_hash)
    }
    fn block_count(&self) -> Result<u64, NodeError> {
        Ok(self.info.blocks)
    }
    fn get_tx_out(&self, _: &OutPoint, _: bool) -> Result<Option<NodeTxOut>, NodeError> {
        Ok(None)
    }
    fn test_mempool_accept(&self, _: &Transaction) -> Result<MempoolAcceptance, NodeError> {
        panic!("unused")
    }
    fn send_raw_transaction(&self, _: &Transaction) -> Result<Txid, NodeError> {
        panic!("unused")
    }
    fn get_raw_transaction(&self, txid: &Txid) -> Result<NodeTransaction, NodeError> {
        self.transactions
            .get(txid)
            .cloned()
            .ok_or(NodeError::TransactionNotFound)
    }
}

fn create_node_app(mock: MockExplorerRpc) -> axum::Router {
    let config = Config {
        node: Some(ConfiguredNode::new(ConfiguredNetwork::Regtest, mock)),
        ..Default::default()
    };
    app_with_config(config)
}

fn create_test_tx(
    prev_txid: Txid,
    prev_vout: u32,
    output_sats: u64,
    script_pubkey: ScriptBuf,
) -> Transaction {
    Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: prev_txid,
                vout: prev_vout,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: bitcoin::Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(output_sats),
            script_pubkey,
        }],
    }
}

// =========================================================================
// RAW MODE TESTS (1-9)
// =========================================================================

#[tokio::test]
async fn test_01_legacy_raw_request_still_works_unchanged() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({"raw_transaction": SEGWIT_HEX.trim()}),
    )
    .await;
    assert_eq!(s, 200);
    assert!(v["txid"].is_string());
    assert_eq!(v["version"], 2);
    assert_eq!(v["input_count"], 1);
    assert_eq!(v["output_count"], 2);
    assert!(v["fee_sats"].is_null());
    assert!(v["fee_rate"].is_null());
    assert!(v["chain_context"].is_null());
    assert!(v["total_input_sats"].is_null());
}

#[tokio::test]
async fn test_02_raw_request_without_network_has_no_addresses() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({"raw_transaction": SEGWIT_HEX.trim()}),
    )
    .await;
    assert_eq!(s, 200);
    let outputs = v["outputs"].as_array().unwrap();
    assert_eq!(outputs.len(), 2);
    for out in outputs {
        assert!(out["address"].is_null());
    }
}

#[tokio::test]
async fn test_03_raw_request_with_bitcoin_network_renders_mainnet_address() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({
            "raw_transaction": SEGWIT_HEX.trim(),
            "network": "bitcoin"
        }),
    )
    .await;
    assert_eq!(s, 200);
    let outputs = v["outputs"].as_array().unwrap();
    // Output 0 is P2PKH on mainnet: starts with '1'
    let addr0 = outputs[0]["address"].as_str().unwrap();
    assert!(
        addr0.starts_with('1'),
        "expected mainnet P2PKH address, got {addr0}"
    );
    // Output 1 is P2WPKH on mainnet: starts with 'bc1q'
    let addr1 = outputs[1]["address"].as_str().unwrap();
    assert!(
        addr1.starts_with("bc1q"),
        "expected mainnet P2WPKH address, got {addr1}"
    );
}

#[tokio::test]
async fn test_04_raw_request_with_regtest_renders_regtest_address() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({
            "raw_transaction": SEGWIT_HEX.trim(),
            "network": "regtest"
        }),
    )
    .await;
    assert_eq!(s, 200);
    let outputs = v["outputs"].as_array().unwrap();
    // Output 0 is P2PKH on regtest: starts with 'm' or 'n'
    let addr0 = outputs[0]["address"].as_str().unwrap();
    assert!(
        addr0.starts_with('m') || addr0.starts_with('n'),
        "expected regtest P2PKH, got {addr0}"
    );
    // Output 1 is P2WPKH on regtest: starts with 'bcrt1q'
    let addr1 = outputs[1]["address"].as_str().unwrap();
    assert!(
        addr1.starts_with("bcrt1q"),
        "expected regtest P2WPKH, got {addr1}"
    );
}

#[tokio::test]
async fn test_05_raw_request_does_not_invent_fee() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({
            "raw_transaction": SEGWIT_HEX.trim(),
            "network": "bitcoin"
        }),
    )
    .await;
    assert_eq!(s, 200);
    assert!(v["fee_sats"].is_null());
    assert!(v["fee_rate"].is_null());
    assert!(v["total_input_sats"].is_null());
}

#[tokio::test]
async fn test_06_raw_request_has_no_chain_context() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({
            "raw_transaction": SEGWIT_HEX.trim(),
            "network": "bitcoin"
        }),
    )
    .await;
    assert_eq!(s, 200);
    assert!(v["chain_context"].is_null());
}

#[tokio::test]
async fn test_07_malformed_raw_gives_sanitized_invalid_transaction() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({"raw_transaction": "deadbeef"}),
    )
    .await;
    assert_eq!(s, 422);
    assert_eq!(v["error"]["code"], "invalid_transaction");
    assert!(!v.to_string().contains("deadbeef"));
}

#[tokio::test]
async fn test_08_unknown_network_rejected() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({
            "raw_transaction": SEGWIT_HEX.trim(),
            "network": "fakenet"
        }),
    )
    .await;
    assert_eq!(s, 400);
    assert_eq!(v["error"]["code"], "invalid_network");
}

#[tokio::test]
async fn test_09_unknown_request_field_rejected() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({
            "raw_transaction": SEGWIT_HEX.trim(),
            "unknown_extra": "not_allowed"
        }),
    )
    .await;
    assert_eq!(s, 400);
    assert_eq!(v["error"]["code"], "invalid_json");
}

// =========================================================================
// SOURCE VALIDATION TESTS (10-12)
// =========================================================================

#[tokio::test]
async fn test_10_raw_plus_txid_rejected() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({
            "raw_transaction": SEGWIT_HEX.trim(),
            "txid": "0000000000000000000000000000000000000000000000000000000000000000"
        }),
    )
    .await;
    assert_eq!(s, 400);
    assert_eq!(v["error"]["code"], "invalid_context");
}

#[tokio::test]
async fn test_11_neither_raw_nor_txid_rejected() {
    let (s1, v1) = request(app(), "POST", "/api/v1/transactions/inspect", json!({})).await;
    assert_eq!(s1, 400);
    assert_eq!(v1["error"]["code"], "invalid_context");

    let (s2, v2) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({"network": "bitcoin"}),
    )
    .await;
    assert_eq!(s2, 400);
    assert_eq!(v2["error"]["code"], "invalid_context");
}

#[tokio::test]
async fn test_12_malformed_txid_rejected() {
    for bad in [
        "not-a-txid",
        "1234",
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
    ] {
        let mock = MockExplorerRpc::new(Network::Regtest);
        let node_app = create_node_app(mock);
        let (s, v) = request(
            node_app,
            "POST",
            "/api/v1/transactions/inspect",
            json!({"txid": bad}),
        )
        .await;
        assert_eq!(s, 400);
        assert_eq!(v["error"]["code"], "invalid_txid");
    }
}

#[tokio::test]
async fn test_txid_with_network_rejected() {
    let mock = MockExplorerRpc::new(Network::Regtest);
    let node_app = create_node_app(mock);
    let (s, v) = request(
        node_app,
        "POST",
        "/api/v1/transactions/inspect",
        json!({
            "txid": "0000000000000000000000000000000000000000000000000000000000000000",
            "network": "regtest"
        }),
    )
    .await;
    assert_eq!(s, 400);
    assert_eq!(v["error"]["code"], "invalid_context");
}

// =========================================================================
// TXID MODE TESTS (13-22)
// =========================================================================

#[tokio::test]
async fn test_13_txid_without_configured_node_returns_node_not_configured() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect",
        json!({"txid": "0000000000000000000000000000000000000000000000000000000000000000"}),
    )
    .await;
    assert_eq!(s, 400);
    assert_eq!(v["error"]["code"], "node_not_configured");
}

#[tokio::test]
async fn test_14_to_20_fake_configured_node_transaction_lookup() {
    let mut mock = MockExplorerRpc::new(Network::Regtest);

    // Prevout tx: pays 100,000 sats to a standard P2WPKH script
    let prev_spk = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_byte_array([3; 20]));
    let prev_tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![TxOut {
            value: Amount::from_sat(100_000),
            script_pubkey: prev_spk.clone(),
        }],
    };
    let prev_txid = prev_tx.compute_txid();
    mock.transactions.insert(
        prev_txid,
        NodeTransaction {
            transaction: prev_tx,
            block_hash: Some(BlockHash::from_byte_array([10; 32])),
            confirmations: Some(20),
        },
    );

    // Confirmed tx: spends prevout, pays 80,000 sats (fee = 20,000 sats)
    let out_spk = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_byte_array([4; 20]));
    let target_tx = create_test_tx(prev_txid, 0, 80_000, out_spk);
    let target_txid = target_tx.compute_txid();
    let block_hash = BlockHash::from_byte_array([11; 32]);
    mock.transactions.insert(
        target_txid,
        NodeTransaction {
            transaction: target_tx,
            block_hash: Some(block_hash),
            confirmations: Some(6),
        },
    );

    let node_app = create_node_app(mock);

    // 14. fake configured-node transaction lookup succeeds
    let (s, v) = request(
        node_app,
        "POST",
        "/api/v1/transactions/inspect",
        json!({"txid": target_txid.to_string()}),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["txid"], target_txid.to_string());

    // 15. confirmed tx returns confirmed chain context
    assert_eq!(v["chain_context"]["status"], "confirmed");
    assert_eq!(v["chain_context"]["confirmations"], 6);
    assert_eq!(v["chain_context"]["block_hash"], block_hash.to_string());
    assert_eq!(v["chain_context"]["network"], "regtest");

    // 17. resolved prevout produces input total
    assert_eq!(v["total_input_sats"], 100_000);

    // 18. resolved prevout produces fee
    assert_eq!(v["fee_sats"], 20_000);

    // 19. resolved prevout produces fee rate
    assert!(v["fee_rate"]["sat_per_vb"].as_f64().unwrap() > 0.0);
    assert_eq!(v["fee_rate"]["fee_sats"], 20_000);

    // 20. resolved prevout address rendered
    let inputs = v["inputs"].as_array().unwrap();
    let prevout = &inputs[0]["resolved_prevout"];
    let prevout_addr = prevout["address"].as_str().unwrap();
    assert!(
        prevout_addr.starts_with("bcrt1q"),
        "expected regtest address, got {prevout_addr}"
    );
    assert_eq!(prevout["value_sats"], 100_000);
}

#[tokio::test]
async fn test_16_mempool_tx_returns_mempool_context() {
    let mut mock = MockExplorerRpc::new(Network::Regtest);

    let prev_tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![TxOut {
            value: Amount::from_sat(50_000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    let prev_txid = prev_tx.compute_txid();
    mock.transactions.insert(
        prev_txid,
        NodeTransaction {
            transaction: prev_tx,
            block_hash: Some(BlockHash::from_byte_array([10; 32])),
            confirmations: Some(1),
        },
    );

    let target_tx = create_test_tx(prev_txid, 0, 45_000, ScriptBuf::new());
    let target_txid = target_tx.compute_txid();
    mock.transactions.insert(
        target_txid,
        NodeTransaction {
            transaction: target_tx,
            block_hash: None,
            confirmations: None, // mempool: no confirmations
        },
    );

    let node_app = create_node_app(mock);
    let (s, v) = request(
        node_app,
        "POST",
        "/api/v1/transactions/inspect",
        json!({"txid": target_txid.to_string()}),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["chain_context"]["status"], "mempool");
    assert!(v["chain_context"]["confirmations"].is_null());
    assert!(v["chain_context"]["block_hash"].is_null());
    assert_eq!(v["fee_sats"], 5_000);
}

#[tokio::test]
async fn test_21_node_lookup_failure_sanitized() {
    let mock = MockExplorerRpc::new(Network::Regtest);
    let node_app = create_node_app(mock);
    let unknown_txid = "2222222222222222222222222222222222222222222222222222222222222222";
    let (s, v) = request(
        node_app,
        "POST",
        "/api/v1/transactions/inspect",
        json!({"txid": unknown_txid}),
    )
    .await;
    assert_eq!(s, 503);
    assert_eq!(v["error"]["code"], "node_unavailable");
    assert_eq!(v["error"]["message"], "Configured node is unavailable.");
    assert!(!v.to_string().contains(unknown_txid));
}

#[tokio::test]
async fn test_22_txid_mismatch_cannot_pass_through_node_layer() {
    struct MismatchRpc {
        info: BlockchainInfo,
    }
    impl NodeRpc for MismatchRpc {
        fn blockchain_info(&self) -> Result<BlockchainInfo, NodeError> {
            Ok(self.info.clone())
        }
        fn best_block_hash(&self) -> Result<BlockHash, NodeError> {
            Ok(self.info.best_block_hash)
        }
        fn block_count(&self) -> Result<u64, NodeError> {
            Ok(self.info.blocks)
        }
        fn get_tx_out(&self, _: &OutPoint, _: bool) -> Result<Option<NodeTxOut>, NodeError> {
            Ok(None)
        }
        fn test_mempool_accept(&self, _: &Transaction) -> Result<MempoolAcceptance, NodeError> {
            panic!("unused")
        }
        fn send_raw_transaction(&self, _: &Transaction) -> Result<Txid, NodeError> {
            panic!("unused")
        }
        fn get_raw_transaction(&self, _txid: &Txid) -> Result<NodeTransaction, NodeError> {
            let dummy = Transaction {
                version: bitcoin::transaction::Version::TWO,
                lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
                input: vec![],
                output: vec![TxOut {
                    value: Amount::from_sat(100),
                    script_pubkey: ScriptBuf::new(),
                }],
            };
            Ok(NodeTransaction {
                transaction: dummy,
                block_hash: None,
                confirmations: Some(1),
            })
        }
    }

    let config = Config {
        node: Some(ConfiguredNode::new(
            ConfiguredNetwork::Regtest,
            MismatchRpc {
                info: BlockchainInfo {
                    network: Network::Regtest,
                    blocks: 10,
                    headers: 10,
                    best_block_hash: BlockHash::from_byte_array([1; 32]),
                    initial_block_download: false,
                    verification_progress: Some(1.0),
                },
            },
        )),
        ..Default::default()
    };
    let node_app = app_with_config(config);
    let req_txid = "3333333333333333333333333333333333333333333333333333333333333333";
    let (s, v) = request(
        node_app,
        "POST",
        "/api/v1/transactions/inspect",
        json!({"txid": req_txid}),
    )
    .await;
    assert_eq!(s, 503);
    assert_eq!(v["error"]["code"], "node_unavailable");
}

// =========================================================================
// CAPABILITIES TESTS (23-28)
// =========================================================================

#[tokio::test]
async fn test_23_to_28_capabilities() {
    // 23, 24, 26, 27, 28: without node
    let (s, v) = request(app(), "GET", "/api/v1/capabilities", json!(null)).await;
    assert_eq!(s, 200);
    assert_eq!(v["transaction_explorer"], true);
    assert_eq!(v["txid_inspection"], true);
    assert_eq!(v["transaction_address_rendering"], true);
    assert_eq!(v["node_context_available"], false);
    assert_eq!(v["signing"], false);
    assert_eq!(v["finalization"], false);
    assert_eq!(v["broadcast_via_api"], false);

    // 25: with node
    let mock = MockExplorerRpc::new(Network::Regtest);
    let node_app = create_node_app(mock);
    let (s2, v2) = request(node_app, "GET", "/api/v1/capabilities", json!(null)).await;
    assert_eq!(s2, 200);
    assert_eq!(v2["node_context_available"], true);
    assert_eq!(v2["txid_inspection"], true);
    assert_eq!(v2["signing"], false);
    assert_eq!(v2["finalization"], false);
    assert_eq!(v2["broadcast_via_api"], false);
}

// =========================================================================
// HTTP & SAFETY TESTS (29-33)
// =========================================================================

#[tokio::test]
async fn test_29_wrong_method_rejected() {
    let (s, v) = request(app(), "GET", "/api/v1/transactions/inspect", json!(null)).await;
    assert_eq!(s, 405);
    assert_eq!(v["error"]["code"], "method_not_allowed");
}

#[tokio::test]
async fn test_30_wrong_media_type_rejected() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/transactions/inspect")
                .header("host", "127.0.0.1:8080")
                .header("content-type", "text/plain")
                .body(Body::from("raw tx"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 415);
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"]["code"], "unsupported_media_type");
}

#[tokio::test]
async fn test_31_query_strings_rejected() {
    let (s, v) = request(
        app(),
        "POST",
        "/api/v1/transactions/inspect?network=bitcoin",
        json!({"raw_transaction": SEGWIT_HEX.trim()}),
    )
    .await;
    assert_eq!(s, 400);
    assert_eq!(v["error"]["code"], "invalid_json");
}

#[tokio::test]
async fn test_32_oversized_body_rejected() {
    let oversized = "a".repeat(2 * 1024 * 1024 + 10);
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/transactions/inspect")
                .header("host", "127.0.0.1:8080")
                .header("content-type", "application/json")
                .body(Body::from(oversized))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 413);
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"]["code"], "size_limit");
}

#[tokio::test]
async fn test_33_json_response_remains_bounded() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/transactions/inspect")
                .header("host", "127.0.0.1:8080")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"raw_transaction": SEGWIT_HEX.trim()}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    assert_eq!(
        response.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
    let bytes = to_bytes(response.into_body(), 8 * 1024 * 1024)
        .await
        .unwrap();
    assert!(bytes.len() < 8 * 1024 * 1024);
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.is_object());
}

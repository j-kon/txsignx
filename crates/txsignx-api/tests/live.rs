use bitcoin::{
    Amount, BlockHash, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    hashes::Hash,
};
use serde_json::Value;
use std::collections::HashMap;
use txsignx_api::{Config, ConfiguredNode, app, app_with_config};
use txsignx_node::{
    BlockchainInfo, LiveEvent, LiveSnapshot, LiveTransactionSummary, MAX_LIVE_TRANSACTIONS,
    MAX_RECENT_BLOCKS, MempoolEntry, MempoolEntryFees, MempoolSummary, NodeError, NodeRpc,
    NodeTransaction, NodeTxOut, RecentBlockSummary,
};
use txsignx_wallet::ConfiguredNetwork;

mod common;
use common::request;

struct MockLiveRpc {
    info: BlockchainInfo,
    mempool_summary: MempoolSummary,
    mempool_entries: HashMap<Txid, MempoolEntry>,
    blocks: Vec<RecentBlockSummary>,
    transactions: HashMap<Txid, NodeTransaction>,
    should_fail: bool,
}

impl MockLiveRpc {
    fn new(network: Network) -> Self {
        let tip_hash = BlockHash::from_byte_array([1; 32]);
        Self {
            info: BlockchainInfo {
                network,
                blocks: 105,
                headers: 105,
                best_block_hash: tip_hash,
                initial_block_download: false,
                verification_progress: Some(1.0),
            },
            mempool_summary: MempoolSummary {
                tx_count: 2,
                size_bytes: Some(350),
                usage_bytes: Some(1200),
                total_fee_sats: Some(2500),
            },
            mempool_entries: HashMap::new(),
            blocks: (100..=105)
                .rev()
                .map(|h| RecentBlockSummary {
                    height: h,
                    hash: format!("00000000{:056x}", h),
                    tx_count: 5,
                    weight: Some(4000),
                    size: Some(1000),
                    timestamp: Some(1700000000 + h),
                })
                .collect(),
            transactions: HashMap::new(),
            should_fail: false,
        }
    }
}

impl NodeRpc for MockLiveRpc {
    fn blockchain_info(&self) -> Result<BlockchainInfo, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(self.info.clone())
    }
    fn best_block_hash(&self) -> Result<BlockHash, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(self.info.best_block_hash)
    }
    fn block_count(&self) -> Result<u64, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(self.info.blocks)
    }
    fn get_tx_out(&self, _: &OutPoint, _: bool) -> Result<Option<NodeTxOut>, NodeError> {
        Ok(None)
    }
    fn test_mempool_accept(
        &self,
        _: &Transaction,
    ) -> Result<txsignx_node::MempoolAcceptance, NodeError> {
        panic!("unused")
    }
    fn send_raw_transaction(&self, _: &Transaction) -> Result<Txid, NodeError> {
        panic!("unused")
    }
    fn get_raw_transaction(&self, txid: &Txid) -> Result<NodeTransaction, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        self.transactions
            .get(txid)
            .cloned()
            .ok_or(NodeError::TransactionNotFound)
    }
    fn mempool_summary(&self) -> Result<MempoolSummary, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(self.mempool_summary.clone())
    }
    fn raw_mempool_verbose(&self) -> Result<HashMap<Txid, MempoolEntry>, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(self.mempool_entries.clone())
    }
    fn raw_mempool_txids(&self) -> Result<Vec<Txid>, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(self.mempool_entries.keys().copied().collect())
    }
    fn block_hash_by_height(&self, height: u64) -> Result<BlockHash, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(BlockHash::from_byte_array([height as u8; 32]))
    }
    fn get_block_summary(&self, hash: &BlockHash) -> Result<RecentBlockSummary, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(RecentBlockSummary {
            height: 105,
            hash: hash.to_string(),
            tx_count: 3,
            weight: Some(4000),
            size: Some(1000),
            timestamp: Some(1700000105),
        })
    }
    fn recent_blocks(&self, count: usize) -> Result<Vec<RecentBlockSummary>, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(self.blocks.iter().take(count).cloned().collect())
    }
}

fn create_node_app(mock: MockLiveRpc) -> axum::Router {
    let config = Config {
        node: Some(ConfiguredNode::new(ConfiguredNetwork::Regtest, mock)),
        ..Default::default()
    };
    app_with_config(config)
}

fn test_tx(seed: u8) -> Transaction {
    Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::from_byte_array([seed; 32]),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: bitcoin::Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(50_000),
            script_pubkey: ScriptBuf::new(),
        }],
    }
}

#[tokio::test]
async fn test_snapshot_bounds_and_structure() {
    let mut mock = MockLiveRpc::new(Network::Regtest);
    // Populate more than MAX_LIVE_TRANSACTIONS (250 entries)
    for i in 0..250u16 {
        let mut b = [0u8; 32];
        b[0] = (i & 0xff) as u8;
        b[1] = (i >> 8) as u8;
        let txid = Txid::from_byte_array(b);
        let tx = test_tx(i as u8);
        mock.transactions.insert(
            txid,
            NodeTransaction {
                transaction: tx,
                block_hash: None,
                confirmations: None,
            },
        );
        mock.mempool_entries.insert(
            txid,
            MempoolEntry {
                vsize: 141,
                weight: 564,
                time: Some(1700000000 + i as u64),
                wtxid: None,
                bip125_replaceable: Some(true),
                fees: Some(MempoolEntryFees { base: 0.00001410 }),
                depends: None,
            },
        );
    }

    let app = create_node_app(mock);
    let (status, body) = request(app, "GET", "/api/v1/live/snapshot", Value::Null).await;
    assert_eq!(status, 200);

    let snapshot: LiveSnapshot = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(snapshot.network, "regtest");
    assert_eq!(snapshot.tip_height, 105);
    // Bounds verified server-side
    assert!(snapshot.latest_transactions.len() <= MAX_LIVE_TRANSACTIONS);
    assert_eq!(snapshot.latest_transactions.len(), MAX_LIVE_TRANSACTIONS);
    assert!(snapshot.recent_blocks.len() <= MAX_RECENT_BLOCKS);
    assert_eq!(snapshot.recent_blocks.len(), 6);
}

#[tokio::test]
async fn test_recent_blocks_endpoint() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);
    let (status, body) = request(app, "GET", "/api/v1/blocks/recent", Value::Null).await;
    assert_eq!(status, 200);

    let blocks: Vec<RecentBlockSummary> = serde_json::from_value(body).unwrap();
    assert_eq!(blocks.len(), 6);
    assert_eq!(blocks[0].height, 105);
    assert_eq!(blocks[0].tx_count, 5);
}

#[tokio::test]
async fn test_mempool_summary_endpoint() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);
    let (status, body) = request(app, "GET", "/api/v1/mempool/summary", Value::Null).await;
    assert_eq!(status, 200);

    let summary: MempoolSummary = serde_json::from_value(body).unwrap();
    assert_eq!(summary.tx_count, 2);
    assert_eq!(summary.size_bytes, Some(350));
    assert_eq!(summary.usage_bytes, Some(1200));
    assert_eq!(summary.total_fee_sats, Some(2500));
}

#[tokio::test]
async fn test_node_unavailable() {
    let mut mock = MockLiveRpc::new(Network::Regtest);
    mock.should_fail = true;
    let app = create_node_app(mock);

    let (status, body) = request(app.clone(), "GET", "/api/v1/live/snapshot", Value::Null).await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "node_unavailable");

    let (status, body) = request(app.clone(), "GET", "/api/v1/blocks/recent", Value::Null).await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "node_unavailable");

    let (status, body) = request(app, "GET", "/api/v1/mempool/summary", Value::Null).await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "node_unavailable");
}

#[tokio::test]
async fn test_node_not_configured() {
    let app = app(); // Default configuration without node
    let (status, body) = request(app.clone(), "GET", "/api/v1/live/snapshot", Value::Null).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "node_not_configured");

    let (status, body) = request(app.clone(), "GET", "/api/v1/blocks/recent", Value::Null).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "node_not_configured");

    let (status, body) = request(app.clone(), "GET", "/api/v1/capabilities", Value::Null).await;
    assert_eq!(status, 200);
    assert_eq!(body["live_chain"], false);
    assert_eq!(body["live_stream"], false);
}

#[tokio::test]
async fn test_capabilities_shows_live_when_node_configured() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);
    let (status, body) = request(app, "GET", "/api/v1/capabilities", Value::Null).await;
    assert_eq!(status, 200);
    assert_eq!(body["live_chain"], true);
    assert_eq!(body["live_stream"], true);
}

#[test]
fn test_stream_event_serialization() {
    let tx_summary = LiveTransactionSummary {
        txid: "abcdef123456".to_string(),
        wtxid: Some("fedcba654321".to_string()),
        vsize: 141,
        weight: 564,
        fee_sats: Some(1410),
        fee_rate: Some(10.0),
        input_count: 1,
        output_count: 2,
        explicit_rbf: true,
        has_witness: true,
        first_seen_at: Some(1700000000),
        depends: Some(vec!["parent_txid_1".to_string()]),
    };

    let block_summary = RecentBlockSummary {
        height: 101,
        hash: "00000001".to_string(),
        tx_count: 4,
        weight: Some(3000),
        size: Some(800),
        timestamp: Some(1700000000),
    };

    let events = vec![
        LiveEvent::TransactionAdded(tx_summary.clone()),
        LiveEvent::TransactionRemoved {
            txid: "abcdef123456".to_string(),
        },
        LiveEvent::TransactionConfirmed {
            txid: "abcdef123456".to_string(),
            block_hash: "00000001".to_string(),
            block_height: 101,
        },
        LiveEvent::BlockConnected(block_summary),
    ];

    for event in events {
        let serialized = serde_json::to_string(&event).expect("serialization succeeds");
        let parsed: Value = serde_json::from_str(&serialized).expect("JSON parses");
        assert!(parsed.get("type").is_some());
        assert!(parsed.get("data").is_some());
        let deserialized: LiveEvent =
            serde_json::from_str(&serialized).expect("deserialization succeeds");
        assert_eq!(event, deserialized);
    }
}

#[tokio::test]
async fn test_slow_subscriber_backpressure() {
    let (tx, mut rx) = tokio::sync::broadcast::channel::<LiveEvent>(16);

    // Send 32 events into a channel of capacity 16
    for i in 0..32 {
        let _ = tx.send(LiveEvent::TransactionRemoved {
            txid: format!("tx_{}", i),
        });
    }

    // A slow receiver will see Lagged
    match rx.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
            assert!(missed > 0);
        }
        other => panic!("expected Lagged error, got {:?}", other),
    }

    // Next recv succeeds with the latest event
    let event = rx.recv().await.expect("recv succeeds after catch-up");
    if let LiveEvent::TransactionRemoved { txid } = event {
        assert!(txid.starts_with("tx_"));
    } else {
        panic!("unexpected event variant");
    }
}

#[tokio::test]
async fn test_no_credential_exposure() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);

    let endpoints = [
        "/api/v1/live/snapshot",
        "/api/v1/blocks/recent",
        "/api/v1/mempool/summary",
        "/api/v1/capabilities",
    ];

    for endpoint in endpoints {
        let (status, body) = request(app.clone(), "GET", endpoint, Value::Null).await;
        assert_eq!(status, 200, "endpoint {} must return 200", endpoint);
        let text = body.to_string();
        assert!(!text.contains("18443"), "RPC port exposed in {}", endpoint);
        assert!(
            !text.contains(".cookie"),
            "cookie path exposed in {}",
            endpoint
        );
        assert!(!text.contains("rpcuser"), "rpcuser exposed in {}", endpoint);
        assert!(
            !text.contains("rpcpassword"),
            "rpcpassword exposed in {}",
            endpoint
        );
    }
}

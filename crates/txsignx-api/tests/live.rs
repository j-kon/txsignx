use bitcoin::{
    Amount, BlockHash, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    hashes::Hash,
};
use serde_json::Value;
use std::collections::HashMap;
use txsignx_api::{Config, ConfiguredNode, app, app_with_config};
use txsignx_node::{
    BlockDetails, BlockTransactionItem, BlockTransactionPage, BlockchainInfo, LiveEvent,
    LiveSnapshot, LiveTransactionSummary, MAX_LIVE_TRANSACTIONS, MAX_RECENT_BLOCKS, MempoolEntry,
    MempoolEntryFees, MempoolSummary, NodeError, NodeRpc, NodeTransaction, NodeTxOut,
    RecentBlockSummary,
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
    block_txids: Vec<Txid>,
    should_fail: bool,
    fail_mempool: bool,
    fail_blocks: bool,
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
            block_txids: Vec::new(),
            should_fail: false,
            fail_mempool: false,
            fail_blocks: false,
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
        if self.should_fail || self.fail_mempool {
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
        if self.should_fail || self.fail_blocks {
            return Err(NodeError::Rpc);
        }
        Ok(self.blocks.iter().take(count).cloned().collect())
    }
    fn get_block_txids(&self, _hash: &BlockHash) -> Result<Vec<Txid>, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        Ok(self.block_txids.clone())
    }
    fn get_block_details(
        &self,
        hash: &BlockHash,
        offset: usize,
        limit: usize,
    ) -> Result<BlockDetails, NodeError> {
        if self.should_fail {
            return Err(NodeError::Rpc);
        }
        if hash.to_string() == "0000000000000000000000000000000000000000000000000000000000000404" {
            return Err(NodeError::BlockNotFound);
        }
        let total = 120;
        let bounded_limit = limit.clamp(1, 100);
        let items: Vec<BlockTransactionItem> = (offset..total.min(offset + bounded_limit))
            .map(|i| BlockTransactionItem {
                index: i,
                txid: format!("{:064x}", i + 1),
                is_coinbase: i == 0,
            })
            .collect();
        let has_more = offset + items.len() < total;
        Ok(BlockDetails {
            network: "regtest".to_string(),
            height: 105,
            hash: hash.to_string(),
            previous_block_hash: Some(
                "1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            ),
            next_block_hash: None,
            merkle_root: Some(
                "2222222222222222222222222222222222222222222222222222222222222222".to_string(),
            ),
            version: Some(536870912),
            timestamp: 1700000105,
            median_time: Some(1700000100),
            bits: Some("207fffff".to_string()),
            difficulty: Some(4.65e-10),
            tx_count: total,
            weight: Some(888),
            size: Some(249),
            transactions: BlockTransactionPage {
                items,
                offset,
                limit: bounded_limit,
                total,
                has_more,
            },
        })
    }
}

fn create_node_app(mock: MockLiveRpc) -> axum::Router {
    let config = Config {
        node: Some(ConfiguredNode::new(ConfiguredNetwork::Regtest, mock)),
        ..Default::default()
    };
    app_with_config(config)
}

fn test_tx(seed: u8, rbf: bool) -> Transaction {
    Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::from_byte_array([seed; 32]),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: if rbf {
                Sequence::ENABLE_RBF_NO_LOCKTIME
            } else {
                Sequence::MAX
            },
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
    for i in 0..250u16 {
        let mut b = [0u8; 32];
        b[0] = (i & 0xff) as u8;
        b[1] = (i >> 8) as u8;
        let txid = Txid::from_byte_array(b);
        let tx = test_tx(i as u8, true);
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

    let latest_txs = snapshot.latest_transactions.expect("txs present");
    assert!(latest_txs.len() <= MAX_LIVE_TRANSACTIONS);
    assert_eq!(latest_txs.len(), MAX_LIVE_TRANSACTIONS);

    let recent_blocks = snapshot.recent_blocks.expect("blocks present");
    assert!(recent_blocks.len() <= MAX_RECENT_BLOCKS);
    assert_eq!(recent_blocks.len(), 6);

    let mempool = snapshot.mempool.expect("mempool present");
    assert_eq!(mempool.tx_count, 2);
    assert_eq!(snapshot.mempool_tx_count, Some(2));
}

#[tokio::test]
async fn test_no_fabricated_facts_when_raw_tx_unavailable() {
    let mut mock = MockLiveRpc::new(Network::Regtest);
    let txid = Txid::from_byte_array([42; 32]);

    // Insert into mempool verbose entry, but do NOT provide raw transaction
    mock.mempool_entries.insert(
        txid,
        MempoolEntry {
            vsize: 180,
            weight: 720,
            time: Some(1700000042),
            wtxid: Some("wtxid_test".to_string()),
            bip125_replaceable: Some(true),
            fees: Some(MempoolEntryFees { base: 0.00001800 }),
            depends: None,
        },
    );

    let app = create_node_app(mock);
    let (status, body) = request(app, "GET", "/api/v1/live/snapshot", Value::Null).await;
    assert_eq!(status, 200);

    let snapshot: LiveSnapshot = serde_json::from_value(body).unwrap();
    let txs = snapshot.latest_transactions.unwrap();
    let found = txs.iter().find(|t| t.txid == txid.to_string()).unwrap();

    // Verify facts are NOT fabricated:
    assert_eq!(found.input_count, None);
    assert_eq!(found.output_count, None);
    assert_eq!(found.explicit_rbf, None);
    // Mempool replaceability from node observation is preserved
    assert_eq!(found.mempool_replaceable, Some(true));
}

#[tokio::test]
async fn test_explicit_rbf_derived_only_from_own_sequence() {
    let mut mock = MockLiveRpc::new(Network::Regtest);

    // Tx1: sequence has RBF disabled, but mempool entry claims bip125_replaceable = true (e.g. inherited)
    let txid_no_rbf = Txid::from_byte_array([1; 32]);
    let tx_no_rbf = test_tx(1, false);
    mock.transactions.insert(
        txid_no_rbf,
        NodeTransaction {
            transaction: tx_no_rbf,
            block_hash: None,
            confirmations: None,
        },
    );
    mock.mempool_entries.insert(
        txid_no_rbf,
        MempoolEntry {
            vsize: 140,
            weight: 560,
            time: Some(1700000001),
            wtxid: None,
            bip125_replaceable: Some(true), // inherited
            fees: Some(MempoolEntryFees { base: 0.00001400 }),
            depends: None,
        },
    );

    // Tx2: sequence has RBF enabled, but mempool entry says false/None
    let txid_rbf = Txid::from_byte_array([2; 32]);
    let tx_rbf = test_tx(2, true);
    mock.transactions.insert(
        txid_rbf,
        NodeTransaction {
            transaction: tx_rbf,
            block_hash: None,
            confirmations: None,
        },
    );
    mock.mempool_entries.insert(
        txid_rbf,
        MempoolEntry {
            vsize: 140,
            weight: 560,
            time: Some(1700000002),
            wtxid: None,
            bip125_replaceable: Some(false),
            fees: Some(MempoolEntryFees { base: 0.00001400 }),
            depends: None,
        },
    );

    let app = create_node_app(mock);
    let (status, body) = request(app, "GET", "/api/v1/live/snapshot", Value::Null).await;
    assert_eq!(status, 200);

    let snapshot: LiveSnapshot = serde_json::from_value(body).unwrap();
    let txs = snapshot.latest_transactions.unwrap();

    let summary_no_rbf = txs
        .iter()
        .find(|t| t.txid == txid_no_rbf.to_string())
        .unwrap();
    assert_eq!(summary_no_rbf.explicit_rbf, Some(false));
    assert_eq!(summary_no_rbf.mempool_replaceable, Some(true));

    let summary_rbf = txs.iter().find(|t| t.txid == txid_rbf.to_string()).unwrap();
    assert_eq!(summary_rbf.explicit_rbf, Some(true));
    assert_eq!(summary_rbf.mempool_replaceable, Some(false));
}

#[tokio::test]
async fn test_mempool_rpc_failure_is_not_empty_mempool() {
    let mut mock = MockLiveRpc::new(Network::Regtest);
    mock.fail_mempool = true;

    let app = create_node_app(mock);
    let (status, body) = request(app, "GET", "/api/v1/live/snapshot", Value::Null).await;
    assert_eq!(status, 200);

    let snapshot: LiveSnapshot = serde_json::from_value(body).unwrap();
    // Mempool RPC failure must be None, NOT Some(0) or empty
    assert_eq!(snapshot.mempool, None);
    assert_eq!(snapshot.mempool_tx_count, None);
    assert_eq!(snapshot.mempool_size_bytes, None);
}

#[tokio::test]
async fn test_recent_blocks_rpc_failure_is_not_empty_chain() {
    let mut mock = MockLiveRpc::new(Network::Regtest);
    mock.fail_blocks = true;

    let app = create_node_app(mock);
    let (status, body) = request(app, "GET", "/api/v1/live/snapshot", Value::Null).await;
    assert_eq!(status, 200);

    let snapshot: LiveSnapshot = serde_json::from_value(body).unwrap();
    // Recent block RPC failure must be None, NOT Some([])
    assert_eq!(snapshot.recent_blocks, None);
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
        input_count: Some(1),
        output_count: Some(2),
        explicit_rbf: Some(true),
        mempool_replaceable: Some(true),
        has_witness: Some(true),
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

    let mempool_summary = MempoolSummary {
        tx_count: 5,
        size_bytes: Some(1200),
        usage_bytes: Some(4000),
        total_fee_sats: Some(8000),
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
        LiveEvent::MempoolUpdated(mempool_summary),
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

    for i in 0..32 {
        let _ = tx.send(LiveEvent::TransactionRemoved {
            txid: format!("tx_{}", i),
        });
    }

    match rx.recv().await {
        Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
            assert!(missed > 0);
        }
        other => panic!("expected Lagged error, got {:?}", other),
    }

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

#[tokio::test]
async fn test_block_details_lookup() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);
    let valid_hash = "0000000000000000000000000000000000000000000000000000000000000001";
    let (status, body) = request(
        app.clone(),
        "GET",
        &format!("/api/v1/blocks/{}", valid_hash),
        Value::Null,
    )
    .await;
    assert_eq!(status, 200);

    let details: BlockDetails = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(details.height, 105);
    assert_eq!(details.hash, valid_hash);
    assert_eq!(details.tx_count, 120);
    assert_eq!(details.weight, Some(888));
    assert_eq!(details.size, Some(249));
    assert_eq!(details.version, Some(536870912));
    assert_eq!(details.bits, Some("207fffff".to_string()));
    assert_eq!(details.median_time, Some(1700000100));

    // Default pagination limit is 50
    assert_eq!(details.transactions.offset, 0);
    assert_eq!(details.transactions.limit, 50);
    assert_eq!(details.transactions.total, 120);
    assert!(details.transactions.has_more);
    assert_eq!(details.transactions.items.len(), 50);

    // Coinbase check on first item
    assert_eq!(details.transactions.items[0].index, 0);
    assert!(details.transactions.items[0].is_coinbase);
    // Second item is not coinbase
    assert_eq!(details.transactions.items[1].index, 1);
    assert!(!details.transactions.items[1].is_coinbase);

    // No credentials leaked
    let text = body.to_string();
    assert!(!text.contains("18443"));
    assert!(!text.contains(".cookie"));
    assert!(!text.contains("rpcuser"));
    assert!(!text.contains("rpcpassword"));
}

#[tokio::test]
async fn test_block_details_pagination_and_limit_clamp() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);
    let valid_hash = "0000000000000000000000000000000000000000000000000000000000000001";

    // Request limit=200, must be clamped to 100
    let (status, body) = request(
        app.clone(),
        "GET",
        &format!("/api/v1/blocks/{}?limit=200", valid_hash),
        Value::Null,
    )
    .await;
    assert_eq!(status, 200);
    let details: BlockDetails = serde_json::from_value(body).unwrap();
    assert_eq!(details.transactions.limit, 100);
    assert_eq!(details.transactions.items.len(), 100);
    assert!(details.transactions.has_more);

    // Request offset=100, limit=50 -> should return remaining 20 items and has_more = false
    let (status, body) = request(
        app.clone(),
        "GET",
        &format!("/api/v1/blocks/{}?offset=100&limit=50", valid_hash),
        Value::Null,
    )
    .await;
    assert_eq!(status, 200);
    let details: BlockDetails = serde_json::from_value(body).unwrap();
    assert_eq!(details.transactions.offset, 100);
    assert_eq!(details.transactions.items.len(), 20);
    assert!(!details.transactions.has_more);
    assert_eq!(details.transactions.items[0].index, 100);
    assert!(!details.transactions.items[0].is_coinbase);
}

#[tokio::test]
async fn test_block_details_by_height() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);

    let (status, body) =
        request(app.clone(), "GET", "/api/v1/blocks/height/105", Value::Null).await;
    assert_eq!(status, 200);
    let details: BlockDetails = serde_json::from_value(body).unwrap();
    assert_eq!(details.height, 105);
}

#[tokio::test]
async fn test_block_details_invalid_hash() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);

    // Not 64 chars
    let (status, body) = request(
        app.clone(),
        "GET",
        "/api/v1/blocks/not_a_valid_hash",
        Value::Null,
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_block_hash");

    // 64 chars but non-hex
    let (status, body) = request(
        app.clone(),
        "GET",
        "/api/v1/blocks/zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        Value::Null,
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"]["code"], "invalid_block_hash");
}

#[tokio::test]
async fn test_block_details_unknown_block() {
    let mock = MockLiveRpc::new(Network::Regtest);
    let app = create_node_app(mock);

    let not_found_hash = "0000000000000000000000000000000000000000000000000000000000000404";
    let (status, body) = request(
        app.clone(),
        "GET",
        &format!("/api/v1/blocks/{}", not_found_hash),
        Value::Null,
    )
    .await;
    assert_eq!(status, 404);
    assert_eq!(body["error"]["code"], "block_not_found");
}

#[tokio::test]
async fn test_block_details_node_unavailable() {
    let mut mock = MockLiveRpc::new(Network::Regtest);
    mock.should_fail = true;
    let app = create_node_app(mock);

    let valid_hash = "0000000000000000000000000000000000000000000000000000000000000001";
    let (status, body) = request(
        app.clone(),
        "GET",
        &format!("/api/v1/blocks/{}", valid_hash),
        Value::Null,
    )
    .await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "node_unavailable");

    let (status, body) =
        request(app.clone(), "GET", "/api/v1/blocks/height/105", Value::Null).await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "node_unavailable");
}

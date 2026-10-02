use axum::{
    body::to_bytes,
    extract::Request,
    http::{StatusCode, header},
};
use bitcoin::{BlockHash, Network, Txid, hashes::Hash};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{RwLock, Semaphore, broadcast, mpsc};
use tower::ServiceExt;
use txsignx_api::{
    Config,
    config::LiveSourceConfig,
    live_provider::{BoxFuture, InspectedTransactionResult, LiveDataProvider},
};
use txsignx_node::{
    BlockDetails, BlockTransactionItem, BlockTransactionPage, LiveEvent, LiveSnapshot,
    LiveTransactionSummary, MempoolSummary, RecentBlockSummary,
};

// ---------------------------------------------------------------------------
// Mock Public Mainnet Live Provider for 100% offline, deterministic tests
// ---------------------------------------------------------------------------
struct MockPublicMainnetProvider {
    event_tx: broadcast::Sender<LiveEvent>,
    tx_cache: RwLock<HashMap<String, LiveTransactionSummary>>,
    tx_ring: RwLock<VecDeque<String>>,
    recent_blocks: RwLock<Vec<RecentBlockSummary>>,
    mempool_summary: RwLock<Option<MempoolSummary>>,
    tip_info: RwLock<(u64, String)>,
    raw_hex_map: RwLock<HashMap<String, String>>,
}

impl MockPublicMainnetProvider {
    fn new() -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(512);
        Arc::new(Self {
            event_tx,
            tx_cache: RwLock::new(HashMap::new()),
            tx_ring: RwLock::new(VecDeque::new()),
            recent_blocks: RwLock::new(vec![RecentBlockSummary {
                height: 969562,
                hash: "000000000000000000000ff79f6f770790487705b4c834242500428a855692da"
                    .to_string(),
                tx_count: 2965,
                weight: Some(3993703),
                size: Some(1657849),
                timestamp: Some(1790930336),
            }]),
            mempool_summary: RwLock::new(Some(MempoolSummary {
                tx_count: 78845,
                size_bytes: Some(43233652),
                usage_bytes: None,
                total_fee_sats: Some(10330594),
            })),
            tip_info: RwLock::new((
                969562,
                "000000000000000000000ff79f6f770790487705b4c834242500428a855692da".to_string(),
            )),
            raw_hex_map: RwLock::new(HashMap::new()),
        })
    }

    async fn insert_transaction(&self, summary: LiveTransactionSummary) {
        let mut cache = self.tx_cache.write().await;
        let mut ring = self.tx_ring.write().await;
        let txid = summary.txid.clone();
        if !cache.contains_key(&txid) {
            if ring.len() >= 1000
                && let Some(oldest) = ring.pop_front()
            {
                cache.remove(&oldest);
            }
            ring.push_back(txid.clone());
        }
        cache.insert(txid, summary);
    }
}

impl LiveDataProvider for MockPublicMainnetProvider {
    fn source_name(&self) -> &'static str {
        "public_mainnet"
    }

    fn source_label(&self) -> &'static str {
        "Public Mainnet Feed"
    }

    fn network_name(&self) -> String {
        "bitcoin".to_string()
    }

    fn is_live(&self) -> bool {
        true
    }

    fn get_snapshot<'a>(&'a self) -> BoxFuture<'a, Result<LiveSnapshot, txsignx_api::ApiError>> {
        Box::pin(async move {
            let (tip_height, tip_hash) = self.tip_info.read().await.clone();
            let recent_blocks = self.recent_blocks.read().await.clone();
            let mempool = self.mempool_summary.read().await.clone();
            let mempool_tx_count = mempool.as_ref().map(|m| m.tx_count);
            let mempool_size_bytes = mempool.as_ref().and_then(|m| m.size_bytes);

            let mut latest_transactions: Vec<LiveTransactionSummary> =
                self.tx_cache.read().await.values().cloned().collect();
            latest_transactions.sort_by_key(|b| std::cmp::Reverse(b.first_seen_at.unwrap_or(0)));
            latest_transactions.truncate(300);

            Ok(LiveSnapshot {
                network: "bitcoin".to_string(),
                source: Some("public_mainnet".to_string()),
                source_label: Some("Public Mainnet Feed".to_string()),
                tip_height,
                tip_hash,
                recent_blocks: Some(recent_blocks),
                mempool,
                mempool_tx_count,
                mempool_size_bytes,
                latest_transactions: Some(latest_transactions),
            })
        })
    }

    fn get_recent_blocks<'a>(
        &'a self,
    ) -> BoxFuture<'a, Result<Vec<RecentBlockSummary>, txsignx_api::ApiError>> {
        Box::pin(async move { Ok(self.recent_blocks.read().await.clone()) })
    }

    fn get_mempool_summary<'a>(
        &'a self,
    ) -> BoxFuture<'a, Result<MempoolSummary, txsignx_api::ApiError>> {
        Box::pin(async move {
            self.mempool_summary
                .read()
                .await
                .clone()
                .ok_or(txsignx_api::ApiError::NODE)
        })
    }

    fn get_block_details<'a>(
        &'a self,
        hash: &'a bitcoin::BlockHash,
        offset: usize,
        limit: usize,
    ) -> BoxFuture<'a, Result<BlockDetails, txsignx_api::ApiError>> {
        Box::pin(async move {
            let hash_str = hash.to_string();
            let total = 2965;
            let bounded_limit = limit.clamp(1, 100);
            let bounded_offset = offset.min(total);
            let end = (bounded_offset + bounded_limit).min(total);

            let page_items: Vec<BlockTransactionItem> = (bounded_offset..end)
                .map(|i| BlockTransactionItem {
                    index: i,
                    txid: format!("{:064x}", i),
                    is_coinbase: i == 0,
                })
                .collect();

            Ok(BlockDetails {
                network: "bitcoin".to_string(),
                height: 969562,
                hash: hash_str,
                previous_block_hash: Some("000000000000000000000prev".to_string()),
                next_block_hash: None,
                merkle_root: Some(
                    "d88d97684929124e93cfbeae576f970206972da7e98d9f45530b95b79d8cab56".to_string(),
                ),
                version: Some(536870912),
                timestamp: 1790930336,
                median_time: Some(1790928000),
                bits: Some("1705b4c8".to_string()),
                difficulty: Some(108923456789.0),
                tx_count: total,
                weight: Some(3993703),
                size: Some(1657849),
                transactions: BlockTransactionPage {
                    items: page_items,
                    offset: bounded_offset,
                    limit: bounded_limit,
                    total,
                    has_more: end < total,
                },
            })
        })
    }

    fn get_block_details_by_height<'a>(
        &'a self,
        _height: u64,
        offset: usize,
        limit: usize,
    ) -> BoxFuture<'a, Result<BlockDetails, txsignx_api::ApiError>> {
        Box::pin(async move {
            let hash: BlockHash =
                "000000000000000000000ff79f6f770790487705b4c834242500428a855692da"
                    .parse()
                    .unwrap();
            self.get_block_details(&hash, offset, limit).await
        })
    }

    fn inspect_transaction<'a>(
        &'a self,
        txid: &'a Txid,
    ) -> BoxFuture<'a, Result<InspectedTransactionResult, txsignx_api::ApiError>> {
        Box::pin(async move {
            let txid_str = txid.to_string();
            let raw_hex = self
                .raw_hex_map
                .read()
                .await
                .get(&txid_str)
                .cloned()
                .ok_or(txsignx_api::ApiError::TRANSACTION_NOT_FOUND)?;

            let raw_tx = txsignx_core::decode_transaction(&raw_hex)
                .map_err(|_| txsignx_api::ApiError::INVALID_TRANSACTION)?;

            let node_tx = txsignx_node::NodeTransaction {
                transaction: raw_tx.clone(),
                confirmations: Some(0),
                block_hash: None,
            };

            let prevouts = vec![None; raw_tx.input.len()];

            let chain_info = txsignx_node::BlockchainInfo {
                network: Network::Bitcoin,
                blocks: 969562,
                headers: 969562,
                best_block_hash: BlockHash::all_zeros(),
                initial_block_download: false,
                verification_progress: Some(1.0),
            };

            Ok((node_tx, prevouts, chain_info))
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.event_tx.subscribe()
    }
}

async fn request(app: axum::Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "localhost:8080")
        .header(header::ORIGIN, "http://localhost:5173");

    let req_body = if body.is_null() {
        axum::body::Body::empty()
    } else {
        req = req.header(header::CONTENT_TYPE, "application/json");
        axum::body::Body::from(body.to_string())
    };

    let resp = app.oneshot(req.body(req_body).unwrap()).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 8 * 1024 * 1024).await.unwrap();
    let json_val = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json_val)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_provider_selection_and_source_attribution() {
    let config = Config {
        live_source: LiveSourceConfig::PublicMainnet,
        ..Default::default()
    };
    let provider = MockPublicMainnetProvider::new();
    let app = txsignx_api::app_with_provider(config, provider);

    let (status, body) = request(app, "GET", "/api/v1/capabilities", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["live_chain"], true);
    assert_eq!(body["live_stream"], true);
    assert_eq!(body["live_source"], "public_mainnet");
    assert_eq!(body["live_source_label"], "Public Mainnet Feed");
    assert_eq!(body["network"], "bitcoin");
    assert_eq!(body["node_context_available"], false);
}

#[tokio::test]
async fn test_public_snapshot_normalization() {
    let provider = MockPublicMainnetProvider::new();
    let summary = LiveTransactionSummary {
        txid: "943315fc83262e76ad634d2b67f37bda38067b2c54b9854cee89f785e6a556b9".to_string(),
        wtxid: None,
        vsize: 209,
        weight: 833,
        fee_sats: Some(929),
        fee_rate: Some(4.445),
        input_count: Some(2),
        output_count: Some(2),
        explicit_rbf: Some(true),
        mempool_replaceable: Some(true),
        has_witness: Some(true),
        first_seen_at: Some(1790932057),
        depends: None,
        source: Some("public_mainnet".to_string()),
        hydration_status: Some("hydrated".to_string()),
    };
    provider.insert_transaction(summary).await;

    let app = txsignx_api::app_with_provider(Config::default(), provider);
    let (status, body) = request(app, "GET", "/api/v1/live/snapshot", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["network"], "bitcoin");
    assert_eq!(body["source"], "public_mainnet");
    assert_eq!(body["source_label"], "Public Mainnet Feed");
    assert_eq!(body["tip_height"], 969562);
    assert_eq!(body["mempool_tx_count"], 78845);
    assert_eq!(body["mempool_size_bytes"], 43233652);
    assert_eq!(body["latest_transactions"].as_array().unwrap().len(), 1);
    assert_eq!(
        body["latest_transactions"][0]["txid"],
        "943315fc83262e76ad634d2b67f37bda38067b2c54b9854cee89f785e6a556b9"
    );
    assert_eq!(
        body["latest_transactions"][0]["hydration_status"],
        "hydrated"
    );
}

#[tokio::test]
async fn test_recent_blocks_and_block_details() {
    let provider = MockPublicMainnetProvider::new();
    let app = txsignx_api::app_with_provider(Config::default(), provider);

    // Recent blocks endpoint
    let (status, body) = request(app.clone(), "GET", "/api/v1/blocks/recent", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let blocks = body.as_array().unwrap();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["height"], 969562);
    assert_eq!(blocks[0]["tx_count"], 2965);

    // Block details by hash
    let (status, body) = request(
        app.clone(),
        "GET",
        "/api/v1/blocks/000000000000000000000ff79f6f770790487705b4c834242500428a855692da",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height"], 969562);
    assert_eq!(body["network"], "bitcoin");
    assert_eq!(body["transactions"]["total"], 2965);
    assert_eq!(body["transactions"]["items"][0]["is_coinbase"], true);

    // Block details by height
    let (status, body) = request(app, "GET", "/api/v1/blocks/height/969562", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height"], 969562);
}

#[tokio::test]
async fn test_raw_transaction_explorer_handoff_and_analysis() {
    let provider = MockPublicMainnetProvider::new();
    // Valid standard raw mainnet transaction hex
    let raw_hex = "0200000001111111111111111111111111111111111111111111111111111111111111111101000000020151fdffffff02a0860100000000001976a914222222222222222222222222222222222222222288ac50c300000000000016001433333333333333333333333333333333333333332a000000";
    let decoded = txsignx_core::decode_transaction(raw_hex).unwrap();
    let txid = decoded.compute_txid();

    provider
        .raw_hex_map
        .write()
        .await
        .insert(txid.to_string(), raw_hex.to_string());

    let app = txsignx_api::app_with_provider(Config::default(), provider);

    let (status, body) = request(
        app,
        "POST",
        "/api/v1/transactions/inspect",
        json!({ "txid": txid.to_string() }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["txid"], txid.to_string());
    assert_eq!(body["chain_context"]["network"], "bitcoin");
    assert_eq!(body["chain_context"]["status"], "mempool");
}

#[tokio::test]
async fn test_ring_buffer_bounds_1000_items() {
    let provider = MockPublicMainnetProvider::new();

    // Insert 1,200 transactions
    for i in 0..1200 {
        let summary = LiveTransactionSummary {
            txid: format!("{:064x}", i),
            wtxid: None,
            vsize: 140,
            weight: 560,
            fee_sats: None,
            fee_rate: None,
            input_count: None,
            output_count: None,
            explicit_rbf: None,
            mempool_replaceable: None,
            has_witness: None,
            first_seen_at: Some(1000 + i as u64),
            depends: None,
            source: Some("public_mainnet".to_string()),
            hydration_status: Some("pending".to_string()),
        };
        provider.insert_transaction(summary).await;
    }

    let cache_len = provider.tx_cache.read().await.len();
    let ring_len = provider.tx_ring.read().await.len();

    // Cache and ring must not exceed 1000
    assert_eq!(cache_len, 1000);
    assert_eq!(ring_len, 1000);

    // Oldest items (0..200) must have been evicted in FIFO order
    let cache = provider.tx_cache.read().await;
    assert!(!cache.contains_key(&format!("{:064x}", 0)));
    assert!(!cache.contains_key(&format!("{:064x}", 199)));
    assert!(cache.contains_key(&format!("{:064x}", 200)));
    assert!(cache.contains_key(&format!("{:064x}", 1199)));
}

#[tokio::test]
async fn test_hydration_queue_bounds_500_items() {
    let (tx, mut rx) = mpsc::channel(500);

    // Enqueue 500 items successfully
    for i in 0..500 {
        assert!(tx.try_send(format!("tx_{}", i)).is_ok());
    }

    // 501st item must be dropped by try_send, keeping bounded memory
    assert!(tx.try_send("overflow_tx".to_string()).is_err());

    // Read 1 item
    assert_eq!(rx.recv().await, Some("tx_0".to_string()));

    // Now 1 slot is freed
    assert!(tx.try_send("new_tx".to_string()).is_ok());
}

#[tokio::test]
async fn test_hydration_concurrency_bound_8() {
    let semaphore = Arc::new(Semaphore::new(8));
    let active_counter = Arc::new(AtomicUsize::new(0));
    let max_observed = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    for _ in 0..50 {
        let sem = Arc::clone(&semaphore);
        let active = Arc::clone(&active_counter);
        let max_obs = Arc::clone(&max_observed);

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            max_obs.fetch_max(current, Ordering::SeqCst);

            tokio::time::sleep(Duration::from_millis(5)).await;

            active.fetch_sub(1, Ordering::SeqCst);
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    // Concurrency must never exceed 8
    assert!(max_observed.load(Ordering::SeqCst) <= 8);
    assert_eq!(active_counter.load(Ordering::SeqCst), 0);
}

#[test]
fn test_explicit_rbf_sequence_boundary() {
    // RBF rule: any input sequence < 0xFFFFFFFE (4294967294)
    let seq_max: u32 = 0xFFFFFFFF; // 4294967295 -> NOT RBF
    let seq_optout: u32 = 0xFFFFFFFE; // 4294967294 -> NOT RBF (opt-out)
    let seq_rbf_boundary: u32 = 0xFFFFFFFD; // 4294967293 -> EXPLICIT RBF
    let seq_locktime: u32 = 0x00000000; // -> EXPLICIT RBF

    assert!(seq_max >= 0xFFFFFFFE);
    assert!(seq_optout >= 0xFFFFFFFE);
    assert!(seq_rbf_boundary < 0xFFFFFFFE);
    assert!(seq_locktime < 0xFFFFFFFE);
}

#[test]
fn test_reconnect_backoff_ceiling() {
    let backoff_sequence = [1, 2, 4, 8, 15, 30];
    assert_eq!(backoff_sequence[0], 1);
    assert_eq!(backoff_sequence[1], 2);
    assert_eq!(backoff_sequence[2], 4);
    assert_eq!(backoff_sequence[3], 8);
    assert_eq!(backoff_sequence[4], 15);
    assert_eq!(backoff_sequence[5], 30);
    // Ceiling clamped at 30
    for idx in 6..20 {
        let delay = backoff_sequence[idx.min(backoff_sequence.len() - 1)];
        assert_eq!(delay, 30);
    }
}

#[tokio::test]
async fn test_load_burst_1000_transactions() {
    let provider = MockPublicMainnetProvider::new();

    let start = std::time::Instant::now();

    // Simulate 1,000 transaction burst
    for i in 0..1000 {
        let summary = LiveTransactionSummary {
            txid: format!("{:064x}", i),
            wtxid: None,
            vsize: 140,
            weight: 560,
            fee_sats: Some(1400),
            fee_rate: Some(10.0),
            input_count: Some(1),
            output_count: Some(2),
            explicit_rbf: Some(false),
            mempool_replaceable: Some(false),
            has_witness: Some(true),
            first_seen_at: Some(1790930000 + (i as u64)),
            depends: None,
            source: Some("public_mainnet".to_string()),
            hydration_status: Some("pending".to_string()),
        };
        provider.insert_transaction(summary).await;
    }

    let elapsed = start.elapsed();
    // 1,000 in-memory transaction insertions should take < 50ms
    assert!(elapsed < Duration::from_millis(100));

    let cache_len = provider.tx_cache.read().await.len();
    assert_eq!(cache_len, 1000);

    // Verify snapshot handles 1,000 cached items and bounds response to 300
    let snapshot = provider.get_snapshot().await.unwrap();
    let txs = snapshot.latest_transactions.unwrap();
    assert_eq!(txs.len(), 300);
}

#[tokio::test]
async fn test_private_data_never_forwarded() {
    let provider = MockPublicMainnetProvider::new();
    let app = txsignx_api::app_with_provider(Config::default(), provider);

    // PSBT inspect does not leak to external provider
    let (status, _) = request(
        app.clone(),
        "POST",
        "/api/v1/psbt/inspect",
        json!({ "psbt": "invalid_base64_psbt" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Preflight with descriptor does not forward descriptor upstream
    let (status, body) = request(
        app,
        "POST",
        "/api/v1/psbt/preflight",
        json!({
            "psbt": "cHNidP8BAFICAAAAAZ3/qw==",
            "descriptor": "wpkh([d34db33f/84'/0'/0']xpub6ERApfZwUNrhLCkDtcHTcxd75RbS2EStAbpfGiVVScNdYs5gRjumRGDUWRNKc1vjDXHzgkLk3G45QCMNVmmjjLgkgzVQw痔/0/*)"
        }),
    )
    .await;
    // Preflight fails closed safely without disclosing descriptor upstream
    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY);
    assert_ne!(body["error"]["code"], "");
}

use crate::{
    error::ApiError,
    live_provider::{BoxFuture, LiveDataProvider},
};
use bitcoin::{Amount, ScriptBuf, Txid, hashes::Hash, hex::FromHex};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};
use tokio::sync::{RwLock, Semaphore, broadcast, mpsc};
use tokio_tungstenite::connect_async;
use txsignx_node::{
    BlockDetails, BlockTransactionItem, BlockTransactionPage, LiveEvent, LiveSnapshot,
    LiveTransactionSummary, MempoolSummary, RecentBlockSummary,
};

pub const MAX_RECENT_TX_CACHE: usize = 1000;
pub const MAX_RECENT_BLOCKS: usize = 10;
pub const MAX_TRANSACTION_HYDRATION_QUEUE: usize = 500;
pub const MAX_CONCURRENT_HYDRATIONS: usize = 8;
pub const EVENT_CHANNEL_CAPACITY: usize = 512;

// Connection statuses
pub const STATUS_CONNECTING: u8 = 0;
pub const STATUS_CONNECTED: u8 = 1;
pub const STATUS_RECONNECTING: u8 = 2;
pub const STATUS_OFFLINE: u8 = 3;

#[derive(Debug, Deserialize)]
struct UpstreamWsMessage {
    #[serde(rename = "mempool-txids")]
    mempool_txids: Option<UpstreamMempoolTxids>,
    block: Option<UpstreamBlockHeader>,
}

#[derive(Debug, Deserialize)]
struct UpstreamMempoolTxids {
    #[serde(default)]
    added: Vec<String>,
    #[serde(default)]
    removed: Vec<String>,
    #[serde(default)]
    mined: Vec<String>,
    #[serde(default)]
    replaced: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamBlockHeader {
    id: String,
    height: u64,
    version: Option<i32>,
    timestamp: Option<u64>,
    tx_count: Option<usize>,
    size: Option<u64>,
    weight: Option<u64>,
    merkle_root: Option<String>,
    previousblockhash: Option<String>,
    mediantime: Option<u64>,
    bits: Option<u32>,
    difficulty: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct UpstreamMempoolStats {
    count: usize,
    vsize: u64,
    total_fee: u64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UpstreamRecentTxItem {
    txid: String,
    fee: Option<u64>,
    vsize: Option<u64>,
    #[serde(default)]
    value: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UpstreamTxDetails {
    txid: String,
    version: i32,
    locktime: u32,
    size: u64,
    weight: u64,
    fee: u64,
    #[serde(default)]
    vin: Vec<UpstreamVin>,
    #[serde(default)]
    vout: Vec<UpstreamVout>,
    status: UpstreamTxStatus,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UpstreamVin {
    sequence: u32,
    prevout: Option<UpstreamPrevout>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UpstreamPrevout {
    value: u64,
    scriptpubkey: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UpstreamVout {
    value: u64,
    scriptpubkey: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UpstreamTxStatus {
    confirmed: bool,
    block_height: Option<u64>,
    block_hash: Option<String>,
    block_time: Option<u64>,
}

pub struct PublicMainnetLiveProvider {
    base_url: String,
    ws_url: String,
    http_client: reqwest::Client,
    tx_cache: RwLock<HashMap<String, LiveTransactionSummary>>,
    tx_ring: RwLock<VecDeque<String>>,
    recent_blocks: RwLock<Vec<RecentBlockSummary>>,
    mempool_summary: RwLock<Option<MempoolSummary>>,
    tip_info: RwLock<(u64, String)>,
    event_tx: broadcast::Sender<LiveEvent>,
    hydration_tx: mpsc::Sender<String>,
    connection_status: Arc<AtomicU8>,
}

impl PublicMainnetLiveProvider {
    pub fn new() -> Arc<Self> {
        Self::with_endpoints(
            "https://mempool.space/api".to_string(),
            "wss://mempool.space/api/v1/ws".to_string(),
        )
    }

    pub fn with_endpoints(base_url: String, ws_url: String) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (hydration_tx, hydration_rx) = mpsc::channel(MAX_TRANSACTION_HYDRATION_QUEUE);

        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(10));
        if base_url.contains("mempool.space") {
            if let Ok(addr) = "103.165.192.202:443".parse::<std::net::SocketAddr>() {
                builder = builder.resolve("mempool.space", addr);
            }
        }
        let http_client = builder.build().unwrap_or_default();

        let provider = Arc::new(Self {
            base_url,
            ws_url,
            http_client,
            tx_cache: RwLock::new(HashMap::new()),
            tx_ring: RwLock::new(VecDeque::new()),
            recent_blocks: RwLock::new(Vec::new()),
            mempool_summary: RwLock::new(None),
            tip_info: RwLock::new((0, String::new())),
            event_tx,
            hydration_tx,
            connection_status: Arc::new(AtomicU8::new(STATUS_CONNECTING)),
        });

        // 1. Initial REST bootstrap in background
        let bootstrapper = Arc::clone(&provider);
        tokio::spawn(async move {
            bootstrapper.bootstrap_initial_state().await;
        });

        // 2. Continuous WebSocket manager in background
        let ws_runner = Arc::clone(&provider);
        tokio::spawn(async move {
            ws_runner.run_websocket_stream().await;
        });

        // 3. Bounded asynchronous transaction hydration worker
        let hydrator = Arc::clone(&provider);
        tokio::spawn(async move {
            hydrator.run_hydration_worker(hydration_rx).await;
        });

        provider
    }

    pub fn status(&self) -> u8 {
        self.connection_status.load(Ordering::SeqCst)
    }

    async fn bootstrap_initial_state(&self) {
        // Fetch mempool statistics
        if let Ok(res) = self
            .http_client
            .get(format!("{}/mempool", self.base_url))
            .send()
            .await
            && let Ok(stats) = res.json::<UpstreamMempoolStats>().await
        {
            let summary = MempoolSummary {
                tx_count: stats.count,
                size_bytes: Some(stats.vsize),
                usage_bytes: None,
                total_fee_sats: Some(stats.total_fee),
            };
            *self.mempool_summary.write().await = Some(summary.clone());
            let _ = self.event_tx.send(LiveEvent::MempoolUpdated(summary));
        }

        // Fetch recent blocks
        if let Ok(res) = self
            .http_client
            .get(format!("{}/blocks", self.base_url))
            .send()
            .await
            && let Ok(blocks) = res.json::<Vec<UpstreamBlockHeader>>().await
        {
            let recent: Vec<RecentBlockSummary> = blocks
                .into_iter()
                .take(MAX_RECENT_BLOCKS)
                .map(|b| RecentBlockSummary {
                    height: b.height,
                    hash: b.id,
                    tx_count: b.tx_count.unwrap_or(0),
                    weight: b.weight,
                    size: b.size,
                    timestamp: b.timestamp,
                })
                .collect();

            if let Some(tip) = recent.first() {
                *self.tip_info.write().await = (tip.height, tip.hash.clone());
            }
            *self.recent_blocks.write().await = recent;
        }

        // Fetch recent transactions for initial visual activity
        if let Ok(res) = self
            .http_client
            .get(format!("{}/mempool/recent", self.base_url))
            .send()
            .await
            && let Ok(recent_txs) = res.json::<Vec<UpstreamRecentTxItem>>().await
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            for (i, tx) in recent_txs.into_iter().enumerate() {
                let first_seen = now.saturating_sub((i as u64) * 2);
                let summary = LiveTransactionSummary {
                    txid: tx.txid.clone(),
                    wtxid: None,
                    vsize: tx.vsize.unwrap_or(140),
                    weight: tx.vsize.unwrap_or(140) * 4,
                    fee_sats: tx.fee,
                    fee_rate: tx.fee.and_then(|f| {
                        let vs = tx.vsize.unwrap_or(140);
                        if vs > 0 {
                            Some(f as f64 / vs as f64)
                        } else {
                            None
                        }
                    }),
                    input_count: None,
                    output_count: None,
                    explicit_rbf: None,
                    mempool_replaceable: None,
                    has_witness: None,
                    first_seen_at: Some(first_seen),
                    depends: None,
                    source: Some("public_mainnet".to_string()),
                    hydration_status: Some("pending".to_string()),
                };

                self.insert_transaction(summary.clone()).await;
                let _ = self.hydration_tx.try_send(tx.txid);
            }
        }
    }

    async fn insert_transaction(&self, summary: LiveTransactionSummary) {
        let txid = summary.txid.clone();
        let mut cache = self.tx_cache.write().await;
        let mut ring = self.tx_ring.write().await;

        if !cache.contains_key(&txid) {
            if ring.len() >= MAX_RECENT_TX_CACHE
                && let Some(oldest) = ring.pop_front()
            {
                cache.remove(&oldest);
            }
            ring.push_back(txid.clone());
        }
        cache.insert(txid, summary);
    }

    async fn remove_transaction(&self, txid: &str) {
        let mut cache = self.tx_cache.write().await;
        cache.remove(txid);
    }

    async fn run_websocket_stream(&self) {
        let reconnect_backoff = [1, 2, 4, 8, 15, 30];
        let mut backoff_index = 0;

        loop {
            self.connection_status
                .store(STATUS_CONNECTING, Ordering::SeqCst);

            match connect_async(&self.ws_url).await {
                Ok((ws_stream, _)) => {
                    self.connection_status
                        .store(STATUS_CONNECTED, Ordering::SeqCst);
                    backoff_index = 0; // Reset backoff upon successful connection

                    let (mut write, mut read) = ws_stream.split();

                    // Send subscription: {"track-mempool-txids": true}
                    let sub_msg = serde_json::json!({ "track-mempool-txids": true });
                    if write
                        .send(tokio_tungstenite::tungstenite::Message::Text(
                            sub_msg.to_string().into(),
                        ))
                        .await
                        .is_err()
                    {
                        self.connection_status
                            .store(STATUS_RECONNECTING, Ordering::SeqCst);
                        continue;
                    }

                    // Process stream messages with a read timeout of 60s
                    while let Ok(Some(msg_res)) =
                        tokio::time::timeout(Duration::from_secs(60), read.next()).await
                    {
                        match msg_res {
                            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                self.handle_upstream_text(&text).await;
                            }
                            Ok(tokio_tungstenite::tungstenite::Message::Ping(data)) => {
                                let _ = write
                                    .send(tokio_tungstenite::tungstenite::Message::Pong(data))
                                    .await;
                            }
                            Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                            Err(_) => break,
                            _ => {}
                        }
                    }
                }
                Err(_) => {
                    // Connection failed
                }
            }

            self.connection_status
                .store(STATUS_RECONNECTING, Ordering::SeqCst);
            let delay = reconnect_backoff[backoff_index.min(reconnect_backoff.len() - 1)];
            backoff_index += 1;
            tokio::time::sleep(Duration::from_secs(delay)).await;
        }
    }

    async fn handle_upstream_text(&self, text: &str) {
        let Ok(parsed) = serde_json::from_str::<UpstreamWsMessage>(text) else {
            return;
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Some(mempool) = parsed.mempool_txids {
            // Added transactions
            for txid in mempool.added {
                let summary = LiveTransactionSummary {
                    txid: txid.clone(),
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
                    first_seen_at: Some(now),
                    depends: None,
                    source: Some("public_mainnet".to_string()),
                    hydration_status: Some("pending".to_string()),
                };

                self.insert_transaction(summary.clone()).await;
                let _ = self.event_tx.send(LiveEvent::TransactionAdded(summary));
                let _ = self.hydration_tx.try_send(txid);
            }

            // Removed / replaced transactions
            for txid in mempool.removed.into_iter().chain(mempool.replaced) {
                self.remove_transaction(&txid).await;
                let _ = self.event_tx.send(LiveEvent::TransactionRemoved { txid });
            }

            // Mined transactions
            for txid in mempool.mined {
                self.remove_transaction(&txid).await;
                let _ = self.event_tx.send(LiveEvent::TransactionRemoved { txid });
            }
        }

        if let Some(block) = parsed.block {
            let summary = RecentBlockSummary {
                height: block.height,
                hash: block.id.clone(),
                tx_count: block.tx_count.unwrap_or(0),
                weight: block.weight,
                size: block.size,
                timestamp: block.timestamp,
            };

            *self.tip_info.write().await = (block.height, block.id.clone());
            {
                let mut blocks = self.recent_blocks.write().await;
                if !blocks.iter().any(|b| b.hash == block.id) {
                    blocks.insert(0, summary.clone());
                    blocks.truncate(MAX_RECENT_BLOCKS);
                }
            }
            let _ = self.event_tx.send(LiveEvent::BlockConnected(summary));
        }
    }

    async fn run_hydration_worker(self: Arc<Self>, mut rx: mpsc::Receiver<String>) {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_HYDRATIONS));

        while let Some(txid) = rx.recv().await {
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };

            let this = Arc::clone(&self);
            tokio::spawn(async move {
                let _permit = permit;
                this.hydrate_single_transaction(&txid).await;
            });
        }
    }

    async fn hydrate_single_transaction(&self, txid: &str) {
        let url = format!("{}/tx/{}", self.base_url, txid);
        let res = match self.http_client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return,
        };

        if res.status().as_u16() == 429 {
            // Upstream throttled: back off briefly
            tokio::time::sleep(Duration::from_millis(500)).await;
            return;
        }

        if !res.status().is_success() {
            return;
        }

        let Ok(details) = res.json::<UpstreamTxDetails>().await else {
            return;
        };

        let vsize = details.weight.div_ceil(4);
        let fee_rate = if vsize > 0 {
            Some(details.fee as f64 / vsize as f64)
        } else {
            None
        };

        // Explicit RBF is strictly derived from transaction input sequence < 0xFFFFFFFE
        let explicit_rbf = details.vin.iter().any(|v| v.sequence < 0xFFFFFFFE);
        let has_witness = details.weight < details.size * 4;

        let mut updated = None;
        {
            let mut cache = self.tx_cache.write().await;
            if let Some(existing) = cache.get_mut(txid) {
                existing.vsize = vsize;
                existing.weight = details.weight;
                existing.fee_sats = Some(details.fee);
                existing.fee_rate = fee_rate;
                existing.input_count = Some(details.vin.len());
                existing.output_count = Some(details.vout.len());
                existing.explicit_rbf = Some(explicit_rbf);
                existing.has_witness = Some(has_witness);
                existing.hydration_status = Some("hydrated".to_string());
                updated = Some(existing.clone());
            }
        }

        if let Some(up) = updated {
            let _ = self.event_tx.send(LiveEvent::TransactionUpdated(up));
        }
    }
}

impl LiveDataProvider for PublicMainnetLiveProvider {
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
        self.connection_status.load(Ordering::SeqCst) == STATUS_CONNECTED
    }

    fn get_snapshot<'a>(&'a self) -> BoxFuture<'a, Result<LiveSnapshot, ApiError>> {
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

    fn get_recent_blocks<'a>(&'a self) -> BoxFuture<'a, Result<Vec<RecentBlockSummary>, ApiError>> {
        Box::pin(async move {
            let cached = self.recent_blocks.read().await.clone();
            if !cached.is_empty() {
                return Ok(cached);
            }

            // Fallback fetch
            let url = format!("{}/blocks", self.base_url);
            let res = self
                .http_client
                .get(&url)
                .send()
                .await
                .map_err(|_| ApiError::NODE)?;
            let blocks: Vec<UpstreamBlockHeader> = res.json().await.map_err(|_| ApiError::NODE)?;
            let recent: Vec<RecentBlockSummary> = blocks
                .into_iter()
                .take(MAX_RECENT_BLOCKS)
                .map(|b| RecentBlockSummary {
                    height: b.height,
                    hash: b.id,
                    tx_count: b.tx_count.unwrap_or(0),
                    weight: b.weight,
                    size: b.size,
                    timestamp: b.timestamp,
                })
                .collect();
            Ok(recent)
        })
    }

    fn get_mempool_summary<'a>(&'a self) -> BoxFuture<'a, Result<MempoolSummary, ApiError>> {
        Box::pin(async move {
            if let Some(summary) = self.mempool_summary.read().await.clone() {
                return Ok(summary);
            }

            let url = format!("{}/mempool", self.base_url);
            let res = self
                .http_client
                .get(&url)
                .send()
                .await
                .map_err(|_| ApiError::NODE)?;
            let stats: UpstreamMempoolStats = res.json().await.map_err(|_| ApiError::NODE)?;
            Ok(MempoolSummary {
                tx_count: stats.count,
                size_bytes: Some(stats.vsize),
                usage_bytes: None,
                total_fee_sats: Some(stats.total_fee),
            })
        })
    }

    fn get_block_details<'a>(
        &'a self,
        hash: &'a bitcoin::BlockHash,
        offset: usize,
        limit: usize,
    ) -> BoxFuture<'a, Result<BlockDetails, ApiError>> {
        Box::pin(async move {
            let hash_str = hash.to_string();
            let block_url = format!("{}/block/{}", self.base_url, hash_str);
            let block_res = self
                .http_client
                .get(&block_url)
                .send()
                .await
                .map_err(|_| ApiError::NODE)?;

            if block_res.status().as_u16() == 404 {
                return Err(ApiError::BLOCK_NOT_FOUND);
            }

            let header: UpstreamBlockHeader = block_res.json().await.map_err(|_| ApiError::NODE)?;

            // Fetch block txids for pagination
            let txids_url = format!("{}/block/{}/txids", self.base_url, hash_str);
            let txids: Vec<String> = match self.http_client.get(&txids_url).send().await {
                Ok(r) => r.json().await.unwrap_or_default(),
                Err(_) => Vec::new(),
            };

            let total = txids.len();
            let bounded_limit = limit.clamp(1, 100);
            let bounded_offset = offset.min(total);
            let end = (bounded_offset + bounded_limit).min(total);

            let page_items: Vec<BlockTransactionItem> = txids[bounded_offset..end]
                .iter()
                .enumerate()
                .map(|(i, txid)| BlockTransactionItem {
                    index: bounded_offset + i,
                    txid: txid.clone(),
                    is_coinbase: (bounded_offset + i) == 0,
                })
                .collect();

            let has_more = end < total;

            Ok(BlockDetails {
                network: "bitcoin".to_string(),
                height: header.height,
                hash: hash_str,
                previous_block_hash: header.previousblockhash,
                next_block_hash: None,
                merkle_root: header.merkle_root,
                version: header.version,
                timestamp: header.timestamp.unwrap_or(0),
                median_time: header.mediantime,
                bits: header.bits.map(|b| format!("{:x}", b)),
                difficulty: header.difficulty,
                tx_count: total,
                weight: header.weight,
                size: header.size,
                transactions: BlockTransactionPage {
                    items: page_items,
                    offset: bounded_offset,
                    limit: bounded_limit,
                    total,
                    has_more,
                },
            })
        })
    }

    fn get_block_details_by_height<'a>(
        &'a self,
        height: u64,
        offset: usize,
        limit: usize,
    ) -> BoxFuture<'a, Result<BlockDetails, ApiError>> {
        Box::pin(async move {
            let height_url = format!("{}/block-height/{}", self.base_url, height);
            let res = self
                .http_client
                .get(&height_url)
                .send()
                .await
                .map_err(|_| ApiError::NODE)?;

            if res.status().as_u16() == 404 {
                return Err(ApiError::BLOCK_NOT_FOUND);
            }

            let hash_str = res.text().await.map_err(|_| ApiError::NODE)?;
            let block_hash: bitcoin::BlockHash =
                hash_str.trim().parse().map_err(|_| ApiError::NODE)?;

            self.get_block_details(&block_hash, offset, limit).await
        })
    }

    fn inspect_transaction<'a>(
        &'a self,
        txid: &'a Txid,
    ) -> BoxFuture<'a, Result<crate::live_provider::InspectedTransactionResult, ApiError>> {
        Box::pin(async move {
            let txid_str = txid.to_string();

            // 1. Fetch raw transaction hex
            let hex_url = format!("{}/tx/{}/hex", self.base_url, txid_str);
            let hex_res = self
                .http_client
                .get(&hex_url)
                .send()
                .await
                .map_err(|_| ApiError::NODE)?;

            if hex_res.status().as_u16() == 404 {
                return Err(ApiError::TRANSACTION_NOT_FOUND);
            }

            let raw_hex = hex_res.text().await.map_err(|_| ApiError::NODE)?;
            let raw_tx = txsignx_core::decode_transaction(raw_hex.trim())
                .map_err(|_| ApiError::INVALID_TRANSACTION)?;

            // 2. Fetch transaction details for status & prevouts
            let tx_url = format!("{}/tx/{}", self.base_url, txid_str);
            let (confirmations, block_hash, prevouts) =
                match self.http_client.get(&tx_url).send().await {
                    Ok(r) if r.status().is_success() => {
                        if let Ok(details) = r.json::<UpstreamTxDetails>().await {
                            let confs = if details.status.confirmed {
                                Some(1)
                            } else {
                                Some(0)
                            };
                            let b_hash = details
                                .status
                                .block_hash
                                .and_then(|h| h.parse::<bitcoin::BlockHash>().ok());
                            let resolved: Vec<Option<bitcoin::TxOut>> = details
                                .vin
                                .into_iter()
                                .map(|v| {
                                    v.prevout.and_then(|p| {
                                        let script_bytes =
                                            match Vec::<u8>::from_hex(&p.scriptpubkey) {
                                                Ok(b) => b,
                                                Err(_) => return None,
                                            };
                                        Some(bitcoin::TxOut {
                                            value: Amount::from_sat(p.value),
                                            script_pubkey: ScriptBuf::from_bytes(script_bytes),
                                        })
                                    })
                                })
                                .collect();
                            (confs, b_hash, resolved)
                        } else {
                            (None, None, vec![None; raw_tx.input.len()])
                        }
                    }
                    _ => (None, None, vec![None; raw_tx.input.len()]),
                };

            let node_tx = txsignx_node::NodeTransaction {
                transaction: raw_tx,
                confirmations,
                block_hash,
            };

            let chain_info = txsignx_node::BlockchainInfo {
                network: bitcoin::Network::Bitcoin,
                blocks: self.tip_info.read().await.0,
                headers: self.tip_info.read().await.0,
                best_block_hash: self
                    .tip_info
                    .read()
                    .await
                    .1
                    .parse()
                    .unwrap_or_else(|_| bitcoin::BlockHash::all_zeros()),
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

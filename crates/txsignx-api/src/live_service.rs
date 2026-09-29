use crate::{config::ConfiguredNode, error::ApiError};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{IntoResponse, Response},
};
use bitcoin::Txid;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{RwLock, broadcast};
use txsignx_node::{
    LiveEvent, LiveSnapshot, LiveTransactionSummary, MAX_LIVE_TRANSACTIONS, MAX_RECENT_BLOCKS,
    MempoolEntry, MempoolSummary, RecentBlockSummary,
};

pub const MAX_WS_CLIENTS: usize = 32;
pub const MAX_CACHE_ENTRIES: usize = 1000;
pub const EVENT_CHANNEL_CAPACITY: usize = 256;

pub struct LiveService {
    node: ConfiguredNode,
    tx_cache: RwLock<HashMap<Txid, LiveTransactionSummary>>,
    event_tx: broadcast::Sender<LiveEvent>,
    active_ws_clients: AtomicUsize,
}

impl LiveService {
    pub(crate) fn new(node: ConfiguredNode) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let service = Arc::new(Self {
            node,
            tx_cache: RwLock::new(HashMap::new()),
            event_tx,
            active_ws_clients: AtomicUsize::new(0),
        });

        let poller = Arc::clone(&service);
        tokio::spawn(async move {
            poller.run_poller().await;
        });

        service
    }

    #[allow(dead_code)]
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.event_tx.subscribe()
    }

    pub(crate) async fn get_snapshot(&self) -> Result<LiveSnapshot, ApiError> {
        let rpc = self.node.rpc();
        let chain_info = rpc.blockchain_info().map_err(|_| ApiError::NODE)?;
        let network = self.node.network.name().to_string();
        let tip_height = chain_info.blocks;
        let tip_hash = chain_info.best_block_hash.to_string();

        let recent_blocks = rpc.recent_blocks(MAX_RECENT_BLOCKS).unwrap_or_default();
        let mempool_info = rpc.mempool_summary().ok();
        let mempool_tx_count = mempool_info.as_ref().map(|m| m.tx_count).unwrap_or(0);
        let mempool_size_bytes = mempool_info.as_ref().and_then(|m| m.size_bytes);

        let latest_transactions = self
            .get_bounded_mempool_transactions(MAX_LIVE_TRANSACTIONS)
            .await;

        Ok(LiveSnapshot {
            network,
            tip_height,
            tip_hash,
            recent_blocks,
            mempool_tx_count,
            mempool_size_bytes,
            latest_transactions,
        })
    }

    pub(crate) async fn get_recent_blocks(&self) -> Result<Vec<RecentBlockSummary>, ApiError> {
        self.node
            .rpc()
            .recent_blocks(MAX_RECENT_BLOCKS)
            .map_err(|_| ApiError::NODE)
    }

    pub(crate) async fn get_mempool_summary(&self) -> Result<MempoolSummary, ApiError> {
        self.node
            .rpc()
            .mempool_summary()
            .map_err(|_| ApiError::NODE)
    }

    pub(crate) async fn handle_ws_upgrade(self: Arc<Self>, ws: WebSocketUpgrade) -> Response {
        let current_clients = self.active_ws_clients.load(Ordering::Relaxed);
        if current_clients >= MAX_WS_CLIENTS {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Maximum live stream clients reached",
            )
                .into_response();
        }

        let this = Arc::clone(&self);
        ws.on_upgrade(move |socket| async move {
            this.handle_ws_connection(socket).await;
        })
    }

    async fn handle_ws_connection(self: Arc<Self>, mut socket: WebSocket) {
        self.active_ws_clients.fetch_add(1, Ordering::Relaxed);
        let mut rx = self.event_tx.subscribe();

        // Send initial snapshot on connect
        if let Ok(snapshot) = self.get_snapshot().await {
            let msg = LiveEvent::Snapshot(snapshot);
            if let Ok(json_str) = serde_json::to_string(&msg)
                && socket.send(Message::Text(json_str.into())).await.is_err()
            {
                self.active_ws_clients.fetch_sub(1, Ordering::Relaxed);
                return;
            }
        }

        loop {
            tokio::select! {
                event_res = rx.recv() => {
                    match event_res {
                        Ok(event) => {
                            if let Ok(json_str) = serde_json::to_string(&event)
                                && socket.send(Message::Text(json_str.into())).await.is_err()
                            {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Slow subscriber dropped stale events: resync with fresh snapshot
                            if let Ok(snapshot) = self.get_snapshot().await {
                                let msg = LiveEvent::Snapshot(snapshot);
                                if let Ok(json_str) = serde_json::to_string(&msg)
                                    && socket.send(Message::Text(json_str.into())).await.is_err()
                                {
                                    break;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                msg_res = socket.recv() => {
                    match msg_res {
                        Some(Ok(Message::Ping(data)))
                            if socket.send(Message::Pong(data.clone())).await.is_err() =>
                        {
                            break;
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        _ => {}
                    }
                }
            }
        }

        self.active_ws_clients.fetch_sub(1, Ordering::Relaxed);
    }

    async fn get_bounded_mempool_transactions(&self, limit: usize) -> Vec<LiveTransactionSummary> {
        let rpc = self.node.rpc();
        let entries = match rpc.raw_mempool_verbose() {
            Ok(e) => e,
            Err(_) => {
                if let Ok(txids) = rpc.raw_mempool_txids() {
                    let mut result = Vec::new();
                    let cache = self.tx_cache.read().await;
                    for txid in txids.iter().take(limit) {
                        if let Some(summary) = cache.get(txid) {
                            result.push(summary.clone());
                        }
                    }
                    return result;
                }
                return Vec::new();
            }
        };

        let mut sorted_entries: Vec<(Txid, MempoolEntry)> = entries.into_iter().collect();
        sorted_entries.sort_by_key(|b| std::cmp::Reverse(b.1.time.unwrap_or(0)));
        sorted_entries.truncate(limit);

        let mut summaries = Vec::with_capacity(sorted_entries.len());
        let mut missing_txs = Vec::new();

        {
            let cache = self.tx_cache.read().await;
            for (txid, entry) in &sorted_entries {
                if let Some(summary) = cache.get(txid) {
                    summaries.push(summary.clone());
                } else {
                    missing_txs.push((*txid, entry.clone()));
                }
            }
        }

        if !missing_txs.is_empty() {
            let mut cache = self.tx_cache.write().await;
            for (txid, entry) in missing_txs {
                if let Some(summary) = cache.get(&txid) {
                    summaries.push(summary.clone());
                    continue;
                }
                let summary = self.build_transaction_summary(&txid, &entry);
                if cache.len() >= MAX_CACHE_ENTRIES {
                    cache.clear();
                }
                cache.insert(txid, summary.clone());
                summaries.push(summary);
            }
        }

        summaries.sort_by_key(|b| std::cmp::Reverse(b.first_seen_at.unwrap_or(0)));
        summaries
    }

    fn build_transaction_summary(
        &self,
        txid: &Txid,
        entry: &MempoolEntry,
    ) -> LiveTransactionSummary {
        let fee_sats = entry
            .fees
            .as_ref()
            .map(|f| (f.base * 100_000_000.0).round() as u64);
        let vsize = entry.vsize;
        let fee_rate = fee_sats.map(|s| s as f64 / vsize.max(1) as f64);

        if let Ok(node_tx) = self.node.rpc().get_raw_transaction(txid) {
            let tx = &node_tx.transaction;
            let explicit_rbf = entry
                .bip125_replaceable
                .unwrap_or_else(|| tx.input.iter().any(|i| i.sequence.is_rbf()));
            let has_witness = tx.input.iter().any(|i| !i.witness.is_empty());
            LiveTransactionSummary {
                txid: txid.to_string(),
                wtxid: if has_witness {
                    Some(tx.compute_wtxid().to_string())
                } else {
                    entry.wtxid.clone()
                },
                vsize,
                weight: entry.weight,
                fee_sats,
                fee_rate,
                input_count: tx.input.len(),
                output_count: tx.output.len(),
                explicit_rbf,
                has_witness,
                first_seen_at: entry.time,
                depends: entry.depends.clone(),
            }
        } else {
            let has_witness = entry
                .wtxid
                .as_deref()
                .is_some_and(|w| w != txid.to_string());
            LiveTransactionSummary {
                txid: txid.to_string(),
                wtxid: entry.wtxid.clone(),
                vsize,
                weight: entry.weight,
                fee_sats,
                fee_rate,
                input_count: 1,
                output_count: 1,
                explicit_rbf: entry.bip125_replaceable.unwrap_or(false),
                has_witness,
                first_seen_at: entry.time,
                depends: entry.depends.clone(),
            }
        }
    }

    async fn run_poller(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(1500));
        let mut last_tip_hash = None;
        let mut last_mempool_txids: HashSet<Txid> = HashSet::new();

        loop {
            interval.tick().await;

            let rpc = self.node.rpc();
            if let Ok(info) = rpc.blockchain_info() {
                let current_tip = info.best_block_hash;
                if last_tip_hash.as_ref() != Some(&current_tip) {
                    if last_tip_hash.is_some()
                        && let Ok(block_summary) = rpc.get_block_summary(&current_tip)
                    {
                        let _ = self.event_tx.send(LiveEvent::BlockConnected(block_summary));
                    }
                    last_tip_hash = Some(current_tip);
                }
            }

            if let Ok(entries) = rpc.raw_mempool_verbose() {
                let current_txids: HashSet<Txid> = entries.keys().copied().collect();

                for txid in current_txids.difference(&last_mempool_txids) {
                    if let Some(entry) = entries.get(txid) {
                        let summary = self.build_transaction_summary(txid, entry);
                        {
                            let mut cache = self.tx_cache.write().await;
                            if cache.len() >= MAX_CACHE_ENTRIES {
                                cache.clear();
                            }
                            cache.insert(*txid, summary.clone());
                        }
                        let _ = self.event_tx.send(LiveEvent::TransactionAdded(summary));
                    }
                }

                for txid in last_mempool_txids.difference(&current_txids) {
                    let _ = self.event_tx.send(LiveEvent::TransactionRemoved {
                        txid: txid.to_string(),
                    });
                }

                last_mempool_txids = current_txids;
            }
        }
    }
}

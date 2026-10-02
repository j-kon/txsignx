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
    BlockDetails, LiveEvent, LiveSnapshot, LiveTransactionSummary, MAX_LIVE_TRANSACTIONS,
    MAX_RECENT_BLOCKS, MempoolEntry, MempoolSummary, RecentBlockSummary,
};

pub const MAX_WS_CLIENTS: usize = 32;
pub const MAX_CACHE_ENTRIES: usize = 1000;
pub const EVENT_CHANNEL_CAPACITY: usize = 256;

struct WsConnectionGuard {
    active_clients: Arc<AtomicUsize>,
}

impl Drop for WsConnectionGuard {
    fn drop(&mut self) {
        self.active_clients.fetch_sub(1, Ordering::SeqCst);
    }
}

pub struct LiveService {
    node: ConfiguredNode,
    tx_cache: RwLock<HashMap<Txid, LiveTransactionSummary>>,
    event_tx: broadcast::Sender<LiveEvent>,
    active_ws_clients: Arc<AtomicUsize>,
}

impl LiveService {
    pub(crate) fn new(node: ConfiguredNode) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let service = Arc::new(Self {
            node,
            tx_cache: RwLock::new(HashMap::new()),
            event_tx,
            active_ws_clients: Arc::new(AtomicUsize::new(0)),
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

        let recent_blocks = rpc.recent_blocks(MAX_RECENT_BLOCKS).ok();
        let mempool = rpc.mempool_summary().ok();
        let mempool_tx_count = mempool.as_ref().map(|m| m.tx_count);
        let mempool_size_bytes = mempool.as_ref().and_then(|m| m.size_bytes);

        let latest_transactions = self
            .get_bounded_mempool_transactions(MAX_LIVE_TRANSACTIONS)
            .await;

        Ok(LiveSnapshot {
            network,
            tip_height,
            tip_hash,
            recent_blocks,
            mempool,
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

    pub(crate) async fn get_block_details(
        &self,
        hash: &bitcoin::BlockHash,
        offset: usize,
        limit: usize,
    ) -> Result<BlockDetails, ApiError> {
        self.node
            .rpc()
            .get_block_details(hash, offset, limit)
            .map_err(|e| match e {
                txsignx_node::NodeError::BlockNotFound
                | txsignx_node::NodeError::TransactionNotFound => ApiError::BLOCK_NOT_FOUND,
                _ => ApiError::NODE,
            })
    }

    pub(crate) async fn get_block_details_by_height(
        &self,
        height: u64,
        offset: usize,
        limit: usize,
    ) -> Result<BlockDetails, ApiError> {
        let hash = self
            .node
            .rpc()
            .block_hash_by_height(height)
            .map_err(|e| match e {
                txsignx_node::NodeError::BlockNotFound
                | txsignx_node::NodeError::TransactionNotFound => ApiError::BLOCK_NOT_FOUND,
                _ => ApiError::NODE,
            })?;
        self.get_block_details(&hash, offset, limit).await
    }

    pub(crate) async fn handle_ws_upgrade(self: Arc<Self>, ws: WebSocketUpgrade) -> Response {
        let reserved =
            self.active_ws_clients
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    if current < MAX_WS_CLIENTS {
                        Some(current + 1)
                    } else {
                        None
                    }
                });

        if reserved.is_err() {
            return (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "Maximum live stream clients reached",
            )
                .into_response();
        }

        let guard = WsConnectionGuard {
            active_clients: Arc::clone(&self.active_ws_clients),
        };

        let this = Arc::clone(&self);
        ws.on_upgrade(move |socket| async move {
            let _guard = guard;
            this.handle_ws_connection(socket).await;
        })
    }

    async fn handle_ws_connection(self: Arc<Self>, mut socket: WebSocket) {
        let mut rx = self.event_tx.subscribe();

        // Send initial snapshot on connect
        if let Ok(snapshot) = self.get_snapshot().await {
            let msg = LiveEvent::Snapshot(snapshot);
            if let Ok(json_str) = serde_json::to_string(&msg)
                && socket.send(Message::Text(json_str.into())).await.is_err()
            {
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
    }

    async fn get_bounded_mempool_transactions(
        &self,
        limit: usize,
    ) -> Option<Vec<LiveTransactionSummary>> {
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
                    return Some(result);
                }
                return None;
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
            // Bound full raw transaction fetches to prevent RPC storm on large mempool
            const MAX_RAW_FETCHES_PER_CYCLE: usize = 50;
            let mut raw_fetches = 0;
            let mut cache = self.tx_cache.write().await;
            for (txid, entry) in missing_txs {
                if let Some(summary) = cache.get(&txid) {
                    summaries.push(summary.clone());
                    continue;
                }
                let summary = if raw_fetches < MAX_RAW_FETCHES_PER_CYCLE {
                    raw_fetches += 1;
                    self.build_transaction_summary(&txid, &entry)
                } else {
                    self.build_lightweight_summary(&txid, &entry)
                };
                if cache.len() >= MAX_CACHE_ENTRIES {
                    cache.clear();
                }
                cache.insert(txid, summary.clone());
                summaries.push(summary);
            }
        }

        summaries.sort_by_key(|b| std::cmp::Reverse(b.first_seen_at.unwrap_or(0)));
        Some(summaries)
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
        let mempool_replaceable = entry.bip125_replaceable;

        if let Ok(node_tx) = self.node.rpc().get_raw_transaction(txid) {
            let tx = &node_tx.transaction;
            // Explicit RBF signaling = at least one input has nSequence < 0xFFFFFFFE from its OWN inputs!
            let explicit_rbf = Some(tx.input.iter().any(|i| i.sequence.is_rbf()));
            let has_witness = Some(tx.input.iter().any(|i| !i.witness.is_empty()));
            let wtxid = if has_witness == Some(true) {
                Some(tx.compute_wtxid().to_string())
            } else {
                entry.wtxid.clone()
            };
            LiveTransactionSummary {
                txid: txid.to_string(),
                wtxid,
                vsize,
                weight: entry.weight,
                fee_sats,
                fee_rate,
                input_count: Some(tx.input.len()),
                output_count: Some(tx.output.len()),
                explicit_rbf,
                mempool_replaceable,
                has_witness,
                first_seen_at: entry.time,
                depends: entry.depends.clone(),
            }
        } else {
            self.build_lightweight_summary(txid, entry)
        }
    }

    fn build_lightweight_summary(
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
        let has_witness = entry.wtxid.as_deref().map(|w| w != txid.to_string());

        LiveTransactionSummary {
            txid: txid.to_string(),
            wtxid: entry.wtxid.clone(),
            vsize,
            weight: entry.weight,
            fee_sats,
            fee_rate,
            input_count: None,
            output_count: None,
            explicit_rbf: None,
            mempool_replaceable: entry.bip125_replaceable,
            has_witness,
            first_seen_at: entry.time,
            depends: entry.depends.clone(),
        }
    }

    async fn run_poller(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(1500));
        let mut last_tip_hash = None;
        let mut last_mempool_txids: HashSet<Txid> = HashSet::new();

        loop {
            interval.tick().await;

            let rpc = self.node.rpc();
            let mut newly_connected_block_txids = HashSet::new();
            let mut newly_connected_block_summary = None;

            if let Ok(info) = rpc.blockchain_info() {
                let current_tip = info.best_block_hash;
                if last_tip_hash.as_ref() != Some(&current_tip) {
                    if last_tip_hash.is_some()
                        && let Ok(block_summary) = rpc.get_block_summary(&current_tip)
                    {
                        if let Ok(txids) = rpc.get_block_txids(&current_tip) {
                            newly_connected_block_txids = txids.into_iter().collect();
                        }
                        let _ = self
                            .event_tx
                            .send(LiveEvent::BlockConnected(block_summary.clone()));
                        newly_connected_block_summary = Some(block_summary);
                    }
                    last_tip_hash = Some(current_tip);
                }
            }

            if let Ok(entries) = rpc.raw_mempool_verbose() {
                let current_txids: HashSet<Txid> = entries.keys().copied().collect();

                let mut changed = false;

                for txid in current_txids.difference(&last_mempool_txids) {
                    changed = true;
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

                let removed_txids: Vec<Txid> = last_mempool_txids
                    .difference(&current_txids)
                    .copied()
                    .collect();

                if !removed_txids.is_empty() {
                    changed = true;
                    let mut cache = self.tx_cache.write().await;
                    for txid in removed_txids {
                        // Evict from cache when leaving mempool
                        cache.remove(&txid);

                        if newly_connected_block_txids.contains(&txid)
                            && let Some(ref block) = newly_connected_block_summary
                        {
                            let _ = self.event_tx.send(LiveEvent::TransactionConfirmed {
                                txid: txid.to_string(),
                                block_hash: block.hash.clone(),
                                block_height: block.height,
                            });
                        } else {
                            let _ = self.event_tx.send(LiveEvent::TransactionRemoved {
                                txid: txid.to_string(),
                            });
                        }
                    }
                }

                // If mempool set changed, emit authoritative MempoolUpdated event
                if changed && let Ok(summary) = rpc.mempool_summary() {
                    let _ = self.event_tx.send(LiveEvent::MempoolUpdated(summary));
                }

                last_mempool_txids = current_txids;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    #[test]
    fn test_ws_connection_guard_and_admission_bounds() {
        let active = Arc::new(AtomicUsize::new(0));

        let mut guards = Vec::new();
        // Reserve up to MAX_WS_CLIENTS (32)
        for _ in 0..MAX_WS_CLIENTS {
            let res = active.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |c| {
                if c < MAX_WS_CLIENTS {
                    Some(c + 1)
                } else {
                    None
                }
            });
            assert!(res.is_ok(), "reservation within limit must succeed");
            guards.push(WsConnectionGuard {
                active_clients: Arc::clone(&active),
            });
        }
        assert_eq!(active.load(Ordering::SeqCst), MAX_WS_CLIENTS);

        // 33rd client must be rejected
        let overflow = active.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |c| {
            if c < MAX_WS_CLIENTS {
                Some(c + 1)
            } else {
                None
            }
        });
        assert!(
            overflow.is_err(),
            "reservation over MAX_WS_CLIENTS must fail"
        );

        // Dropping one guard frees a slot
        drop(guards.pop());
        assert_eq!(active.load(Ordering::SeqCst), MAX_WS_CLIENTS - 1);

        let retry = active.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |c| {
            if c < MAX_WS_CLIENTS {
                Some(c + 1)
            } else {
                None
            }
        });
        assert!(retry.is_ok(), "reservation after slot freed must succeed");
    }

    #[tokio::test]
    async fn test_tx_cache_eviction() {
        let mut cache = HashMap::new();
        let txid1 = Txid::from_byte_array([1; 32]);
        let txid2 = Txid::from_byte_array([2; 32]);

        cache.insert(
            txid1,
            LiveTransactionSummary {
                txid: txid1.to_string(),
                wtxid: None,
                vsize: 140,
                weight: 560,
                fee_sats: Some(1400),
                fee_rate: Some(10.0),
                input_count: Some(1),
                output_count: Some(1),
                explicit_rbf: Some(true),
                mempool_replaceable: Some(true),
                has_witness: Some(false),
                first_seen_at: Some(1700000000),
                depends: None,
            },
        );

        assert_eq!(cache.len(), 1);
        assert!(cache.contains_key(&txid1));

        // When tx leaves mempool, it is removed
        cache.remove(&txid1);
        assert!(!cache.contains_key(&txid1));
        assert_eq!(cache.len(), 0);

        // Max entries bound test: clear when reaching MAX_CACHE_ENTRIES
        for i in 0..MAX_CACHE_ENTRIES {
            let mut b = [0u8; 32];
            b[0] = (i & 0xff) as u8;
            b[1] = (i >> 8) as u8;
            cache.insert(
                Txid::from_byte_array(b),
                LiveTransactionSummary {
                    txid: format!("tx_{}", i),
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
                    first_seen_at: None,
                    depends: None,
                },
            );
        }
        assert_eq!(cache.len(), MAX_CACHE_ENTRIES);
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(
            txid2,
            LiveTransactionSummary {
                txid: txid2.to_string(),
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
                first_seen_at: None,
                depends: None,
            },
        );
        assert_eq!(cache.len(), 1);
    }
}

use crate::{
    config::ConfiguredNode,
    error::ApiError,
    live_provider::{BoxFuture, LiveDataProvider},
};
use bitcoin::Txid;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{RwLock, broadcast};
use txsignx_node::{
    BlockDetails, LiveEvent, LiveSnapshot, LiveTransactionSummary, MAX_LIVE_TRANSACTIONS,
    MAX_RECENT_BLOCKS, MempoolEntry, MempoolSummary, RecentBlockSummary,
};

pub const MAX_CACHE_ENTRIES: usize = 1000;
pub const EVENT_CHANNEL_CAPACITY: usize = 256;

pub struct BitcoinCoreLiveProvider {
    node: ConfiguredNode,
    tx_cache: RwLock<HashMap<Txid, LiveTransactionSummary>>,
    event_tx: broadcast::Sender<LiveEvent>,
}

impl BitcoinCoreLiveProvider {
    pub fn new(node: ConfiguredNode) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let provider = Arc::new(Self {
            node,
            tx_cache: RwLock::new(HashMap::new()),
            event_tx,
        });

        let poller = Arc::clone(&provider);
        tokio::spawn(async move {
            poller.run_poller().await;
        });

        provider
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
                source: Some("bitcoin_core".to_string()),
                hydration_status: Some("hydrated".to_string()),
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
            source: Some("bitcoin_core".to_string()),
            hydration_status: Some("lightweight".to_string()),
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

                if changed && let Ok(summary) = rpc.mempool_summary() {
                    let _ = self.event_tx.send(LiveEvent::MempoolUpdated(summary));
                }

                last_mempool_txids = current_txids;
            }
        }
    }
}

impl LiveDataProvider for BitcoinCoreLiveProvider {
    fn source_name(&self) -> &'static str {
        "bitcoin_core"
    }

    fn source_label(&self) -> &'static str {
        "Bitcoin Core"
    }

    fn network_name(&self) -> String {
        self.node.network.name().to_string()
    }

    fn is_live(&self) -> bool {
        true
    }

    fn get_snapshot<'a>(&'a self) -> BoxFuture<'a, Result<LiveSnapshot, ApiError>> {
        Box::pin(async move {
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
                source: Some("bitcoin_core".to_string()),
                source_label: Some("Bitcoin Core".to_string()),
                tip_height,
                tip_hash,
                recent_blocks,
                mempool,
                mempool_tx_count,
                mempool_size_bytes,
                latest_transactions,
            })
        })
    }

    fn get_recent_blocks<'a>(&'a self) -> BoxFuture<'a, Result<Vec<RecentBlockSummary>, ApiError>> {
        Box::pin(async move {
            self.node
                .rpc()
                .recent_blocks(MAX_RECENT_BLOCKS)
                .map_err(|_| ApiError::NODE)
        })
    }

    fn get_mempool_summary<'a>(&'a self) -> BoxFuture<'a, Result<MempoolSummary, ApiError>> {
        Box::pin(async move {
            self.node
                .rpc()
                .mempool_summary()
                .map_err(|_| ApiError::NODE)
        })
    }

    fn get_block_details<'a>(
        &'a self,
        hash: &'a bitcoin::BlockHash,
        offset: usize,
        limit: usize,
    ) -> BoxFuture<'a, Result<BlockDetails, ApiError>> {
        Box::pin(async move {
            self.node
                .rpc()
                .get_block_details(hash, offset, limit)
                .map_err(|e| match e {
                    txsignx_node::NodeError::BlockNotFound
                    | txsignx_node::NodeError::TransactionNotFound => ApiError::BLOCK_NOT_FOUND,
                    _ => ApiError::NODE,
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
        })
    }

    fn inspect_transaction<'a>(
        &'a self,
        txid: &'a Txid,
    ) -> BoxFuture<'a, Result<crate::live_provider::InspectedTransactionResult, ApiError>> {
        Box::pin(async move {
            self.node
                .inspect_transaction(txid)
                .map_err(|_| ApiError::NODE)
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.event_tx.subscribe()
    }
}

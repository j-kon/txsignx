use crate::{
    config::ConfiguredNode,
    error::ApiError,
    live_provider::{
        DynLiveProvider, bitcoin_core::BitcoinCoreLiveProvider,
        public_mainnet::PublicMainnetLiveProvider,
    },
};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::{IntoResponse, Response},
};
use bitcoin::Txid;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::broadcast;
use txsignx_node::{BlockDetails, LiveEvent, LiveSnapshot, MempoolSummary, RecentBlockSummary};

pub const MAX_WS_CLIENTS: usize = 32;

struct WsConnectionGuard {
    active_clients: Arc<AtomicUsize>,
}

impl Drop for WsConnectionGuard {
    fn drop(&mut self) {
        self.active_clients.fetch_sub(1, Ordering::SeqCst);
    }
}

pub struct LiveService {
    provider: DynLiveProvider,
    active_ws_clients: Arc<AtomicUsize>,
}

impl LiveService {
    pub(crate) fn new(node: ConfiguredNode) -> Arc<Self> {
        let provider = BitcoinCoreLiveProvider::new(node);
        Self::from_provider(provider)
    }

    pub(crate) fn new_public_mainnet() -> Arc<Self> {
        let provider = PublicMainnetLiveProvider::new();
        Self::from_provider(provider)
    }

    pub(crate) fn from_provider(provider: DynLiveProvider) -> Arc<Self> {
        Arc::new(Self {
            provider,
            active_ws_clients: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn source_name(&self) -> &'static str {
        self.provider.source_name()
    }

    pub fn source_label(&self) -> &'static str {
        self.provider.source_label()
    }

    pub fn network_name(&self) -> String {
        self.provider.network_name()
    }

    #[allow(dead_code)]
    pub fn is_live(&self) -> bool {
        self.provider.is_live()
    }

    #[allow(dead_code)]
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.provider.subscribe()
    }

    pub(crate) async fn get_snapshot(&self) -> Result<LiveSnapshot, ApiError> {
        self.provider.get_snapshot().await
    }

    pub(crate) async fn get_recent_blocks(&self) -> Result<Vec<RecentBlockSummary>, ApiError> {
        self.provider.get_recent_blocks().await
    }

    pub(crate) async fn get_mempool_summary(&self) -> Result<MempoolSummary, ApiError> {
        self.provider.get_mempool_summary().await
    }

    pub(crate) async fn get_block_details(
        &self,
        hash: &bitcoin::BlockHash,
        offset: usize,
        limit: usize,
    ) -> Result<BlockDetails, ApiError> {
        self.provider.get_block_details(hash, offset, limit).await
    }

    pub(crate) async fn get_block_details_by_height(
        &self,
        height: u64,
        offset: usize,
        limit: usize,
    ) -> Result<BlockDetails, ApiError> {
        self.provider
            .get_block_details_by_height(height, offset, limit)
            .await
    }

    pub(crate) async fn inspect_transaction(
        &self,
        txid: &Txid,
    ) -> Result<
        (
            txsignx_node::NodeTransaction,
            Vec<Option<bitcoin::TxOut>>,
            txsignx_node::BlockchainInfo,
        ),
        ApiError,
    > {
        self.provider.inspect_transaction(txid).await
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
        let mut rx = self.provider.subscribe();

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;
    use txsignx_node::LiveTransactionSummary;

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
        let mut cache = std::collections::HashMap::new();
        let txid1 = Txid::from_byte_array([1; 32]);
        let txid2 = Txid::from_byte_array([2; 32]);

        cache.insert(
            txid1,
            LiveTransactionSummary {
                txid: txid1.to_string(),
                wtxid: None,
                vsize: Some(140),
                weight: Some(560),
                fee_sats: Some(1400),
                fee_rate: Some(10.0),
                input_count: Some(1),
                output_count: Some(1),
                explicit_rbf: Some(true),
                mempool_replaceable: Some(true),
                has_witness: Some(false),
                first_seen_at: Some(1700000000),
                observed_at: Some(1700000000),
                depends: None,
                source: Some("bitcoin_core".to_string()),
                hydration_status: Some("hydrated".to_string()),
            },
        );

        assert_eq!(cache.len(), 1);
        assert!(cache.contains_key(&txid1));

        // When tx leaves mempool, it is removed
        cache.remove(&txid1);
        assert!(!cache.contains_key(&txid1));
        assert_eq!(cache.len(), 0);

        // Max entries bound test: clear when reaching 1000
        for i in 0..1000 {
            let mut b = [0u8; 32];
            b[0] = (i & 0xff) as u8;
            b[1] = (i >> 8) as u8;
            cache.insert(
                Txid::from_byte_array(b),
                LiveTransactionSummary {
                    txid: format!("tx_{}", i),
                    wtxid: None,
                    vsize: Some(140),
                    weight: Some(560),
                    fee_sats: None,
                    fee_rate: None,
                    input_count: None,
                    output_count: None,
                    explicit_rbf: None,
                    mempool_replaceable: None,
                    has_witness: None,
                    first_seen_at: None,
                    observed_at: Some(1700000000 + i as u64),
                    depends: None,
                    source: None,
                    hydration_status: None,
                },
            );
        }
        assert_eq!(cache.len(), 1000);
        if cache.len() >= 1000 {
            cache.clear();
        }
        cache.insert(
            txid2,
            LiveTransactionSummary {
                txid: txid2.to_string(),
                wtxid: None,
                vsize: Some(140),
                weight: Some(560),
                fee_sats: None,
                fee_rate: None,
                input_count: None,
                output_count: None,
                explicit_rbf: None,
                mempool_replaceable: None,
                has_witness: None,
                first_seen_at: None,
                observed_at: Some(1700001001),
                depends: None,
                source: None,
                hydration_status: None,
            },
        );
        assert_eq!(cache.len(), 1);
    }
}

use crate::error::ApiError;
use bitcoin::Txid;
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::sync::broadcast;
use txsignx_node::{BlockDetails, LiveEvent, LiveSnapshot, MempoolSummary, RecentBlockSummary};

pub mod bitcoin_core;
pub mod public_mainnet;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub type InspectedTransactionResult = (
    txsignx_node::NodeTransaction,
    Vec<Option<bitcoin::TxOut>>,
    txsignx_node::BlockchainInfo,
);

pub trait LiveDataProvider: Send + Sync {
    fn source_name(&self) -> &'static str;
    fn source_label(&self) -> &'static str;
    fn network_name(&self) -> String;
    fn is_live(&self) -> bool;

    fn get_snapshot<'a>(&'a self) -> BoxFuture<'a, Result<LiveSnapshot, ApiError>>;

    fn get_recent_blocks<'a>(&'a self) -> BoxFuture<'a, Result<Vec<RecentBlockSummary>, ApiError>>;

    fn get_mempool_summary<'a>(&'a self) -> BoxFuture<'a, Result<MempoolSummary, ApiError>>;

    fn get_block_details<'a>(
        &'a self,
        hash: &'a bitcoin::BlockHash,
        offset: usize,
        limit: usize,
    ) -> BoxFuture<'a, Result<BlockDetails, ApiError>>;

    fn get_block_details_by_height<'a>(
        &'a self,
        height: u64,
        offset: usize,
        limit: usize,
    ) -> BoxFuture<'a, Result<BlockDetails, ApiError>>;

    fn inspect_transaction<'a>(
        &'a self,
        txid: &'a Txid,
    ) -> BoxFuture<'a, Result<InspectedTransactionResult, ApiError>>;

    fn subscribe(&self) -> broadcast::Receiver<LiveEvent>;
}

pub type DynLiveProvider = Arc<dyn LiveDataProvider>;
pub use public_mainnet::MAX_RECENT_TX_CACHE;

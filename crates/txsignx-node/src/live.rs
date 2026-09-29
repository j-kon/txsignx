use serde::{Deserialize, Serialize};

pub const MAX_LIVE_TRANSACTIONS: usize = 200;
pub const MAX_RECENT_BLOCKS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentBlockSummary {
    pub height: u64,
    pub hash: String,
    pub tx_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolSummary {
    pub tx_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_fee_sats: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveTransactionSummary {
    pub txid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wtxid: Option<String>,
    pub vsize: u64,
    pub weight: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_sats: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_rate: Option<f64>,
    pub input_count: usize,
    pub output_count: usize,
    pub explicit_rbf: bool,
    pub has_witness: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_seen_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveSnapshot {
    pub network: String,
    pub tip_height: u64,
    pub tip_hash: String,
    pub recent_blocks: Vec<RecentBlockSummary>,
    pub mempool_tx_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mempool_size_bytes: Option<u64>,
    pub latest_transactions: Vec<LiveTransactionSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum LiveEvent {
    Snapshot(LiveSnapshot),
    TransactionAdded(LiveTransactionSummary),
    TransactionRemoved {
        txid: String,
    },
    TransactionConfirmed {
        txid: String,
        block_hash: String,
        block_height: u64,
    },
    BlockConnected(RecentBlockSummary),
}

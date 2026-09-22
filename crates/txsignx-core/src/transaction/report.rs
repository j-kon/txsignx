use super::ScriptType;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeeRateReport {
    pub fee_sats: u64,
    pub vsize_vb: usize,
    pub sat_per_vb: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionConfirmationStatus {
    Confirmed,
    Mempool,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionChainContext {
    pub network: String,
    pub status: TransactionConfirmationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedPrevout {
    pub value_sats: u64,
    pub script_pubkey_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_pubkey_asm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

/// Context-free observations, not a consensus-validity or safety verdict.
#[derive(Debug, Clone, Serialize)]
pub struct TransactionReport {
    pub txid: String,
    pub wtxid: String,
    pub version: i32,
    /// Raw nLockTime value; no chain height or time context is inferred.
    pub locktime: u32,
    pub input_count: usize,
    pub output_count: usize,
    pub size_bytes: usize,
    pub weight_wu: u64,
    pub vsize_vb: usize,
    pub has_witness: bool,
    /// True if at least one input has nSequence < 0xfffffffe.
    pub explicit_rbf: bool,
    pub total_output_sats: u64,
    /// Always None for raw-only analysis: spent output values are unavailable.
    pub fee_sats: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_input_sats: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_rate: Option<FeeRateReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_context: Option<TransactionChainContext>,
    pub inputs: Vec<InputReport>,
    pub outputs: Vec<OutputReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InputReport {
    pub index: usize,
    pub previous_txid: String,
    pub previous_vout: u32,
    pub sequence: u32,
    pub script_sig_hex: String,
    pub script_sig_size_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_sig_asm: Option<String>,
    pub witness_item_count: usize,
    pub witness_items: Vec<WitnessItemReport>,
    pub explicit_rbf: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_prevout: Option<ResolvedPrevout>,
}

/// Witness bytes are represented only as hex, never interpreted as terminal text.
#[derive(Debug, Clone, Serialize)]
pub struct WitnessItemReport {
    pub index: usize,
    pub size_bytes: usize,
    pub hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputReport {
    pub index: usize,
    pub value_sats: u64,
    pub script_pubkey_hex: String,
    pub script_pubkey_size_bytes: usize,
    pub script_type: ScriptType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_pubkey_asm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

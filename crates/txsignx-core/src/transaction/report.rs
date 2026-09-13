use super::ScriptType;
use serde::Serialize;

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
    pub witness_item_count: usize,
    pub witness_items: Vec<WitnessItemReport>,
    pub explicit_rbf: bool,
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
}

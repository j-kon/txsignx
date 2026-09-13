use super::{PsbtFeeReport, PsbtSigningState, PsbtUtxoReport};
use crate::transaction::OutputReport;
use serde::Serialize;

/// Structural facts from supplied PSBT v0 metadata, not a validity verdict.
#[derive(Debug, Clone, Serialize)]
pub struct PsbtReport {
    pub psbt_version: u32,
    pub format: String,
    pub unsigned_txid: String,
    pub transaction_version: i32,
    pub locktime: u32,
    pub input_count: usize,
    pub output_count: usize,
    pub total_output_sats: u64,
    pub explicit_rbf: bool,
    pub global_xpub_count: usize,
    pub proprietary_count: usize,
    pub unknown_count: usize,
    pub signing_state: PsbtSigningState,
    pub fee: PsbtFeeReport,
    pub inputs: Vec<PsbtInputReport>,
    pub outputs: Vec<PsbtOutputReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PsbtSighashReport {
    pub value: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PsbtInputReport {
    pub index: usize,
    pub previous_txid: String,
    pub previous_vout: u32,
    pub sequence: u32,
    pub explicit_rbf: bool,
    pub utxo: PsbtUtxoReport,
    pub sighash_type: Option<PsbtSighashReport>,
    pub partial_ecdsa_signature_count: usize,
    pub tap_key_signature_present: bool,
    pub tap_script_signature_count: usize,
    pub bip32_derivation_count: usize,
    pub tap_key_origin_count: usize,
    pub redeem_script_present: bool,
    pub witness_script_present: bool,
    pub final_script_sig_present: bool,
    pub final_script_witness_present: bool,
    pub proprietary_count: usize,
    pub unknown_count: usize,
    pub signing_state: PsbtSigningState,
}

#[derive(Debug, Clone, Serialize)]
pub struct PsbtOutputReport {
    #[serde(flatten)]
    pub transaction_output: OutputReport,
    pub bip32_derivation_count: usize,
    pub tap_key_origin_count: usize,
    pub redeem_script_present: bool,
    pub witness_script_present: bool,
    pub tap_internal_key_present: bool,
    pub tap_tree_present: bool,
    pub proprietary_count: usize,
    pub unknown_count: usize,
}

use thiserror::Error;

/// Failures at the untrusted transaction-input boundary.
#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("raw transaction hex is empty")]
    EmptyInput,
    #[error(
        "raw transaction hex exceeds the {max_hex_chars}-character safety limit (got {actual_hex_chars})"
    )]
    InputTooLarge {
        actual_hex_chars: usize,
        max_hex_chars: usize,
    },
    #[error("raw transaction hex must contain an even number of characters (got {hex_chars})")]
    OddLength { hex_chars: usize },
    #[error("invalid hexadecimal transaction input: {0}")]
    InvalidHex(#[from] bitcoin::hex::HexToBytesError),
    #[error("invalid Bitcoin consensus transaction: {0}")]
    InvalidTransaction(#[from] bitcoin::consensus::encode::Error),
    #[error("transaction weight {weight_wu} WU exceeds the {max_weight_wu} WU safety limit")]
    WeightExceeded { weight_wu: u64, max_weight_wu: u64 },
}

use crate::{
    AnalysisError,
    limits::{MAX_TRANSACTION_HEX_CHARS, MAX_TRANSACTION_WEIGHT_WU},
};
use bitcoin::{Transaction, consensus::deserialize, hex::FromHex};

/// Decode strict hexadecimal transaction data under the documented safety limits.
pub fn decode_transaction(raw_hex: &str) -> Result<Transaction, AnalysisError> {
    if raw_hex.is_empty() {
        return Err(AnalysisError::EmptyInput);
    }
    if raw_hex.len() > MAX_TRANSACTION_HEX_CHARS {
        return Err(AnalysisError::InputTooLarge {
            actual_hex_chars: raw_hex.len(),
            max_hex_chars: MAX_TRANSACTION_HEX_CHARS,
        });
    }
    if !raw_hex.len().is_multiple_of(2) {
        return Err(AnalysisError::OddLength {
            hex_chars: raw_hex.len(),
        });
    }
    let bytes = Vec::<u8>::from_hex(raw_hex)?;
    // This API requires full consumption: trailing bytes are an error.
    let transaction: Transaction = deserialize(&bytes)?;
    let weight_wu = transaction.weight().to_wu();
    if weight_wu > MAX_TRANSACTION_WEIGHT_WU {
        return Err(AnalysisError::WeightExceeded {
            weight_wu,
            max_weight_wu: MAX_TRANSACTION_WEIGHT_WU,
        });
    }
    Ok(transaction)
}

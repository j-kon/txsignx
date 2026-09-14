use crate::{
    AnalysisError,
    limits::{MAX_REPORT_WITNESS_ITEMS, MAX_TRANSACTION_HEX_CHARS, MAX_TRANSACTION_WEIGHT_WU},
};
use bitcoin::{
    Transaction,
    consensus::deserialize,
    hex::{DisplayHex, FromHex},
};

use super::{
    InputReport, OutputReport, TransactionReport, WitnessItemReport, classify_script,
    signals_explicit_rbf,
};

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
    check_transaction_weight(&transaction)?;
    Ok(transaction)
}

fn check_transaction_weight(transaction: &Transaction) -> Result<(), AnalysisError> {
    let weight_wu = transaction.weight().to_wu();
    if weight_wu > MAX_TRANSACTION_WEIGHT_WU {
        return Err(AnalysisError::WeightExceeded {
            weight_wu,
            max_weight_wu: MAX_TRANSACTION_WEIGHT_WU,
        });
    }
    Ok(())
}

/// Analyze raw transaction facts without network, prevout, wallet or mempool context.
pub fn analyze_transaction(raw_hex: &str) -> Result<TransactionReport, AnalysisError> {
    analyze_decoded_transaction(&decode_transaction(raw_hex)?)
}

/// Inspect already-decoded transaction facts under the same report safety limits.
pub fn analyze_decoded_transaction(
    transaction: &Transaction,
) -> Result<TransactionReport, AnalysisError> {
    check_transaction_weight(transaction)?;
    // An empty witness item costs only one serialized byte but creates a report object.
    // Bound that expansion across the entire transaction before allocating reports.
    let mut remaining_items = MAX_REPORT_WITNESS_ITEMS;
    for input in &transaction.input {
        remaining_items = remaining_items.checked_sub(input.witness.len()).ok_or(
            AnalysisError::WitnessItemLimitExceeded {
                max_items: MAX_REPORT_WITNESS_ITEMS,
            },
        )?;
    }
    let total_output_sats = transaction.output.iter().try_fold(0_u64, |sum, output| {
        sum.checked_add(output.value.to_sat())
            .ok_or(AnalysisError::OutputValueOverflow)
    })?;
    let inputs: Vec<InputReport> = transaction
        .input
        .iter()
        .enumerate()
        .map(|(index, input)| InputReport {
            index,
            previous_txid: input.previous_output.txid.to_string(),
            previous_vout: input.previous_output.vout,
            sequence: input.sequence.to_consensus_u32(),
            script_sig_hex: input.script_sig.as_bytes().to_lower_hex_string(),
            script_sig_size_bytes: input.script_sig.len(),
            witness_item_count: input.witness.len(),
            witness_items: input
                .witness
                .iter()
                .enumerate()
                .map(|(index, item)| WitnessItemReport {
                    index,
                    size_bytes: item.len(),
                    hex: item.to_lower_hex_string(),
                })
                .collect(),
            explicit_rbf: signals_explicit_rbf(input.sequence.to_consensus_u32()),
        })
        .collect();
    let outputs = transaction
        .output
        .iter()
        .enumerate()
        .map(|(index, output)| OutputReport {
            index,
            value_sats: output.value.to_sat(),
            script_pubkey_hex: output.script_pubkey.as_bytes().to_lower_hex_string(),
            script_pubkey_size_bytes: output.script_pubkey.len(),
            script_type: classify_script(&output.script_pubkey),
        })
        .collect();

    Ok(TransactionReport {
        txid: transaction.compute_txid().to_string(),
        wtxid: transaction.compute_wtxid().to_string(),
        version: transaction.version.0,
        locktime: transaction.lock_time.to_consensus_u32(),
        input_count: transaction.input.len(),
        output_count: transaction.output.len(),
        size_bytes: transaction.total_size(),
        weight_wu: transaction.weight().to_wu(),
        vsize_vb: transaction.vsize(),
        has_witness: transaction
            .input
            .iter()
            .any(|input| !input.witness.is_empty()),
        explicit_rbf: inputs.iter().any(|input| input.explicit_rbf),
        total_output_sats,
        fee_sats: None,
        inputs,
        outputs,
    })
}

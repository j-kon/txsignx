use crate::{
    AnalysisError,
    limits::{MAX_REPORT_WITNESS_ITEMS, MAX_TRANSACTION_HEX_CHARS, MAX_TRANSACTION_WEIGHT_WU},
};
use bitcoin::{
    Network, Transaction, TxOut,
    consensus::deserialize,
    hex::{DisplayHex, FromHex},
};

use super::{
    FeeRateReport, InputReport, OutputReport, ResolvedPrevout, TransactionChainContext,
    TransactionReport, WitnessItemReport, classify_script, derive_address, disassemble_script,
    signals_explicit_rbf,
};

/// Optional context for transaction analysis including network, chain context, and resolved prevouts.
#[derive(Debug, Clone, Default)]
pub struct TransactionAnalysisContext {
    pub network: Option<Network>,
    pub chain_context: Option<TransactionChainContext>,
    pub resolved_prevouts: Option<Vec<Option<TxOut>>>,
}

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

/// Inspect already-decoded transaction facts without context.
pub fn analyze_decoded_transaction(
    transaction: &Transaction,
) -> Result<TransactionReport, AnalysisError> {
    analyze_decoded_transaction_with_context(transaction, None)
}

/// Inspect already-decoded transaction facts with optional chain and prevout context.
pub fn analyze_decoded_transaction_with_context(
    transaction: &Transaction,
    context: Option<&TransactionAnalysisContext>,
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

    let network = context.and_then(|c| c.network);
    let prevouts = context.and_then(|c| c.resolved_prevouts.as_ref());
    if let Some(prevouts) = prevouts
        && prevouts.len() != transaction.input.len()
    {
        return Err(AnalysisError::PrevoutCountMismatch);
    }

    let is_coinbase = transaction.is_coinbase();

    let inputs: Vec<InputReport> = transaction
        .input
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let script_sig_asm = if input.script_sig.is_empty() {
                None
            } else {
                Some(disassemble_script(&input.script_sig))
            };
            let resolved_prevout = if is_coinbase || input.previous_output.is_null() {
                None
            } else if let Some(prevouts) = prevouts {
                prevouts
                    .get(index)
                    .and_then(|opt| opt.as_ref())
                    .map(|txout| {
                        let script_pubkey_hex =
                            txout.script_pubkey.as_bytes().to_lower_hex_string();
                        let script_pubkey_asm = if txout.script_pubkey.is_empty() {
                            None
                        } else {
                            Some(disassemble_script(&txout.script_pubkey))
                        };
                        let address =
                            network.and_then(|net| derive_address(&txout.script_pubkey, net));
                        ResolvedPrevout {
                            value_sats: txout.value.to_sat(),
                            script_pubkey_hex,
                            script_pubkey_asm,
                            address,
                        }
                    })
            } else {
                None
            };

            InputReport {
                index,
                previous_txid: input.previous_output.txid.to_string(),
                previous_vout: input.previous_output.vout,
                sequence: input.sequence.to_consensus_u32(),
                script_sig_hex: input.script_sig.as_bytes().to_lower_hex_string(),
                script_sig_size_bytes: input.script_sig.len(),
                script_sig_asm,
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
                resolved_prevout,
            }
        })
        .collect();

    let outputs: Vec<OutputReport> = transaction
        .output
        .iter()
        .enumerate()
        .map(|(index, output)| {
            let script_pubkey_asm = if output.script_pubkey.is_empty() {
                None
            } else {
                Some(disassemble_script(&output.script_pubkey))
            };
            let address = network.and_then(|net| derive_address(&output.script_pubkey, net));
            OutputReport {
                index,
                value_sats: output.value.to_sat(),
                script_pubkey_hex: output.script_pubkey.as_bytes().to_lower_hex_string(),
                script_pubkey_size_bytes: output.script_pubkey.len(),
                script_type: classify_script(&output.script_pubkey),
                script_pubkey_asm,
                address,
            }
        })
        .collect();

    // Fee calculation: only possible when all non-coinbase inputs have resolved prevouts
    let (total_input_sats, fee_sats, fee_rate) = if is_coinbase {
        (None, None, None)
    } else if let Some(prevouts) = prevouts {
        let all_resolved = !inputs.is_empty() && prevouts.iter().all(|p| p.is_some());
        if all_resolved {
            let mut input_sum = 0_u64;
            for p in prevouts.iter().flatten() {
                input_sum = input_sum
                    .checked_add(p.value.to_sat())
                    .ok_or(AnalysisError::InputValueOverflow)?;
            }
            if total_output_sats > input_sum {
                return Err(AnalysisError::OutputExceedsInput {
                    output_sats: total_output_sats,
                    input_sats: input_sum,
                });
            }
            let fee = input_sum - total_output_sats;
            let vsize = transaction.vsize();
            let sat_per_vb = if vsize > 0 {
                let raw_rate = (fee as f64) / (vsize as f64);
                (raw_rate * 100.0).round() / 100.0
            } else {
                0.0
            };
            (
                Some(input_sum),
                Some(fee),
                Some(FeeRateReport {
                    fee_sats: fee,
                    vsize_vb: vsize,
                    sat_per_vb,
                }),
            )
        } else {
            (None, None, None)
        }
    } else {
        (None, None, None)
    };

    let chain_context = context.and_then(|c| c.chain_context.clone());

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
        fee_sats,
        total_input_sats,
        fee_rate,
        chain_context,
        inputs,
        outputs,
    })
}

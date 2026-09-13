use super::{
    PsbtError, PsbtInputReport, PsbtOutputReport, PsbtReport, PsbtSighashReport, decode_psbt, fee,
    signing, utxo,
};
use crate::transaction::analyze_decoded_transaction;

/// Inspect bounded standard base64 PSBT v0 text. No PSBT is modified and no
/// signature, wallet ownership, network, or chain membership is verified.
pub fn analyze_psbt(text: &str) -> Result<PsbtReport, PsbtError> {
    let psbt = decode_psbt(text)?;
    let tx = analyze_decoded_transaction(&psbt.unsigned_tx)?;
    // Defensive invariant before zipping; the current parser already enforces it.
    if psbt.inputs.len() != tx.input_count || psbt.outputs.len() != tx.output_count {
        return Err(PsbtError::InvalidPsbt);
    }
    let inputs: Vec<_> = psbt
        .inputs
        .iter()
        .zip(tx.inputs)
        .zip(&psbt.unsigned_tx.input)
        .map(|((input, facts), txin)| PsbtInputReport {
            index: facts.index,
            previous_txid: facts.previous_txid,
            previous_vout: facts.previous_vout,
            sequence: facts.sequence,
            explicit_rbf: facts.explicit_rbf,
            utxo: utxo::inspect(input, &txin.previous_output),
            sighash_type: input.sighash_type.map(|s| PsbtSighashReport {
                value: s.to_u32(),
                name: s.to_string(),
            }),
            partial_ecdsa_signature_count: input.partial_sigs.len(),
            tap_key_signature_present: input.tap_key_sig.is_some(),
            tap_script_signature_count: input.tap_script_sigs.len(),
            bip32_derivation_count: input.bip32_derivation.len(),
            tap_key_origin_count: input.tap_key_origins.len(),
            redeem_script_present: input.redeem_script.is_some(),
            witness_script_present: input.witness_script.is_some(),
            final_script_sig_present: input.final_script_sig.is_some(),
            final_script_witness_present: input.final_script_witness.is_some(),
            proprietary_count: input.proprietary.len(),
            unknown_count: input.unknown.len(),
            signing_state: signing::input_state(input),
        })
        .collect();
    let outputs = psbt
        .outputs
        .iter()
        .zip(tx.outputs)
        .map(|(output, facts)| PsbtOutputReport {
            transaction_output: facts,
            bip32_derivation_count: output.bip32_derivation.len(),
            tap_key_origin_count: output.tap_key_origins.len(),
            redeem_script_present: output.redeem_script.is_some(),
            witness_script_present: output.witness_script.is_some(),
            tap_internal_key_present: output.tap_internal_key.is_some(),
            tap_tree_present: output.tap_tree.is_some(),
            proprietary_count: output.proprietary.len(),
            unknown_count: output.unknown.len(),
        })
        .collect();
    Ok(PsbtReport {
        psbt_version: psbt.version,
        format: "BIP174".into(),
        unsigned_txid: tx.txid,
        transaction_version: tx.version,
        locktime: tx.locktime,
        input_count: tx.input_count,
        output_count: tx.output_count,
        total_output_sats: tx.total_output_sats,
        explicit_rbf: tx.explicit_rbf,
        global_xpub_count: psbt.xpub.len(),
        proprietary_count: psbt.proprietary.len(),
        unknown_count: psbt.unknown.len(),
        signing_state: signing::overall(inputs.iter().map(|i| i.signing_state)),
        fee: fee::inspect(&psbt),
        inputs,
        outputs,
    })
}

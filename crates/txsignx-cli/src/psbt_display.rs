use std::io::{self, Write};
use txsignx_core::psbt::{
    PsbtFeeStatus, PsbtReport, PsbtSigningState, PsbtUtxoSource, PsbtUtxoStatus,
};

fn state(state: PsbtSigningState) -> &'static str {
    match state {
        PsbtSigningState::Unsigned => "Unsigned",
        PsbtSigningState::PartiallySigned => "Partially Signed",
        PsbtSigningState::Finalized => "Finalized (structural markers)",
        PsbtSigningState::Mixed => "Mixed",
    }
}
fn source(source: PsbtUtxoSource) -> &'static str {
    match source {
        PsbtUtxoSource::Missing => "missing",
        PsbtUtxoSource::WitnessUtxo => "witness_utxo",
        PsbtUtxoSource::NonWitnessUtxo => "non_witness_utxo",
        PsbtUtxoSource::Both => "both",
    }
}
fn consistency(status: PsbtUtxoStatus) -> &'static str {
    match status {
        PsbtUtxoStatus::Missing => "missing",
        PsbtUtxoStatus::Valid => "valid (supplied metadata is consistent)",
        PsbtUtxoStatus::TxidMismatch => "txid_mismatch",
        PsbtUtxoStatus::VoutOutOfRange => "vout_out_of_range",
        PsbtUtxoStatus::WitnessNonWitnessMismatch => "witness_non_witness_mismatch",
    }
}
pub fn write_report(out: &mut impl Write, report: &PsbtReport) -> io::Result<()> {
    writeln!(
        out,
        "TxSignX PSBT Inspection\n------------------------------------"
    )?;
    writeln!(
        out,
        "PSBT version: {} ({})",
        report.psbt_version, report.format
    )?;
    writeln!(out, "Unsigned TXID: {}", report.unsigned_txid)?;
    writeln!(
        out,
        "Transaction version: {}\nLocktime: {}",
        report.transaction_version, report.locktime
    )?;
    writeln!(
        out,
        "Inputs: {}\nOutputs: {}",
        report.input_count, report.output_count
    )?;
    writeln!(out, "Explicit RBF: {}", report.explicit_rbf)?;
    writeln!(out, "Signing state: {}", state(report.signing_state))?;
    writeln!(out, "Output total: {} sats", report.total_output_sats)?;
    if let Some(fee) = report.fee.fee_sats {
        writeln!(out, "Fee: {fee} sats (from supplied UTXO context)")?;
    } else {
        let reason = match report.fee.status {
            PsbtFeeStatus::MissingUtxoContext => "missing UTXO information",
            PsbtFeeStatus::InvalidUtxoContext => "inconsistent UTXO information",
            PsbtFeeStatus::NegativeFee => "outputs exceed inputs",
            PsbtFeeStatus::Overflow => "value arithmetic overflow",
            _ => "fee calculation error",
        };
        writeln!(out, "Fee: unavailable — {reason}")?;
    }
    writeln!(
        out,
        "Fee rate: unavailable until final transaction size is known"
    )?;
    writeln!(
        out,
        "Global xpubs: {}\nUnknown globals: {}\nProprietary globals: {}",
        report.global_xpub_count, report.unknown_count, report.proprietary_count
    )?;
    writeln!(out, "\nInputs")?;
    for input in &report.inputs {
        writeln!(
            out,
            "\nInput {}\n  Previous output: {}:{}",
            input.index, input.previous_txid, input.previous_vout
        )?;
        writeln!(
            out,
            "  Sequence: {}\n  Explicit RBF: {}",
            input.sequence, input.explicit_rbf
        )?;
        writeln!(
            out,
            "  UTXO source: {}\n  UTXO consistency: {}",
            source(input.utxo.source),
            consistency(input.utxo.status)
        )?;
        if let Some(value) = input.utxo.value_sats {
            writeln!(out, "  Previous value: {value} sats")?;
        }
        if let Some(script) = input.utxo.script_type {
            writeln!(
                out,
                "  Previous script: {}",
                crate::display::script_name(script)
            )?;
        }
        if let Some(hex) = &input.utxo.script_pubkey_hex {
            writeln!(out, "  Previous script hex: {hex}")?;
        }
        if let Some(sighash) = &input.sighash_type {
            writeln!(out, "  Sighash: {} ({})", sighash.name, sighash.value)?;
        } else {
            writeln!(out, "  Sighash: not specified")?;
        }
        writeln!(
            out,
            "  Partial ECDSA signatures: {}\n  Taproot key signature: {}\n  Taproot script signatures: {}",
            input.partial_ecdsa_signature_count,
            input.tap_key_signature_present,
            input.tap_script_signature_count
        )?;
        writeln!(
            out,
            "  BIP32 derivations: {}\n  Taproot key origins: {}",
            input.bip32_derivation_count, input.tap_key_origin_count
        )?;
        writeln!(
            out,
            "  Redeem script: {}\n  Witness script: {}",
            input.redeem_script_present, input.witness_script_present
        )?;
        writeln!(
            out,
            "  Final scriptSig: {}\n  Final scriptWitness: {}",
            input.final_script_sig_present, input.final_script_witness_present
        )?;
        writeln!(
            out,
            "  Unknown: {}\n  Proprietary: {}\n  Signing state: {}",
            input.unknown_count,
            input.proprietary_count,
            state(input.signing_state)
        )?;
    }
    writeln!(out, "\nOutputs")?;
    for output in &report.outputs {
        let tx = &output.transaction_output;
        writeln!(
            out,
            "\nOutput {}\n  Value: {} sats\n  Script: {}\n  Script bytes: {}\n  Script hex: {}",
            tx.index,
            tx.value_sats,
            crate::display::script_name(tx.script_type),
            tx.script_pubkey_size_bytes,
            tx.script_pubkey_hex
        )?;
        writeln!(
            out,
            "  BIP32 derivations: {}\n  Taproot key origins: {}",
            output.bip32_derivation_count, output.tap_key_origin_count
        )?;
        writeln!(
            out,
            "  Redeem script: {}\n  Witness script: {}\n  Taproot internal key: {}\n  Taproot tree: {}",
            output.redeem_script_present,
            output.witness_script_present,
            output.tap_internal_key_present,
            output.tap_tree_present
        )?;
        writeln!(
            out,
            "  Unknown: {}\n  Proprietary: {}",
            output.unknown_count, output.proprietary_count
        )?;
    }
    writeln!(
        out,
        "\nSecurity note: Inspection is structural. Signature validity, wallet ownership,\nnetwork correctness, and broadcast readiness are not established."
    )
}

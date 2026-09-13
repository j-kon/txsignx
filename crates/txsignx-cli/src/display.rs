use std::io::{self, Write};
use txsignx_core::{TransactionReport, transaction::ScriptType};

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

pub(crate) fn script_name(script_type: ScriptType) -> &'static str {
    match script_type {
        ScriptType::P2pkh => "P2PKH",
        ScriptType::P2sh => "P2SH",
        ScriptType::P2wpkh => "P2WPKH",
        ScriptType::P2wsh => "P2WSH",
        ScriptType::P2tr => "P2TR",
        ScriptType::OpReturn => "OP_RETURN",
        ScriptType::Unknown => "Unknown",
    }
}

pub fn write_human_report(out: &mut impl Write, report: &TransactionReport) -> io::Result<()> {
    writeln!(out, "TxSignX Transaction Analysis")?;
    writeln!(out, "------------------------------------")?;
    writeln!(out, "TXID: {}", report.txid)?;
    writeln!(out, "wTXID: {}", report.wtxid)?;
    writeln!(out, "Version: {}", report.version)?;
    writeln!(out, "Locktime: {}", report.locktime)?;
    writeln!(out, "Inputs: {}", report.input_count)?;
    writeln!(out, "Outputs: {}", report.output_count)?;
    writeln!(out, "Size: {} bytes", report.size_bytes)?;
    writeln!(out, "Weight: {} WU", report.weight_wu)?;
    writeln!(out, "Virtual size: {} vB", report.vsize_vb)?;
    writeln!(out, "Witness: {}", yes_no(report.has_witness))?;
    writeln!(
        out,
        "Explicit RBF signaling: {}",
        yes_no(report.explicit_rbf)
    )?;
    writeln!(out, "Output total: {} sats", report.total_output_sats)?;
    match report.fee_sats {
        Some(fee) => writeln!(out, "Fee: {fee} sats")?,
        None => writeln!(out, "Fee: unavailable without prevout context")?,
    }
    writeln!(out, "\nInputs")?;
    for input in &report.inputs {
        writeln!(out, "  Input {}", input.index)?;
        writeln!(
            out,
            "    Previous output: {}:{}",
            input.previous_txid, input.previous_vout
        )?;
        writeln!(
            out,
            "    Sequence: {} ({:#010x})",
            input.sequence, input.sequence
        )?;
        writeln!(
            out,
            "    Explicit RBF signaling: {}",
            yes_no(input.explicit_rbf)
        )?;
        writeln!(
            out,
            "    scriptSig ({} bytes): {}",
            input.script_sig_size_bytes, input.script_sig_hex
        )?;
        writeln!(out, "    Witness items: {}", input.witness_item_count)?;
        for item in &input.witness_items {
            writeln!(
                out,
                "      {} ({} bytes): {}",
                item.index, item.size_bytes, item.hex
            )?;
        }
    }
    writeln!(out, "\nOutputs")?;
    for output in &report.outputs {
        writeln!(
            out,
            "  Output {}: {} sats ({})",
            output.index,
            output.value_sats,
            script_name(output.script_type)
        )?;
        writeln!(
            out,
            "    scriptPubKey ({} bytes): {}",
            output.script_pubkey_size_bytes, output.script_pubkey_hex
        )?;
    }
    writeln!(
        out,
        "\nObservations only; consensus validity and mempool replaceability are not verified."
    )?;
    Ok(())
}

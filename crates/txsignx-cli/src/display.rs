use std::io::{self, Write};
use txsignx_core::{
    TransactionReport,
    transaction::{ScriptType, TransactionConfirmationStatus},
};

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
    writeln!(out, "============================")?;

    writeln!(out, "\nTransaction")?;
    writeln!(out, "-----------")?;
    writeln!(out, "TXID: {}", report.txid)?;
    writeln!(out, "wTXID: {}", report.wtxid)?;
    writeln!(out, "Version: {}", report.version)?;
    writeln!(out, "Locktime: {}", report.locktime)?;
    writeln!(out, "Inputs: {}", report.input_count)?;
    writeln!(out, "Outputs: {}", report.output_count)?;
    writeln!(out, "Witness: {}", yes_no(report.has_witness))?;
    writeln!(out, "SegWit: {}", yes_no(report.has_witness))?;
    writeln!(
        out,
        "Explicit RBF signaling: {}",
        yes_no(report.explicit_rbf)
    )?;

    if let Some(chain) = &report.chain_context {
        writeln!(out, "\nChain context")?;
        writeln!(out, "-------------")?;
        writeln!(out, "Network: {}", chain.network)?;
        match chain.status {
            TransactionConfirmationStatus::Confirmed => {
                writeln!(out, "Status: confirmed")?;
                if let Some(c) = chain.confirmations {
                    writeln!(out, "Confirmations: {c}")?;
                }
                if let Some(b) = &chain.block_hash {
                    writeln!(out, "Block hash: {b}")?;
                }
            }
            TransactionConfirmationStatus::Mempool => {
                writeln!(out, "Status: unconfirmed / mempool")?;
            }
            TransactionConfirmationStatus::Unavailable => {
                writeln!(out, "Status: unavailable")?;
            }
        }
    }

    writeln!(out, "\nSize")?;
    writeln!(out, "----")?;
    writeln!(out, "Size: {} bytes", report.size_bytes)?;
    writeln!(out, "Weight: {} WU", report.weight_wu)?;
    writeln!(out, "Virtual size: {} vB", report.vsize_vb)?;

    writeln!(out, "\nFees")?;
    writeln!(out, "----")?;
    if let Some(input_total) = report.total_input_sats {
        writeln!(out, "Input total: {input_total} sats")?;
        writeln!(out, "Output total: {} sats", report.total_output_sats)?;
        if let Some(fee) = report.fee_sats {
            writeln!(out, "Fee: {fee} sats")?;
        }
        if let Some(rate) = &report.fee_rate {
            writeln!(out, "Fee rate: {:.2} sat/vB", rate.sat_per_vb)?;
        }
    } else {
        writeln!(out, "Output total: {} sats", report.total_output_sats)?;
        writeln!(out, "Fee: unavailable without prevout context")?;
    }

    writeln!(out, "\nInputs")?;
    writeln!(out, "------")?;
    for input in &report.inputs {
        writeln!(out, "  Input {}", input.index)?;
        writeln!(
            out,
            "    Previous output: {}:{}",
            input.previous_txid, input.previous_vout
        )?;
        if let Some(prevout) = &input.resolved_prevout {
            writeln!(
                out,
                "    Previous output value: {} sats",
                prevout.value_sats
            )?;
            if let Some(addr) = &prevout.address {
                writeln!(out, "    Previous output address: {addr}")?;
            }
        }
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
        if let Some(asm) = &input.script_sig_asm {
            if !asm.is_empty() {
                writeln!(out, "      scriptSig asm: {asm}")?;
            }
        }
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
    writeln!(out, "-------")?;
    for output in &report.outputs {
        writeln!(
            out,
            "  Output {}: {} sats ({})",
            output.index,
            output.value_sats,
            script_name(output.script_type)
        )?;
        if let Some(addr) = &output.address {
            writeln!(out, "    Address: {addr}")?;
        }
        writeln!(
            out,
            "    scriptPubKey ({} bytes): {}",
            output.script_pubkey_size_bytes, output.script_pubkey_hex
        )?;
        if let Some(asm) = &output.script_pubkey_asm {
            if !asm.is_empty() {
                writeln!(out, "      scriptPubKey asm: {asm}")?;
            }
        }
    }
    writeln!(
        out,
        "\nObservations only; consensus validity and mempool replaceability are not verified."
    )?;
    Ok(())
}

use std::io::{self, Write};
use txsignx_wallet::{WalletContextReport, WalletOwnership};
fn ownership(value: WalletOwnership) -> String {
    match value {
        WalletOwnership::External { derivation_index } => {
            format!("External wallet match, index {derivation_index}")
        }
        WalletOwnership::Internal { derivation_index } => {
            format!("Internal wallet match, index {derivation_index}")
        }
        WalletOwnership::NoMatchWithinWindow => "No wallet match within window".into(),
        WalletOwnership::Unavailable { reason } => format!("Unavailable: {reason:?}"),
    }
}
pub fn write_report(out: &mut impl Write, context: &WalletContextReport) -> io::Result<()> {
    writeln!(
        out,
        "\nWallet context\n  Configured network: {}\n  Derivation window: {}",
        context.configured_network().name(),
        context.derivation_window()
    )?;
    writeln!(out, "Inputs")?;
    for input in context.inputs() {
        writeln!(out, "  [{}] {}", input.index, ownership(input.ownership))?;
    }
    writeln!(out, "Outputs")?;
    for output in context.outputs() {
        writeln!(
            out,
            "  [{}] {}{}",
            output.index,
            ownership(output.ownership),
            if output.expected_change {
                " (expected change)"
            } else {
                ""
            }
        )?;
    }
    Ok(())
}

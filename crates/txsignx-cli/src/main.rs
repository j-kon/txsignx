use std::{
    error::Error,
    io::{self, BufWriter, Write},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use txsignx_core::analyze_transaction;

mod display;

#[derive(Parser)]
#[command(name = "txsignx", version, about = "Bitcoin transaction security before signing.", color = clap::ColorChoice::Never)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Inspect raw Bitcoin transactions.
    Tx {
        #[command(subcommand)]
        command: TransactionCommand,
    },
}

#[derive(Subcommand)]
enum TransactionCommand {
    /// Analyze a consensus-serialized transaction without network or prevout context.
    Inspect {
        /// Raw transaction as strict hex, without whitespace or a 0x prefix.
        #[arg(value_name = "RAW_TX_HEX")]
        raw_tx_hex: String,
        /// Emit only a JSON report on stdout (diagnostics remain on stderr).
        #[arg(long)]
        json: bool,
    },
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Command::Tx {
            command: TransactionCommand::Inspect { raw_tx_hex, json },
        } => {
            // Analyze before writing anything, so malformed input leaves stdout empty.
            let report = analyze_transaction(&raw_tx_hex)?;
            let mut stdout = BufWriter::new(io::stdout().lock());
            if json {
                serde_json::to_writer_pretty(&mut stdout, &report)?;
                writeln!(stdout)?;
            } else {
                display::write_human_report(&mut stdout, &report)?;
            }
            stdout.flush()?;
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Do not echo the raw transaction or panic if stderr itself is unavailable.
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            ExitCode::FAILURE
        }
    }
}

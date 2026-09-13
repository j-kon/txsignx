use std::{
    error::Error,
    io::{self, BufWriter, Write},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use txsignx_core::{analyze_psbt, analyze_transaction};

mod display;
mod psbt_display;
mod psbt_input;

#[derive(Parser)]
#[command(name = "txsignx", version, about = "Bitcoin transaction security before signing.", color = clap::ColorChoice::Never)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Inspect PSBT v0 / BIP174 metadata.
    Psbt {
        #[command(subcommand)]
        command: PsbtCommand,
    },
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

#[derive(Subcommand)]
enum PsbtCommand {
    /// Inspect standard base64 PSBT v0 without modifying or signing it.
    Inspect {
        #[command(flatten)]
        source: psbt_input::PsbtSource,
        #[arg(long)]
        json: bool,
    },
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Command::Psbt {
            command: PsbtCommand::Inspect { source, json },
        } => {
            let text = psbt_input::read(source)?;
            let report = analyze_psbt(&text)?;
            let mut stdout = BufWriter::new(io::stdout().lock());
            if json {
                serde_json::to_writer_pretty(&mut stdout, &report)?;
                writeln!(stdout)?;
            } else {
                psbt_display::write_report(&mut stdout, &report)?;
            }
            stdout.flush()?;
        }
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
    // Clap's default diagnostics may echo positional values. Keep parse failures
    // independent of supplied PSBT contents; help/version contain only static text.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                return ExitCode::SUCCESS;
            }
            let _ = writeln!(
                io::stderr().lock(),
                "error: invalid command arguments; choose exactly one PSBT source (text, --file, --stdin); run txsignx --help for usage"
            );
            return ExitCode::from(2);
        }
    };
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Do not echo the raw transaction or panic if stderr itself is unavailable.
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            ExitCode::FAILURE
        }
    }
}

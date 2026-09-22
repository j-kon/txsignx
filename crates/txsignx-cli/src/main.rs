use std::{
    error::Error,
    io::{self, BufWriter, Write},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use txsignx_core::analyze_psbt;

mod banner;
mod display;
mod node_display;
mod node_input;
mod policy_display;
mod preflight;
mod psbt_display;
mod psbt_input;
mod style;
mod wallet_display;
mod wallet_input;

#[derive(Parser)]
#[command(name = "txsignx", version, about = "Bitcoin transaction security before signing.", color = clap::ColorChoice::Never)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List active development policy rules and deferred context requirements.
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
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
    /// Analyze a consensus-serialized transaction or fetch by txid from Bitcoin Core.
    Inspect {
        /// Raw transaction as strict hex, without whitespace or a 0x prefix.
        #[arg(value_name = "RAW_TX_HEX")]
        raw_tx_hex: Option<String>,
        /// Transaction ID to fetch from a configured Bitcoin Core node.
        #[arg(long, value_name = "TXID")]
        txid: Option<String>,
        /// Exact local HTTP endpoint: http://127.0.0.1:PORT or http://[::1]:PORT.
        #[arg(long, value_name = "URL", visible_alias = "rpc-url")]
        node_url: Option<String>,
        /// Bitcoin Core cookie authentication file; contents are never reported.
        #[arg(long, value_name = "PATH", visible_alias = "rpc-cookie-file")]
        cookie_file: Option<std::path::PathBuf>,
        /// Bitcoin network: bitcoin, mainnet, testnet, testnet4, signet, regtest.
        #[arg(long, value_name = "NETWORK")]
        network: Option<String>,
        /// Emit only a JSON report on stdout (diagnostics remain on stderr).
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum PsbtCommand {
    /// Evaluate deterministic development policy (exit 0 PASS, 2 REVIEW, 3 BLOCK).
    Preflight(Box<preflight::PreflightArgs>),
    /// Broadcast an externally finalized PSBT only after Regtest wallet/node PASS gates.
    Broadcast(Box<preflight::PreflightArgs>),
    /// Inspect standard base64 PSBT v0 without modifying or signing it.
    Inspect {
        #[command(flatten)]
        source: psbt_input::PsbtSource,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum PolicyCommand {
    /// List rule metadata; reserved rules are not evaluated.
    List {
        #[arg(long)]
        json: bool,
    },
}

fn run(cli: Cli) -> Result<ExitCode, Box<dyn Error>> {
    let Some(command) = cli.command else {
        let mut stdout = BufWriter::new(io::stdout().lock());
        banner::write_banner(
            &mut stdout,
            banner::should_use_color(),
            env!("CARGO_PKG_VERSION"),
        )?;
        stdout.flush()?;
        return Ok(ExitCode::SUCCESS);
    };
    match command {
        Command::Policy {
            command: PolicyCommand::List { json },
        } => return preflight::list(json),
        Command::Psbt {
            command: PsbtCommand::Preflight(args),
        } => return preflight::run(*args, false),
        Command::Psbt {
            command: PsbtCommand::Broadcast(args),
        } => return preflight::run(*args, true),
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
            command:
                TransactionCommand::Inspect {
                    raw_tx_hex,
                    txid,
                    node_url,
                    cookie_file,
                    network,
                    json,
                },
        } => {
            let report = match (raw_tx_hex, txid) {
                (Some(_), Some(_)) => {
                    return Err("cannot specify both raw transaction hex and --txid".into());
                }
                (None, None) => {
                    return Err("must specify either raw transaction hex or --txid".into());
                }
                (Some(raw_hex), None) => {
                    if node_url.is_some() || cookie_file.is_some() {
                        return Err("node options are only supported with --txid".into());
                    }
                    let parsed_network = match network.as_deref() {
                        Some(net_str) => {
                            let conf: txsignx_wallet::ConfiguredNetwork = net_str.parse()?;
                            Some(conf.bitcoin_network())
                        }
                        None => None,
                    };
                    let tx = txsignx_core::decode_transaction(&raw_hex)?;
                    let context = txsignx_core::transaction::TransactionAnalysisContext {
                        network: parsed_network,
                        chain_context: None,
                        resolved_prevouts: None,
                    };
                    txsignx_core::transaction::analyze_decoded_transaction_with_context(
                        &tx,
                        Some(&context),
                    )?
                }
                (None, Some(txid_str)) => {
                    let (Some(url), Some(cookie_path), Some(net_str)) = (
                        node_url.as_deref(),
                        cookie_file.as_deref(),
                        network.as_deref(),
                    ) else {
                        return Err(
                            "--txid requires explicit --node-url, --cookie-file, and --network"
                                .into(),
                        );
                    };
                    let conf_net: txsignx_wallet::ConfiguredNetwork = net_str.parse()?;
                    let bitcoin_net = conf_net.bitcoin_network();
                    let txid: bitcoin::Txid =
                        txid_str.parse().map_err(|_| "invalid txid format")?;

                    let client = txsignx_node::BitcoinCoreRpc::new(url, cookie_path)?;
                    let (node_tx, prevouts, _) =
                        txsignx_node::fetch_transaction_with_context(&client, &txid, bitcoin_net)?;

                    let status = if node_tx.confirmations.unwrap_or(0) > 0 {
                        txsignx_core::transaction::TransactionConfirmationStatus::Confirmed
                    } else {
                        txsignx_core::transaction::TransactionConfirmationStatus::Mempool
                    };
                    let chain_context = txsignx_core::transaction::TransactionChainContext {
                        network: conf_net.name().to_string(),
                        status,
                        confirmations: node_tx.confirmations,
                        block_hash: node_tx.block_hash.map(|h| h.to_string()),
                    };
                    let context = txsignx_core::transaction::TransactionAnalysisContext {
                        network: Some(bitcoin_net),
                        chain_context: Some(chain_context),
                        resolved_prevouts: Some(prevouts),
                    };
                    txsignx_core::transaction::analyze_decoded_transaction_with_context(
                        &node_tx.transaction,
                        Some(&context),
                    )?
                }
            };

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
    Ok(ExitCode::SUCCESS)
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
                "error: invalid command arguments; run txsignx --help for usage"
            );
            return ExitCode::FAILURE;
        }
    };
    match run(cli) {
        Ok(code) => code,
        Err(error) => {
            // Do not echo the raw transaction or panic if stderr itself is unavailable.
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            ExitCode::FAILURE
        }
    }
}

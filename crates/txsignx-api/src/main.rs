use clap::Parser;
use std::{net::SocketAddr, path::PathBuf, process::ExitCode};
use txsignx_api::{Config, ConfiguredNode, app_with_config};
#[derive(Parser)]
#[command(version, about = "Local Bitcoin transaction inspection API", color = clap::ColorChoice::Never)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: SocketAddr,
    /// Required to bind an address other than loopback. No authentication is provided.
    #[arg(long)]
    allow_external: bool,
    /// Exact browser origin; repeat to allow multiple origins. No wildcards.
    #[arg(long)]
    allowed_origin: Vec<String>,
    /// Exact loopback HTTP Bitcoin Core RPC endpoint.
    #[arg(long)]
    rpc_url: Option<String>,
    /// Server-owned Bitcoin Core cookie file; never accepted from HTTP clients.
    #[arg(long)]
    rpc_cookie_file: Option<PathBuf>,
    /// Configured node network: bitcoin, testnet, testnet4, signet, regtest.
    #[arg(long)]
    network: Option<String>,
}
fn configuration(args: &Args) -> Result<Config, ()> {
    if !args.bind.ip().is_loopback() && !args.allow_external {
        return Err(());
    }
    let mut config = Config::default();
    if !args.allowed_origin.is_empty() {
        for origin in &args.allowed_origin {
            let uri: axum::http::Uri = origin.parse().map_err(|_| ())?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri
                    .authority()
                    .is_none_or(|a| a.as_str().contains(['*', '@']))
                || uri.path_and_query().is_some_and(|p| p.as_str() != "/")
                || origin.ends_with('/')
            {
                return Err(());
            }
        }
        config.allowed_origins = args.allowed_origin.clone();
    }
    config.allowed_hosts = vec![args.bind.to_string()];
    if args.bind.ip().is_loopback() {
        config
            .allowed_hosts
            .push(format!("localhost:{}", args.bind.port()));
    }
    config.node = match (&args.rpc_url, &args.rpc_cookie_file, &args.network) {
        (None, None, None) => None,
        (Some(url), Some(cookie), Some(network)) => Some(ConfiguredNode::new(
            network.parse().map_err(|_| ())?,
            txsignx_node::BitcoinCoreRpc::new(url, cookie).map_err(|_| ())?,
        )),
        _ => return Err(()),
    };
    Ok(config)
}
#[tokio::main]
async fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            let _ = error.print();
            return ExitCode::SUCCESS;
        }
        Err(_) => {
            eprintln!("error: invalid arguments; run txsignx-api --help");
            return ExitCode::FAILURE;
        }
    };
    let config = match configuration(&args) {
        Ok(config) => config,
        Err(()) => {
            eprintln!("error: invalid server configuration");
            return ExitCode::FAILURE;
        }
    };
    if !args.bind.ip().is_loopback() {
        eprintln!("warning: external binding exposes an unauthenticated inspection service");
    }
    let listener = match tokio::net::TcpListener::bind(args.bind).await {
        Ok(listener) => listener,
        Err(_) => {
            eprintln!("error: could not bind server");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("TxSignX API listening on {}", args.bind);
    match axum::serve(listener, app_with_config(config))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("error: server failure");
            ExitCode::FAILURE
        }
    }
}

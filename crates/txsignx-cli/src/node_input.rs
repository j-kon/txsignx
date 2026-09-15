use clap::Args;
use std::{error::Error, path::PathBuf};
use txsignx_node::BitcoinCoreRpc;
#[derive(Args)]
#[group(multiple = true)]
pub struct NodeArgs {
    /// Exact local HTTP endpoint: http://127.0.0.1:PORT or http://[::1]:PORT.
    #[arg(long)]
    rpc_url: Option<String>,
    /// Bitcoin Core cookie authentication file; contents are never reported.
    #[arg(long)]
    rpc_cookie_file: Option<PathBuf>,
}
impl NodeArgs {
    pub fn enabled(&self) -> bool {
        self.rpc_url.is_some() || self.rpc_cookie_file.is_some()
    }
    pub fn client(&self, network: Option<&str>) -> Result<Option<BitcoinCoreRpc>, Box<dyn Error>> {
        if !self.enabled() {
            return Ok(None);
        }
        let (Some(url), Some(path), Some(network)) =
            (&self.rpc_url, &self.rpc_cookie_file, network)
        else {
            return Err("node mode requires RPC URL, cookie file and explicit network".into());
        };
        let _: txsignx_wallet::ConfiguredNetwork = network.parse()?;
        Ok(Some(BitcoinCoreRpc::new(url, path)?))
    }
}

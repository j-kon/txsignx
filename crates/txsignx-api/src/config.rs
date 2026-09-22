use std::{sync::Arc, time::Duration};
use txsignx_wallet::ConfiguredNetwork;
type Observe = dyn Fn(
        &txsignx_core::PsbtReport,
    ) -> Result<txsignx_node::NodeContextReport, txsignx_node::NodeError>
    + Send
    + Sync;
type InspectTx = dyn Fn(
        &bitcoin::Txid,
    ) -> Result<
        (
            txsignx_node::NodeTransaction,
            Vec<Option<bitcoin::TxOut>>,
            txsignx_node::BlockchainInfo,
        ),
        txsignx_node::NodeError,
    > + Send
    + Sync;
#[derive(Clone)]
pub struct ConfiguredNode {
    pub network: ConfiguredNetwork,
    pub(crate) observe: Arc<Observe>,
    pub(crate) inspect_tx: Arc<InspectTx>,
}
impl ConfiguredNode {
    pub fn new<R: txsignx_node::NodeRpc + Send + Sync + 'static>(
        network: ConfiguredNetwork,
        rpc: R,
    ) -> Self {
        let rpc = Arc::new(rpc);
        let rpc_observe = Arc::clone(&rpc);
        let rpc_inspect = Arc::clone(&rpc);
        Self {
            network,
            observe: Arc::new(move |report| {
                txsignx_node::build_node_context(&*rpc_observe, report, network.bitcoin_network())
            }),
            inspect_tx: Arc::new(move |txid| {
                txsignx_node::fetch_transaction_with_context(
                    &*rpc_inspect,
                    txid,
                    network.bitcoin_network(),
                )
            }),
        }
    }

    pub(crate) fn inspect_transaction(
        &self,
        txid: &bitcoin::Txid,
    ) -> Result<
        (
            txsignx_node::NodeTransaction,
            Vec<Option<bitcoin::TxOut>>,
            txsignx_node::BlockchainInfo,
        ),
        txsignx_node::NodeError,
    > {
        (self.inspect_tx)(txid)
    }
}
#[derive(Clone)]
pub struct Config {
    pub node: Option<ConfiguredNode>,
    pub allowed_origins: Vec<String>,
    /// Exact authorities accepted in Host, including ports.
    pub allowed_hosts: Vec<String>,
    pub request_timeout: Duration,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            node: None,
            allowed_origins: vec![
                "http://localhost:5173".into(),
                "http://127.0.0.1:5173".into(),
            ],
            allowed_hosts: vec![
                "localhost:8080".into(),
                "127.0.0.1:8080".into(),
                "[::1]:8080".into(),
            ],
            request_timeout: Duration::from_secs(30),
        }
    }
}
pub const TEXT_BYTES: usize = 1024 * 1024;
pub const BODY_BYTES: usize = 2 * 1024 * 1024;
pub const RESPONSE_BYTES: usize = 8 * 1024 * 1024;

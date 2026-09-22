use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Transaction {
    pub raw_transaction: Option<String>,
    pub txid: Option<String>,
    pub network: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Psbt {
    pub psbt: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preflight {
    pub psbt: String,
    pub policy: Option<Policy>,
    pub wallet: Option<Wallet>,
    pub node: Option<Node>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    pub max_absolute_fee_sats: Option<u64>,
    pub max_fee_ratio_bps: Option<u16>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Node {
    pub use_configured_node: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Wallet {
    pub network: String,
    pub external_descriptor: String,
    pub internal_descriptor: String,
    pub derivation_window: Option<u32>,
    #[serde(default)]
    pub expected_change_outputs: Vec<usize>,
}

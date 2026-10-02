use crate::{
    NodeError, RpcEndpoint,
    live::{
        BlockDetails, BlockTransactionItem, BlockTransactionPage, MAX_RECENT_BLOCKS,
        MempoolSummary, RecentBlockSummary,
    },
};
use bitcoin::{
    BlockHash, Network, OutPoint, Transaction, TxOut, Txid,
    base64::{Engine, engine::general_purpose::STANDARD},
};
use bitcoincore_rpc::RpcApi;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MempoolEntry {
    pub vsize: u64,
    pub weight: u64,
    #[serde(default)]
    pub time: Option<u64>,
    #[serde(default)]
    pub wtxid: Option<String>,
    #[serde(default, rename = "bip125-replaceable")]
    pub bip125_replaceable: Option<bool>,
    #[serde(default)]
    pub fees: Option<MempoolEntryFees>,
    #[serde(default)]
    pub depends: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MempoolEntryFees {
    pub base: f64,
}

#[derive(Debug, Clone)]
pub struct BlockchainInfo {
    pub network: Network,
    pub blocks: u64,
    pub headers: u64,
    pub best_block_hash: BlockHash,
    pub initial_block_download: bool,
    pub verification_progress: Option<f64>,
}
#[derive(Debug, Clone)]
pub struct NodeTxOut {
    pub best_block: BlockHash,
    pub confirmations: u32,
    pub coinbase: bool,
    pub output: TxOut,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolAcceptance {
    pub txid: Txid,
    pub allowed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeTransaction {
    pub transaction: Transaction,
    pub block_hash: Option<BlockHash>,
    pub confirmations: Option<u32>,
}
/// Callers supply a trusted node implementation. Policy never holds or calls this trait.
pub trait NodeRpc {
    fn blockchain_info(&self) -> Result<BlockchainInfo, NodeError>;
    fn best_block_hash(&self) -> Result<BlockHash, NodeError>;
    fn block_count(&self) -> Result<u64, NodeError>;
    fn get_tx_out(
        &self,
        outpoint: &OutPoint,
        include_mempool: bool,
    ) -> Result<Option<NodeTxOut>, NodeError>;
    fn test_mempool_accept(
        &self,
        transaction: &Transaction,
    ) -> Result<MempoolAcceptance, NodeError>;
    fn send_raw_transaction(&self, transaction: &Transaction) -> Result<Txid, NodeError>;
    fn get_raw_transaction(&self, txid: &Txid) -> Result<NodeTransaction, NodeError>;
    fn mempool_summary(&self) -> Result<MempoolSummary, NodeError> {
        Err(NodeError::Rpc)
    }
    fn raw_mempool_verbose(&self) -> Result<HashMap<Txid, MempoolEntry>, NodeError> {
        Err(NodeError::Rpc)
    }
    fn raw_mempool_txids(&self) -> Result<Vec<Txid>, NodeError> {
        Err(NodeError::Rpc)
    }
    fn block_hash_by_height(&self, _height: u64) -> Result<BlockHash, NodeError> {
        Err(NodeError::Rpc)
    }
    fn get_block_summary(&self, _hash: &BlockHash) -> Result<RecentBlockSummary, NodeError> {
        Err(NodeError::Rpc)
    }
    fn recent_blocks(&self, _count: usize) -> Result<Vec<RecentBlockSummary>, NodeError> {
        Err(NodeError::Rpc)
    }
    fn get_block_txids(&self, _hash: &BlockHash) -> Result<Vec<Txid>, NodeError> {
        Err(NodeError::Rpc)
    }
    fn get_block_details(
        &self,
        _hash: &BlockHash,
        _offset: usize,
        _limit: usize,
    ) -> Result<BlockDetails, NodeError> {
        Err(NodeError::Rpc)
    }
}
pub fn node_network(chain: &str) -> Result<Network, NodeError> {
    match chain {
        "main" => Ok(Network::Bitcoin),
        "test" => Ok(Network::Testnet),
        "testnet4" => Ok(Network::Testnet4),
        "signet" => Ok(Network::Signet),
        "regtest" => Ok(Network::Regtest),
        _ => Err(NodeError::UnsupportedNetwork),
    }
}
/// No Debug or Serialize: endpoint and authentication are deliberately private.
pub struct BitcoinCoreRpc {
    wire: Wire,
}
struct Wire {
    endpoint: RpcEndpoint,
    auth: String,
}
impl BitcoinCoreRpc {
    pub fn new(url: &str, cookie_file: &Path) -> Result<Self, NodeError> {
        let endpoint = RpcEndpoint::parse(url)?;
        let cookie = crate::config::cookie(cookie_file)?;
        Ok(Self {
            wire: Wire {
                endpoint,
                auth: format!("Basic {}", STANDARD.encode(cookie.as_bytes())),
            },
        })
    }
}
// Use the reviewed bitcoincore-rpc typed API with a bounded transport. Its default
// Client logs raw responses and permits redirects; this private adapter does neither.
impl RpcApi for Wire {
    fn call<T: for<'a> Deserialize<'a>>(
        &self,
        cmd: &str,
        args: &[serde_json::Value],
    ) -> bitcoincore_rpc::Result<T> {
        self.request(cmd, args)
            .map_err(|_| bitcoincore_rpc::Error::UnexpectedStructure)
    }
}
impl Wire {
    fn request<T: for<'a> Deserialize<'a>>(
        &self,
        cmd: &str,
        args: &[serde_json::Value],
    ) -> Result<T, NodeError> {
        let started = Instant::now();
        let body = serde_json::to_vec(
            &serde_json::json!({"jsonrpc":"2.0","id":1,"method":cmd,"params":args}),
        )
        .map_err(|_| NodeError::Rpc)?;
        let response = minreq::post(self.endpoint.url())
            .with_follow_redirects(false)
            .with_max_redirects(0)
            .with_timeout(5)
            .with_max_headers_size(8192)
            .with_max_status_line_length(1024)
            .with_header("Content-Type", "application/json")
            .with_header("Authorization", &self.auth)
            .with_body(body)
            .send_lazy()
            .map_err(|_| NodeError::Rpc)?;
        if response.status_code != 200 && response.status_code != 500 {
            return Err(NodeError::Rpc);
        }
        let mut bytes = Vec::new();
        for item in response.take(1_048_577) {
            if started.elapsed() > Duration::from_secs(5) {
                return Err(NodeError::Rpc);
            }
            bytes.push(item.map_err(|_| NodeError::Rpc)?.0);
        }
        if bytes.len() > 1_048_576 {
            return Err(NodeError::Rpc);
        }
        decode_response(&bytes)
    }
}
fn decode_response<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, NodeError> {
    let response: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| NodeError::Rpc)?;
    if response.get("id") != Some(&serde_json::json!(1))
        || response.get("jsonrpc") != Some(&serde_json::json!("2.0"))
    {
        return Err(NodeError::Rpc);
    }
    if let Some(err) = response.get("error").filter(|e| !e.is_null()) {
        if let Some(code) = err.get("code").and_then(|c| c.as_i64())
            && code == -5
        {
            return Err(NodeError::TransactionNotFound);
        }
        return Err(NodeError::Rpc);
    }
    let result = response.get("result").ok_or(NodeError::Rpc)?;
    serde_json::from_value(result.clone()).map_err(|_| NodeError::Rpc)
}
#[derive(Deserialize)]
struct ChainResponse {
    chain: String,
    blocks: u64,
    headers: u64,
    bestblockhash: BlockHash,
    initialblockdownload: bool,
    verificationprogress: Option<f64>,
}
impl NodeRpc for BitcoinCoreRpc {
    fn blockchain_info(&self) -> Result<BlockchainInfo, NodeError> {
        // Minimal DTO avoids assumptions about version-dependent softfork fields.
        let r: ChainResponse = self
            .wire
            .call("getblockchaininfo", &[])
            .map_err(|_| NodeError::Rpc)?;
        Ok(BlockchainInfo {
            network: node_network(&r.chain)?,
            blocks: r.blocks,
            headers: r.headers,
            best_block_hash: r.bestblockhash,
            initial_block_download: r.initialblockdownload,
            verification_progress: r.verificationprogress,
        })
    }
    fn best_block_hash(&self) -> Result<BlockHash, NodeError> {
        self.wire.get_best_block_hash().map_err(|_| NodeError::Rpc)
    }
    fn block_count(&self) -> Result<u64, NodeError> {
        self.wire.get_block_count().map_err(|_| NodeError::Rpc)
    }
    fn get_tx_out(&self, p: &OutPoint, m: bool) -> Result<Option<NodeTxOut>, NodeError> {
        self.wire
            .get_tx_out(&p.txid, p.vout, Some(m))
            .map_err(|_| NodeError::Rpc)?
            .map(|r| {
                let script = r.script_pub_key.script().map_err(|_| NodeError::Rpc)?;
                if script.len() > 10000 || r.value > bitcoin::Amount::MAX_MONEY {
                    return Err(NodeError::Rpc);
                }
                Ok(NodeTxOut {
                    best_block: r.bestblock,
                    confirmations: r.confirmations,
                    coinbase: r.coinbase,
                    output: TxOut {
                        value: r.value,
                        script_pubkey: script,
                    },
                })
            })
            .transpose()
    }
    fn test_mempool_accept(&self, tx: &Transaction) -> Result<MempoolAcceptance, NodeError> {
        let r = self
            .wire
            .test_mempool_accept(&[tx])
            .map_err(|_| NodeError::Rpc)?;
        acceptance(tx, r)
    }
    fn send_raw_transaction(&self, tx: &Transaction) -> Result<Txid, NodeError> {
        let id = self
            .wire
            .send_raw_transaction(tx)
            .map_err(|_| NodeError::Rpc)?;
        if id != tx.compute_txid() {
            return Err(NodeError::Rpc);
        }
        Ok(id)
    }
    fn get_raw_transaction(&self, txid: &Txid) -> Result<NodeTransaction, NodeError> {
        #[derive(Deserialize)]
        struct RawTxVerbose {
            hex: String,
            #[serde(default)]
            confirmations: Option<u32>,
            #[serde(default)]
            blockhash: Option<BlockHash>,
        }
        let r: RawTxVerbose = self.wire.request(
            "getrawtransaction",
            &[serde_json::json!(txid.to_string()), serde_json::json!(true)],
        )?;
        let transaction = txsignx_core::decode_transaction(&r.hex).map_err(|_| NodeError::Rpc)?;
        if transaction.compute_txid() != *txid {
            return Err(NodeError::Rpc);
        }
        Ok(NodeTransaction {
            transaction,
            block_hash: r.blockhash,
            confirmations: r.confirmations,
        })
    }
    fn mempool_summary(&self) -> Result<MempoolSummary, NodeError> {
        #[derive(Deserialize)]
        struct RawMempoolInfo {
            size: usize,
            #[serde(default)]
            bytes: Option<u64>,
            #[serde(default)]
            usage: Option<u64>,
            #[serde(default)]
            total_fee: Option<f64>,
        }
        let r: RawMempoolInfo = self.wire.request("getmempoolinfo", &[])?;
        Ok(MempoolSummary {
            tx_count: r.size,
            size_bytes: r.bytes,
            usage_bytes: r.usage,
            total_fee_sats: r.total_fee.map(|f| (f * 100_000_000.0).round() as u64),
        })
    }
    fn raw_mempool_verbose(&self) -> Result<HashMap<Txid, MempoolEntry>, NodeError> {
        let raw: HashMap<String, MempoolEntry> = self
            .wire
            .request("getrawmempool", &[serde_json::json!(true)])?;
        let mut map = HashMap::with_capacity(raw.len());
        for (k, v) in raw {
            if let Ok(txid) = k.parse::<Txid>() {
                map.insert(txid, v);
            }
        }
        Ok(map)
    }
    fn raw_mempool_txids(&self) -> Result<Vec<Txid>, NodeError> {
        let raw: Vec<String> = self
            .wire
            .request("getrawmempool", &[serde_json::json!(false)])?;
        let txids = raw
            .into_iter()
            .filter_map(|s| s.parse::<Txid>().ok())
            .collect();
        Ok(txids)
    }
    fn block_hash_by_height(&self, height: u64) -> Result<BlockHash, NodeError> {
        let hash_str: String = self
            .wire
            .request("getblockhash", &[serde_json::json!(height)])?;
        hash_str.parse().map_err(|_| NodeError::Rpc)
    }
    fn get_block_summary(&self, hash: &BlockHash) -> Result<RecentBlockSummary, NodeError> {
        #[derive(Deserialize)]
        struct RawBlock {
            hash: String,
            height: u64,
            #[serde(default, rename = "nTx")]
            n_tx: Option<usize>,
            #[serde(default)]
            size: Option<u64>,
            #[serde(default)]
            weight: Option<u64>,
            #[serde(default)]
            time: Option<u64>,
            #[serde(default)]
            tx: Option<Vec<String>>,
        }
        let r: RawBlock = self.wire.request(
            "getblock",
            &[serde_json::json!(hash.to_string()), serde_json::json!(1)],
        )?;
        let tx_count = r
            .n_tx
            .or_else(|| r.tx.as_ref().map(|t| t.len()))
            .unwrap_or(0);
        Ok(RecentBlockSummary {
            height: r.height,
            hash: r.hash,
            tx_count,
            weight: r.weight,
            size: r.size,
            timestamp: r.time,
        })
    }
    fn recent_blocks(&self, count: usize) -> Result<Vec<RecentBlockSummary>, NodeError> {
        let bounded_count = count.clamp(1, MAX_RECENT_BLOCKS);
        let info = self.blockchain_info()?;
        let tip_height = info.blocks;
        let mut blocks = Vec::with_capacity(bounded_count);
        for i in 0..bounded_count {
            if tip_height < i as u64 {
                break;
            }
            let h = tip_height - i as u64;
            let hash = self.block_hash_by_height(h)?;
            let summary = self.get_block_summary(&hash)?;
            blocks.push(summary);
        }
        Ok(blocks)
    }
    fn get_block_txids(&self, hash: &BlockHash) -> Result<Vec<Txid>, NodeError> {
        #[derive(Deserialize)]
        struct RawBlockTx {
            #[serde(default)]
            tx: Option<Vec<String>>,
        }
        let r: RawBlockTx = self.wire.request(
            "getblock",
            &[serde_json::json!(hash.to_string()), serde_json::json!(1)],
        )?;
        let mut txids = Vec::new();
        if let Some(list) = r.tx {
            for s in list {
                if let Ok(txid) = s.parse::<Txid>() {
                    txids.push(txid);
                }
            }
        }
        Ok(txids)
    }
    fn get_block_details(
        &self,
        hash: &BlockHash,
        offset: usize,
        limit: usize,
    ) -> Result<BlockDetails, NodeError> {
        #[derive(Deserialize)]
        struct RawBlockFull {
            hash: String,
            height: u64,
            #[serde(default)]
            previousblockhash: Option<String>,
            #[serde(default)]
            nextblockhash: Option<String>,
            #[serde(default)]
            merkleroot: Option<String>,
            #[serde(default)]
            version: Option<i32>,
            time: u64,
            #[serde(default)]
            mediantime: Option<u64>,
            #[serde(default)]
            bits: Option<String>,
            #[serde(default)]
            difficulty: Option<f64>,
            #[serde(default, rename = "nTx")]
            n_tx: Option<usize>,
            #[serde(default)]
            weight: Option<u64>,
            #[serde(default)]
            size: Option<u64>,
            #[serde(default)]
            tx: Option<Vec<String>>,
        }
        let r: RawBlockFull = self
            .wire
            .request(
                "getblock",
                &[serde_json::json!(hash.to_string()), serde_json::json!(1)],
            )
            .map_err(|e| match e {
                NodeError::TransactionNotFound => NodeError::BlockNotFound,
                other => other,
            })?;
        let network = self.blockchain_info()?.network.to_string();
        let all_txs = r.tx.unwrap_or_default();
        let total = r.n_tx.unwrap_or(all_txs.len());
        let bounded_limit = limit.clamp(1, 100);
        let items: Vec<BlockTransactionItem> = all_txs
            .into_iter()
            .enumerate()
            .skip(offset)
            .take(bounded_limit)
            .map(|(idx, txid)| BlockTransactionItem {
                index: idx,
                txid,
                is_coinbase: idx == 0,
            })
            .collect();
        let has_more = offset + items.len() < total;
        let page = BlockTransactionPage {
            items,
            offset,
            limit: bounded_limit,
            total,
            has_more,
        };
        Ok(BlockDetails {
            network,
            height: r.height,
            hash: r.hash,
            previous_block_hash: r.previousblockhash,
            next_block_hash: r.nextblockhash,
            merkle_root: r.merkleroot,
            version: r.version,
            timestamp: r.time,
            median_time: r.mediantime,
            bits: r.bits,
            difficulty: r.difficulty,
            tx_count: total,
            weight: r.weight,
            size: r.size,
            transactions: page,
        })
    }
}
fn acceptance(
    tx: &Transaction,
    r: Vec<bitcoincore_rpc::json::TestMempoolAcceptResult>,
) -> Result<MempoolAcceptance, NodeError> {
    if r.len() != 1 {
        return Err(NodeError::Rpc);
    }
    let item = r.first().ok_or(NodeError::Rpc)?;
    if item.txid != tx.compute_txid() {
        return Err(NodeError::Rpc);
    }
    Ok(MempoolAcceptance {
        txid: item.txid,
        allowed: item.allowed,
    })
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeChainTip {
    pub height: u64,
    pub hash: BlockHash,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_response_envelopes_fail_without_echo() {
        for bytes in [
            b"not json".as_slice(),
            br#"{"jsonrpc":"2.0","id":2,"result":1}"#,
            br#"{"jsonrpc":"2.0","id":1,"error":{"message":"SECRET_MARKER"}}"#,
            br#"{"jsonrpc":"2.0","id":1}"#,
        ] {
            assert_eq!(decode_response::<u64>(bytes), Err(NodeError::Rpc));
        }
        assert_eq!(
            decode_response::<u64>(br#"{"jsonrpc":"2.0","id":1,"result":4}"#),
            Ok(4)
        );
    }
    #[test]
    fn typed_response_rejects_negative_or_excess_confirmations() {
        for count in [serde_json::json!(-1), serde_json::json!(4294967296u64)] {
            let v = serde_json::json!({"bestblock":"00".repeat(32),"confirmations":count,"value":1.0,"scriptPubKey":{"asm":"","hex":"","type":"nonstandard"},"coinbase":false});
            assert!(serde_json::from_value::<bitcoincore_rpc::json::GetTxOutResult>(v).is_err());
        }
    }
    #[test]
    fn acceptance_requires_exactly_one_matching_candidate() {
        let tx = txsignx_core::psbt::decode_psbt(include_str!("../../../fixtures/policy/pass.b64"))
            .unwrap()
            .unsigned_tx;
        for json in [
            serde_json::json!([]),
            serde_json::json!([{"txid":"00".repeat(32),"allowed":true}]),
            serde_json::json!([{"txid":tx.compute_txid(),"allowed":true},{"txid":tx.compute_txid(),"allowed":true}]),
        ] {
            let items = serde_json::from_value(json).unwrap();
            assert_eq!(acceptance(&tx, items), Err(NodeError::Rpc));
        }
    }
}

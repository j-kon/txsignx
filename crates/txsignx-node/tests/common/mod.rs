#![allow(dead_code)]
use bitcoin::{BlockHash, OutPoint, Transaction, Txid, hashes::Hash};
use std::cell::{Cell, RefCell};
use txsignx_node::*;
pub fn inspection() -> txsignx_core::PsbtReport {
    txsignx_core::analyze_psbt(include_str!("../../../../fixtures/policy/pass.b64")).unwrap()
}
pub fn hash(n: u8) -> BlockHash {
    BlockHash::from_byte_array([n; 32])
}
pub struct Fake {
    pub info: BlockchainInfo,
    pub chain: Option<NodeTxOut>,
    pub mempool: Option<NodeTxOut>,
    pub changes: usize,
    pub attempts: Cell<usize>,
    pub calls: RefCell<Vec<bool>>,
    pub unavailable: bool,
}
impl Fake {
    pub fn new() -> Self {
        let r = inspection();
        let i = &r.inputs[0];
        let o = NodeTxOut {
            best_block: hash(1),
            confirmations: 101,
            coinbase: false,
            output: bitcoin::TxOut {
                value: bitcoin::Amount::from_sat(i.utxo.value_sats.unwrap()),
                script_pubkey: bitcoin::ScriptBuf::from_hex(
                    i.utxo.script_pubkey_hex.as_ref().unwrap(),
                )
                .unwrap(),
            },
        };
        Self {
            info: BlockchainInfo {
                network: bitcoin::Network::Regtest,
                blocks: 200,
                headers: 200,
                best_block_hash: hash(1),
                initial_block_download: false,
                verification_progress: Some(1.0),
            },
            chain: Some(o.clone()),
            mempool: Some(o),
            changes: 0,
            attempts: Cell::new(0),
            calls: RefCell::new(vec![]),
            unavailable: false,
        }
    }
}
impl NodeRpc for Fake {
    fn blockchain_info(&self) -> Result<BlockchainInfo, NodeError> {
        self.attempts.set(self.attempts.get() + 1);
        if self.unavailable {
            Err(NodeError::Rpc)
        } else {
            Ok(self.info.clone())
        }
    }
    fn best_block_hash(&self) -> Result<BlockHash, NodeError> {
        Ok(hash(if self.attempts.get() <= self.changes {
            2
        } else {
            1
        }))
    }
    fn block_count(&self) -> Result<u64, NodeError> {
        Ok(self.info.blocks)
    }
    fn get_tx_out(&self, _: &OutPoint, m: bool) -> Result<Option<NodeTxOut>, NodeError> {
        self.calls.borrow_mut().push(m);
        Ok(if m {
            self.mempool.clone()
        } else {
            self.chain.clone()
        })
    }
    fn test_mempool_accept(&self, _: &Transaction) -> Result<MempoolAcceptance, NodeError> {
        panic!("read-only context must not test acceptance")
    }
    fn send_raw_transaction(&self, _: &Transaction) -> Result<Txid, NodeError> {
        panic!("read-only context must not send")
    }
}

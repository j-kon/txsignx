use bitcoin::{
    Amount, BlockHash, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid,
    hashes::Hash,
};
use std::collections::HashMap;
use txsignx_node::{
    BlockchainInfo, MempoolAcceptance, NodeError, NodeRpc, NodeTransaction, NodeTxOut,
    fetch_transaction_with_context, resolve_transaction_prevouts,
};

struct MockExplorerRpc {
    info: BlockchainInfo,
    transactions: HashMap<Txid, NodeTransaction>,
}

impl MockExplorerRpc {
    fn new(network: Network) -> Self {
        Self {
            info: BlockchainInfo {
                network,
                blocks: 100,
                headers: 100,
                best_block_hash: BlockHash::from_byte_array([1; 32]),
                initial_block_download: false,
                verification_progress: Some(1.0),
            },
            transactions: HashMap::new(),
        }
    }
}

impl NodeRpc for MockExplorerRpc {
    fn blockchain_info(&self) -> Result<BlockchainInfo, NodeError> {
        Ok(self.info.clone())
    }
    fn best_block_hash(&self) -> Result<BlockHash, NodeError> {
        Ok(self.info.best_block_hash)
    }
    fn block_count(&self) -> Result<u64, NodeError> {
        Ok(self.info.blocks)
    }
    fn get_tx_out(&self, _: &OutPoint, _: bool) -> Result<Option<NodeTxOut>, NodeError> {
        Ok(None)
    }
    fn test_mempool_accept(&self, _: &Transaction) -> Result<MempoolAcceptance, NodeError> {
        panic!("unused")
    }
    fn send_raw_transaction(&self, _: &Transaction) -> Result<Txid, NodeError> {
        panic!("unused")
    }
    fn get_raw_transaction(&self, txid: &Txid) -> Result<NodeTransaction, NodeError> {
        self.transactions
            .get(txid)
            .cloned()
            .ok_or(NodeError::TransactionNotFound)
    }
}

fn dummy_tx(prev_txid: Txid, prev_vout: u32) -> Transaction {
    Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: prev_txid,
                vout: prev_vout,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: bitcoin::Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(50_000),
            script_pubkey: ScriptBuf::new(),
        }],
    }
}

#[test]
fn fetch_confirmed_transaction_with_context_succeeds() {
    let mut mock = MockExplorerRpc::new(Network::Regtest);

    let prev_tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![TxOut {
            value: Amount::from_sat(100_000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    let prev_txid = prev_tx.compute_txid();
    mock.transactions.insert(
        prev_txid,
        NodeTransaction {
            transaction: prev_tx,
            block_hash: Some(BlockHash::from_byte_array([2; 32])),
            confirmations: Some(10),
        },
    );

    let tx = dummy_tx(prev_txid, 0);
    let txid = tx.compute_txid();
    let block_hash = BlockHash::from_byte_array([3; 32]);
    mock.transactions.insert(
        txid,
        NodeTransaction {
            transaction: tx,
            block_hash: Some(block_hash),
            confirmations: Some(5),
        },
    );

    let (node_tx, prevouts, info) =
        fetch_transaction_with_context(&mock, &txid, Network::Regtest).unwrap();

    assert_eq!(node_tx.confirmations, Some(5));
    assert_eq!(node_tx.block_hash, Some(block_hash));
    assert_eq!(info.network, Network::Regtest);
    assert_eq!(prevouts.len(), 1);
    assert_eq!(prevouts[0].as_ref().unwrap().value.to_sat(), 100_000);
}

#[test]
fn fetch_unconfirmed_mempool_transaction_reports_none_block_hash() {
    let mut mock = MockExplorerRpc::new(Network::Regtest);

    let prev_tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![TxOut {
            value: Amount::from_sat(80_000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    let prev_txid = prev_tx.compute_txid();
    mock.transactions.insert(
        prev_txid,
        NodeTransaction {
            transaction: prev_tx,
            block_hash: Some(BlockHash::from_byte_array([2; 32])),
            confirmations: Some(20),
        },
    );

    let tx = dummy_tx(prev_txid, 0);
    let txid = tx.compute_txid();
    mock.transactions.insert(
        txid,
        NodeTransaction {
            transaction: tx,
            block_hash: None,
            confirmations: None,
        },
    );

    let (node_tx, prevouts, _) =
        fetch_transaction_with_context(&mock, &txid, Network::Regtest).unwrap();

    assert_eq!(node_tx.confirmations, None);
    assert_eq!(node_tx.block_hash, None);
    assert_eq!(prevouts[0].as_ref().unwrap().value.to_sat(), 80_000);
}

#[test]
fn fetch_missing_transaction_returns_not_found() {
    let mock = MockExplorerRpc::new(Network::Regtest);
    let non_existent_txid = Txid::from_byte_array([99; 32]);

    let err =
        fetch_transaction_with_context(&mock, &non_existent_txid, Network::Regtest).unwrap_err();
    assert_eq!(err, NodeError::TransactionNotFound);
}

#[test]
fn network_mismatch_fails_closed() {
    let mock = MockExplorerRpc::new(Network::Regtest);
    let txid = Txid::from_byte_array([1; 32]);

    // Expected mainnet (Bitcoin), but node is Regtest
    let err = fetch_transaction_with_context(&mock, &txid, Network::Bitcoin).unwrap_err();
    assert_eq!(err, NodeError::UnsupportedNetwork);
}

#[test]
fn prevout_resolution_handles_missing_previous_transaction() {
    let mock = MockExplorerRpc::new(Network::Regtest);
    // previous tx is not in mock
    let missing_prev_txid = Txid::from_byte_array([88; 32]);
    let tx = dummy_tx(missing_prev_txid, 0);

    let prevouts = resolve_transaction_prevouts(&mock, &tx).unwrap();
    assert_eq!(prevouts.len(), 1);
    assert!(prevouts[0].is_none());
}

#[test]
fn prevout_resolution_fails_on_vout_out_of_bounds() {
    let mut mock = MockExplorerRpc::new(Network::Regtest);
    let prev_tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
        input: vec![],
        output: vec![TxOut {
            value: Amount::from_sat(50_000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    let prev_txid = prev_tx.compute_txid();
    mock.transactions.insert(
        prev_txid,
        NodeTransaction {
            transaction: prev_tx,
            block_hash: None,
            confirmations: None,
        },
    );

    // prev_tx only has 1 output (vout 0), but tx requests vout 5
    let bad_tx = dummy_tx(prev_txid, 5);
    let err = resolve_transaction_prevouts(&mock, &bad_tx).unwrap_err();
    assert_eq!(err, NodeError::InconsistentObservation);
}

#[test]
fn prevout_resolution_handles_coinbase() {
    let mock = MockExplorerRpc::new(Network::Regtest);
    let genesis = bitcoin::blockdata::constants::genesis_block(Network::Bitcoin);
    let coinbase = &genesis.txdata[0];

    let prevouts = resolve_transaction_prevouts(&mock, coinbase).unwrap();
    assert_eq!(prevouts.len(), coinbase.input.len());
    assert!(prevouts.iter().all(|p| p.is_none()));
}

#[test]
fn prevout_resolution_rejects_exorbitant_input_count() {
    let mock = MockExplorerRpc::new(Network::Regtest);
    let mut tx = dummy_tx(Txid::from_byte_array([1; 32]), 0);
    // Add 257 inputs (limit is 256)
    while tx.input.len() <= 256 {
        tx.input.push(tx.input[0].clone());
    }

    let err = resolve_transaction_prevouts(&mock, &tx).unwrap_err();
    assert_eq!(err, NodeError::InvalidInspection);
}

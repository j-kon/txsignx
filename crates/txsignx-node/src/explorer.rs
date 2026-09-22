use crate::{BlockchainInfo, NodeError, NodeRpc, NodeTransaction, context::MAX_NODE_INPUTS};
use bitcoin::{Amount, Network, Transaction, TxOut, Txid};
use std::collections::HashMap;

/// Resolve previous outputs for all inputs of a transaction using the provided NodeRpc.
/// Bounds input count to MAX_NODE_INPUTS (256) and caches previous transactions in memory
/// to avoid duplicate RPC lookups.
pub fn resolve_transaction_prevouts(
    rpc: &impl NodeRpc,
    transaction: &Transaction,
) -> Result<Vec<Option<TxOut>>, NodeError> {
    if transaction.input.is_empty() || transaction.input.len() > MAX_NODE_INPUTS {
        return Err(NodeError::InvalidInspection);
    }

    if transaction.is_coinbase() {
        return Ok(vec![None; transaction.input.len()]);
    }

    let mut cache: HashMap<Txid, Transaction> = HashMap::new();
    let mut prevouts = Vec::with_capacity(transaction.input.len());

    for input in &transaction.input {
        if input.previous_output.is_null() {
            prevouts.push(None);
            continue;
        }

        let prev_txid = input.previous_output.txid;
        let vout = input.previous_output.vout as usize;

        let prev_tx = match cache.get(&prev_txid) {
            Some(tx) => tx,
            None => match rpc.get_raw_transaction(&prev_txid) {
                Ok(node_tx) => {
                    cache.insert(prev_txid, node_tx.transaction);
                    &cache[&prev_txid]
                }
                Err(NodeError::TransactionNotFound) => {
                    // Previous transaction not available; return None for this prevout
                    prevouts.push(None);
                    continue;
                }
                Err(err) => return Err(err),
            },
        };

        if vout >= prev_tx.output.len() {
            return Err(NodeError::InconsistentObservation);
        }

        let output = prev_tx.output[vout].clone();
        if output.value > Amount::MAX_MONEY || output.script_pubkey.len() > 10000 {
            return Err(NodeError::InconsistentObservation);
        }

        prevouts.push(Some(output));
    }

    Ok(prevouts)
}

/// Fetch a transaction by txid from Bitcoin Core, verify explicit network matching and readiness,
/// and resolve previous outputs for fee calculation.
pub fn fetch_transaction_with_context(
    rpc: &impl NodeRpc,
    txid: &Txid,
    expected_network: Network,
) -> Result<(NodeTransaction, Vec<Option<TxOut>>, BlockchainInfo), NodeError> {
    let info = rpc.blockchain_info()?;
    if info.network != expected_network {
        return Err(NodeError::UnsupportedNetwork);
    }
    if info.initial_block_download || info.headers != info.blocks || info.blocks == u64::MAX {
        return Err(NodeError::NodeNotReady);
    }

    let node_tx = rpc.get_raw_transaction(txid)?;
    let prevouts = resolve_transaction_prevouts(rpc, &node_tx.transaction)?;
    Ok((node_tx, prevouts, info))
}

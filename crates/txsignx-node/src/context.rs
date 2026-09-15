use crate::*;
use bitcoin::{
    Network, OutPoint, ScriptBuf,
    hashes::{Hash, sha256},
};
use serde::Serialize;
use txsignx_core::{PsbtReport, psbt::PsbtUtxoStatus};
/// Point queries are deliberately tighter than core's offline parsing ceiling.
pub const MAX_NODE_INPUTS: usize = 256;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeUtxoAvailability {
    ConfirmedUnspent,
    MempoolUnconfirmed,
    SpentInMempool,
    NotAvailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrevoutVerification {
    Match,
    ValueMismatch,
    ScriptMismatch,
    ValueAndScriptMismatch,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeInputContext {
    pub index: usize,
    pub outpoint: OutPoint,
    pub availability: NodeUtxoAvailability,
    pub confirmations: Option<u32>,
    pub coinbase: Option<bool>,
    pub prevout_verification: PrevoutVerification,
}
/// Only the builder constructs this report; immutable getters and no Deserialize.
#[derive(Debug, Clone, Serialize)]
pub struct NodeContextReport {
    configured_network: Network,
    node_network: Network,
    tip: NodeChainTip,
    inputs: Vec<NodeInputContext>,
    #[serde(skip)]
    binding: sha256::Hash,
}
impl NodeContextReport {
    pub fn configured_network(&self) -> Network {
        self.configured_network
    }
    pub fn node_network(&self) -> Network {
        self.node_network
    }
    pub fn tip(&self) -> &NodeChainTip {
        &self.tip
    }
    pub fn inputs(&self) -> &[NodeInputContext] {
        &self.inputs
    }
    pub fn validate_for(&self, r: &PsbtReport) -> Result<(), NodeError> {
        if binding(r)? == self.binding {
            Ok(())
        } else {
            Err(NodeError::ContextMismatch)
        }
    }
}
fn binding(r: &PsbtReport) -> Result<sha256::Hash, NodeError> {
    if r.inputs.is_empty()
        || r.inputs.len() > MAX_NODE_INPUTS
        || r.input_count != r.inputs.len()
        || r.output_count != r.outputs.len()
        || r.inputs.iter().enumerate().any(|(n, i)| n != i.index)
        || r.outputs
            .iter()
            .enumerate()
            .any(|(n, o)| n != o.transaction_output.index)
    {
        return Err(NodeError::InvalidInspection);
    }
    // Includes ordered outpoints/outputs, sequence, every resolved prevout and policy fact.
    let mut engine = sha256::Hash::engine();
    serde_json::to_writer(&mut engine, r).map_err(|_| NodeError::InvalidInspection)?;
    Ok(sha256::Hash::from_engine(engine))
}
pub fn build_node_context(
    rpc: &impl NodeRpc,
    r: &PsbtReport,
    network: Network,
) -> Result<NodeContextReport, NodeError> {
    let binding = binding(r)?;
    for _ in 0..3 {
        let info = rpc.blockchain_info()?;
        if info.initial_block_download
            || info.headers != info.blocks
            || info.blocks == u64::MAX
            || info
                .verification_progress
                .is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
        {
            return Err(NodeError::NodeNotReady);
        }
        let tip = NodeChainTip {
            height: info.blocks,
            hash: info.best_block_hash,
        };
        let inputs = observe(rpc, r, &tip);
        let after_hash = rpc.best_block_hash()?;
        let after_height = rpc.block_count()?;
        let final_hash = rpc.best_block_hash()?;
        if after_hash != tip.hash || after_height != tip.height || final_hash != tip.hash {
            continue;
        }
        let inputs = match inputs {
            Err(NodeError::UnstableChainTip) => continue,
            other => other?,
        };
        return Ok(NodeContextReport {
            configured_network: network,
            node_network: info.network,
            tip,
            inputs,
            binding,
        });
    }
    Err(NodeError::UnstableChainTip)
}
fn observe(
    rpc: &impl NodeRpc,
    r: &PsbtReport,
    tip: &NodeChainTip,
) -> Result<Vec<NodeInputContext>, NodeError> {
    let mut inputs = Vec::with_capacity(r.input_count);
    let mut points = std::collections::BTreeSet::new();
    for i in &r.inputs {
        let point = OutPoint {
            txid: i
                .previous_txid
                .parse()
                .map_err(|_| NodeError::InvalidInspection)?,
            vout: i.previous_vout,
        };
        if !points.insert(point) {
            return Err(NodeError::InvalidInspection);
        }
        let chain = rpc.get_tx_out(&point, false)?;
        let mempool = rpc.get_tx_out(&point, true)?;
        for o in [&chain, &mempool].into_iter().flatten() {
            if o.best_block != tip.hash {
                return Err(NodeError::UnstableChainTip);
            }
            if u64::from(o.confirmations) > tip.height + 1
                || o.output.value > bitcoin::Amount::MAX_MONEY
                || o.output.script_pubkey.len() > 10000
            {
                return Err(NodeError::InconsistentObservation);
            }
        }
        if let Some(c) = &chain {
            if c.confirmations == 0 {
                return Err(NodeError::InconsistentObservation);
            }
        }
        if let (Some(c), Some(m)) = (&chain, &mempool) {
            if c.output != m.output
                || c.confirmations != m.confirmations
                || c.coinbase != m.coinbase
            {
                return Err(NodeError::InconsistentObservation);
            }
        }
        if chain.is_none()
            && mempool
                .as_ref()
                .is_some_and(|m| m.confirmations != 0 || m.coinbase)
        {
            return Err(NodeError::InconsistentObservation);
        }
        let availability = match (&chain, &mempool) {
            (Some(_), Some(_)) => NodeUtxoAvailability::ConfirmedUnspent,
            (None, Some(_)) => NodeUtxoAvailability::MempoolUnconfirmed,
            (Some(_), None) => NodeUtxoAvailability::SpentInMempool,
            (None, None) => NodeUtxoAvailability::NotAvailable,
        };
        let observed = chain.as_ref().or(mempool.as_ref());
        let verification =
            if let Some(o) = observed.filter(|_| i.utxo.status == PsbtUtxoStatus::Valid) {
                let amount = i.utxo.value_sats.ok_or(NodeError::InvalidInspection)?;
                let script = ScriptBuf::from_hex(
                    i.utxo
                        .script_pubkey_hex
                        .as_deref()
                        .ok_or(NodeError::InvalidInspection)?,
                )
                .map_err(|_| NodeError::InvalidInspection)?;
                match (
                    amount == o.output.value.to_sat(),
                    script == o.output.script_pubkey,
                ) {
                    (true, true) => PrevoutVerification::Match,
                    (false, true) => PrevoutVerification::ValueMismatch,
                    (true, false) => PrevoutVerification::ScriptMismatch,
                    (false, false) => PrevoutVerification::ValueAndScriptMismatch,
                }
            } else {
                PrevoutVerification::Unavailable
            };
        inputs.push(NodeInputContext {
            index: i.index,
            outpoint: point,
            availability,
            confirmations: observed.map(|o| o.confirmations),
            coinbase: observed.map(|o| o.coinbase),
            prevout_verification: verification,
        });
    }
    Ok(inputs)
}

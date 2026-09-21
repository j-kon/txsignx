//! Regtest-only orchestration. No signatures or final fields are created here.
use bitcoin::{Network, Txid};
use std::error::Error;
use txsignx_core::PsbtReport;
use txsignx_node::{NodeContextReport, NodeError, NodeRpc};
use txsignx_policy::*;
use txsignx_wallet::WalletContextReport;
pub struct BroadcastOutcome {
    pub policy: PolicyReport,
    pub txid: Option<Txid>,
}
/// Recompute policy from bound facts; never accept a caller-supplied PASS verdict.
pub fn execute(
    rpc: &impl NodeRpc,
    text: &str,
    inspection: &PsbtReport,
    wallet: Option<&WalletContextReport>,
    node: Option<&NodeContextReport>,
    config: &PolicyConfig,
) -> Result<BroadcastOutcome, Box<dyn Error>> {
    let inspected = txsignx_core::analyze_psbt(text)?;
    if serde_json::to_value(&inspected)? != serde_json::to_value(inspection)? {
        return Err(NodeError::ContextMismatch.into());
    }
    let policy =
        PolicyEngine::development()?.evaluate_with_context(inspection, config, wallet, node)?;
    if policy.decision != PolicyDecision::Pass {
        return Ok(BroadcastOutcome { policy, txid: None });
    }
    let wallet = wallet.ok_or("broadcast requires full wallet context")?;
    let node = node.ok_or("broadcast requires full node context")?;
    if node.configured_network() != Network::Regtest
        || node.node_network() != Network::Regtest
        || wallet.configured_network().bitcoin_network() != Network::Regtest
    {
        return Err("broadcast is restricted to configured and node-reported Regtest".into());
    }
    if wallet.expected_change_outputs().is_empty() {
        return Err("broadcast requires explicit expected change".into());
    }
    for code in [
        "TG001", "TG004", "TG005", "TG006", "TG015", "TG016", "TG017",
    ] {
        if !policy
            .rule_evaluations
            .iter()
            .any(|e| e.code == code && e.status == RuleEvaluationStatus::Evaluated)
        {
            return Err("broadcast requires fully evaluated wallet and node rules".into());
        }
    }
    let psbt = txsignx_core::psbt::decode_psbt(text)?;
    for (input, facts) in psbt.inputs.iter().zip(&inspection.inputs) {
        let script = bitcoin::ScriptBuf::from_hex(
            facts
                .utxo
                .script_pubkey_hex
                .as_deref()
                .ok_or(NodeError::Extraction)?,
        )
        .map_err(|_| NodeError::Extraction)?;
        let sig = input
            .final_script_sig
            .as_ref()
            .is_some_and(|s| !s.is_empty());
        let witness = input
            .final_script_witness
            .as_ref()
            .is_some_and(|w| !w.is_empty());
        // Native witness requires a final witness. Legacy/wrapped scripts require a
        // final scriptSig; wrapped witness additionally reaches Core's script gate.
        if (script.is_witness_program() && (!witness || sig))
            || (!script.is_witness_program() && !sig)
        {
            return Err(NodeError::Extraction.into());
        }
    }
    let tx = psbt.extract_tx().map_err(|_| NodeError::Extraction)?;
    require_current_regtest(rpc, node)?;
    let acceptance = rpc.test_mempool_accept(&tx)?;
    if acceptance.txid != tx.compute_txid() || !acceptance.allowed {
        return Err(NodeError::MempoolRejected.into());
    }
    require_current_regtest(rpc, node)?;
    let txid = rpc.send_raw_transaction(&tx)?;
    if txid != tx.compute_txid() {
        return Err(NodeError::Rpc.into());
    }
    Ok(BroadcastOutcome {
        policy,
        txid: Some(txid),
    })
}

fn require_current_regtest(rpc: &impl NodeRpc, node: &NodeContextReport) -> Result<(), NodeError> {
    let current = rpc.blockchain_info()?;
    if current.network != Network::Regtest
        || current.initial_block_download
        || current.headers != current.blocks
        || current.blocks != node.tip().height
        || current.best_block_hash != node.tip().hash
        || current
            .verification_progress
            .is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
    {
        return Err(NodeError::ContextMismatch);
    }
    Ok(())
}

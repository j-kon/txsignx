use crate::*;
use txsignx_node::{NodeUtxoAvailability as A, PrevoutVerification as V};
pub enum NodeRule {
    WrongNetwork,
    ImmatureCoinbase,
    UtxoUnavailable,
    PrevoutMismatch,
    MempoolConflict,
}
impl PolicyRule for NodeRule {
    fn metadata(&self) -> RuleMetadata {
        let (code, title, description, severity) = match self {
            Self::WrongNetwork => (
                "TG001",
                "Wrong Network",
                "Explicit configured network differs from node-reported chain.",
                Severity::Critical,
            ),
            Self::ImmatureCoinbase => (
                "TG006",
                "Immature Coinbase Input",
                "Node-reported coinbase has fewer than 100 confirmations.",
                Severity::Critical,
            ),
            Self::UtxoUnavailable => (
                "TG015",
                "Node UTXO Unavailable",
                "Outpoint is unavailable in the node's queried UTXO views.",
                Severity::Critical,
            ),
            Self::PrevoutMismatch => (
                "TG016",
                "Node Prevout Mismatch",
                "Resolved PSBT prevout disagrees with node-reported value or script.",
                Severity::Critical,
            ),
            Self::MempoolConflict => (
                "TG017",
                "Mempool Spend Conflict",
                "Chain UTXO is absent from the node's mempool-aware view.",
                Severity::High,
            ),
        };
        RuleMetadata {
            code,
            title,
            description,
            default_severity: severity,
            active: true,
            required_context: vec!["explicit network and bound Bitcoin Core node context"],
        }
    }
    fn evaluation(
        &self,
        c: &PolicyContext<'_>,
    ) -> (RuleEvaluationStatus, Option<RuleEvaluationReason>) {
        let Some(n) = c.node else {
            return (
                RuleEvaluationStatus::NotEvaluated,
                Some(RuleEvaluationReason::NoNodeContext),
            );
        };
        let usable = match self {
            Self::ImmatureCoinbase => n
                .inputs()
                .iter()
                .filter(|i| i.coinbase.is_some() && i.confirmations.is_some())
                .count(),
            Self::PrevoutMismatch => n
                .inputs()
                .iter()
                .filter(|i| i.prevout_verification != V::Unavailable)
                .count(),
            _ => n.inputs().len(),
        };
        if usable == 0 {
            (
                RuleEvaluationStatus::NotEvaluated,
                Some(RuleEvaluationReason::NoUsableInputContext),
            )
        } else if usable < n.inputs().len() {
            (
                RuleEvaluationStatus::PartiallyEvaluated,
                Some(RuleEvaluationReason::SomeInputContextUnavailable),
            )
        } else {
            (RuleEvaluationStatus::Evaluated, None)
        }
    }
    fn evaluate(&self, c: &PolicyContext<'_>) -> Vec<Finding> {
        let Some(n) = c.node else {
            return vec![];
        };
        let m = self.metadata();
        if matches!(self, Self::WrongNetwork) {
            return if n.configured_network() != n.node_network() {
                vec![m.finding(FindingLocation::Global,"The explicitly configured network does not match the connected Bitcoin Core chain.","Verify the selected node and configured network.")]
            } else {
                vec![]
            };
        }
        n.inputs().iter().filter_map(|i|{
   let message=match self {
    Self::ImmatureCoinbase if i.coinbase==Some(true)&&i.confirmations.is_some_and(|n|n<100)=>format!("Bitcoin Core reports input {} is a coinbase with fewer than 100 confirmations.",i.index),
    Self::UtxoUnavailable if i.availability==A::NotAvailable=>format!("Input {} was not available in the connected node's current chain or mempool UTXO view.",i.index),
    Self::PrevoutMismatch=>{let what=match i.prevout_verification{V::ValueMismatch=>"value",V::ScriptMismatch=>"scriptPubKey",V::ValueAndScriptMismatch=>"value and scriptPubKey",_=>return None};format!("Input {} resolved PSBT {what} differs from Bitcoin Core's reported prevout.",i.index)},
    Self::MempoolConflict if i.availability==A::SpentInMempool=>format!("Input {} is present in Core's chain UTXO view but absent from its mempool-aware view, indicating a current mempool spend. Replacement workflows may be intentional.",i.index),
    _=>return None,
   };
   Some(m.finding(FindingLocation::Input{index:i.index},message,"Review the node-reported context and transaction intent."))
  }).collect()
    }
}

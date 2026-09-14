use crate::{PolicyEngine, PolicyError, RuleMetadata, rules::*};
use serde::Serialize;

impl PolicyEngine {
    pub fn development() -> Result<Self, PolicyError> {
        Self::new(vec![
            Box::new(ExcessiveAbsoluteFee),
            Box::new(ExcessiveFeePercentage),
            Box::new(UnknownWalletInput),
            Box::new(UnknownChangeOutput),
            Box::new(InvalidUtxoContext),
            Box::new(MissingUtxoContext),
            Box::new(UnusualSighashType),
            Box::new(UnknownOrProprietaryMetadata),
            Box::new(UnrecognizedScriptType),
            Box::new(NonZeroOpReturnValue),
        ])
    }
}
/// Reserved codes have no evaluator and no assigned severity.
#[derive(Debug, Clone, Serialize)]
pub struct DeferredRuleMetadata {
    pub code: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub active: bool,
    pub required_context: Vec<&'static str>,
}
#[derive(Debug, Clone, Serialize)]
pub struct RuleCatalog {
    pub active_rules: Vec<RuleMetadata>,
    pub deferred_rules: Vec<DeferredRuleMetadata>,
}
pub fn rule_catalog() -> Result<RuleCatalog, PolicyError> {
    let deferred_rules = [
        (
            "TG001",
            "Wrong Network",
            "explicit expected network / wallet context",
        ),
        (
            "TG006",
            "Immature Coinbase Input",
            "chain confirmation/height context",
        ),
        (
            "TG007",
            "Dust Output",
            "explicit relay/dust policy assumptions or node policy context",
        ),
        ("TG008", "Address Reuse", "wallet address/history context"),
    ]
    .into_iter()
    .map(|(code, title, context)| DeferredRuleMetadata {
        code,
        title,
        description: "Reserved / deferred; not evaluated in Milestone 4.",
        active: false,
        required_context: vec![context],
    })
    .collect();
    Ok(RuleCatalog {
        active_rules: PolicyEngine::development()?.metadata(),
        deferred_rules,
    })
}

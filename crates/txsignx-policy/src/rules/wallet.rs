use crate::*;
use txsignx_wallet::WalletOwnership;
pub struct UnknownWalletInput;
pub struct UnknownChangeOutput;
impl PolicyRule for UnknownWalletInput {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG004",
            title: "Unknown Wallet Input",
            description: "Resolved prevout has no descriptor match within the configured derivation window.",
            default_severity: Severity::High,
            active: true,
            required_context: vec!["public descriptor wallet context", "valid resolved prevout"],
        }
    }
    fn evaluation(
        &self,
        context: &PolicyContext<'_>,
    ) -> (RuleEvaluationStatus, Option<RuleEvaluationReason>) {
        let Some(wallet) = context.wallet else {
            return (
                RuleEvaluationStatus::NotEvaluated,
                Some(RuleEvaluationReason::NoWalletContext),
            );
        };
        let usable = wallet
            .inputs()
            .iter()
            .filter(|i| !matches!(i.ownership, WalletOwnership::Unavailable { .. }))
            .count();
        if usable == 0 {
            (
                RuleEvaluationStatus::NotEvaluated,
                Some(RuleEvaluationReason::NoUsableInputContext),
            )
        } else if usable < wallet.inputs().len() {
            (
                RuleEvaluationStatus::PartiallyEvaluated,
                Some(RuleEvaluationReason::SomeInputContextUnavailable),
            )
        } else {
            (RuleEvaluationStatus::Evaluated, None)
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        let Some(wallet) = context.wallet else {
            return vec![];
        };
        wallet.inputs().iter().filter(|i|i.ownership==WalletOwnership::NoMatchWithinWindow).map(|i|
            self.metadata().finding(FindingLocation::Input{index:i.index},
                format!("Input {}'s resolved previous output did not match the configured wallet descriptors within the {}-index derivation window.",i.index,wallet.derivation_window()),
                "Review the bounded descriptor context and transaction intent; collaborative transactions may legitimately include non-matching inputs.")
        ).collect()
    }
}
impl PolicyRule for UnknownChangeOutput {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG005",
            title: "Unknown Change Output",
            description: "Explicitly expected change does not match the internal keychain (HIGH external match; CRITICAL no match).",
            default_severity: Severity::High,
            active: true,
            required_context: vec![
                "public descriptor wallet context",
                "explicit expected-change output indexes",
            ],
        }
    }
    fn evaluation(
        &self,
        context: &PolicyContext<'_>,
    ) -> (RuleEvaluationStatus, Option<RuleEvaluationReason>) {
        match context.wallet {
            None => (
                RuleEvaluationStatus::NotEvaluated,
                Some(RuleEvaluationReason::NoWalletContext),
            ),
            Some(wallet) if wallet.expected_change_outputs().is_empty() => (
                RuleEvaluationStatus::NotEvaluated,
                Some(RuleEvaluationReason::NoExpectedChangeOutput),
            ),
            Some(_) => (RuleEvaluationStatus::Evaluated, None),
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        let Some(wallet) = context.wallet else {
            return vec![];
        };
        wallet.outputs().iter().filter(|o|o.expected_change).filter_map(|o| {
            let (severity,message)=match o.ownership {
                WalletOwnership::Internal{..}=>return None,
                WalletOwnership::External{..}=>(Severity::High,format!("Expected-change output {} matches the external/receive keychain instead of the internal/change keychain within the configured window.",o.index)),
                WalletOwnership::NoMatchWithinWindow=>(Severity::Critical,format!("Expected-change output {} did not match either configured wallet descriptor within the {}-index derivation window.",o.index,wallet.derivation_window())),
                // WalletContextReport construction never emits unavailable outputs.
                WalletOwnership::Unavailable{..}=>return None,
            };
            let mut finding=self.metadata().finding(FindingLocation::Output{index:o.index},message,"Verify the explicitly declared change destination and bounded descriptor configuration before signing.");
            finding.severity=severity;Some(finding)
        }).collect()
    }
}

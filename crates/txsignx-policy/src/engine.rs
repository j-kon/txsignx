use crate::*;
use std::collections::BTreeSet;
use txsignx_core::{
    PsbtReport,
    psbt::{PsbtFeeStatus, PsbtUtxoStatus},
};

/// Rules read already-derived facts. Implementations are trusted application code;
/// they must be deterministic and must not perform I/O or mutate input.
pub trait PolicyRule {
    fn metadata(&self) -> RuleMetadata;
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding>;
}
pub struct PolicyContext<'a> {
    pub inspection: &'a PsbtReport,
    pub config: &'a PolicyConfig,
}
struct RegisteredRule {
    metadata: RuleMetadata,
    rule: Box<dyn PolicyRule>,
}
pub struct PolicyEngine {
    rules: Vec<RegisteredRule>,
}
impl PolicyEngine {
    pub fn new(rules: Vec<Box<dyn PolicyRule>>) -> Result<Self, PolicyError> {
        let mut codes = BTreeSet::new();
        let mut registered = Vec::new();
        for rule in rules {
            let metadata = rule.metadata();
            let code = metadata.code.as_bytes();
            if code.len() != 5
                || !code.starts_with(b"TG")
                || !code[2..].iter().all(u8::is_ascii_digit)
            {
                return Err(PolicyError::InvalidRuleCode);
            }
            if !codes.insert(metadata.code) {
                return Err(PolicyError::DuplicateRuleCode);
            }
            registered.push(RegisteredRule { metadata, rule });
        }
        Ok(Self { rules: registered })
    }
    pub fn metadata(&self) -> Vec<RuleMetadata> {
        self.rules.iter().map(|r| r.metadata.clone()).collect()
    }
    pub fn evaluate(
        &self,
        inspection: &PsbtReport,
        config: &PolicyConfig,
    ) -> Result<PolicyReport, PolicyError> {
        config.validate()?;
        validate_inspection(inspection)?;
        let context = PolicyContext { inspection, config };
        let mut findings = Vec::new();
        let mut evaluated_rules = Vec::new();
        for registered in &self.rules {
            if !registered.metadata.active {
                continue;
            }
            let mut rule_findings = registered.rule.evaluate(&context);
            if rule_findings
                .iter()
                .any(|f| f.code != registered.metadata.code)
            {
                return Err(PolicyError::FindingCodeMismatch);
            }
            rule_findings.sort_by(|a, b| {
                a.location
                    .cmp(&b.location)
                    .then(a.severity.cmp(&b.severity))
                    .then(a.title.cmp(&b.title))
                    .then(a.message.cmp(&b.message))
                    .then(a.recommendation.cmp(&b.recommendation))
            });
            findings.extend(rule_findings);
            evaluated_rules.push(registered.metadata.code.to_owned());
        }
        let highest_severity = findings.iter().map(|f| f.severity).max();
        let (decision, risk_level) = match highest_severity {
            Some(Severity::Critical) => (PolicyDecision::Block, RiskLevel::Critical),
            Some(Severity::High) => (PolicyDecision::Review, RiskLevel::High),
            Some(Severity::Medium) => (PolicyDecision::Review, RiskLevel::Medium),
            _ => (PolicyDecision::Pass, RiskLevel::Low),
        };
        Ok(PolicyReport {
            decision,
            risk_level,
            highest_severity,
            finding_count: findings.len(),
            findings,
            evaluated_rules,
            config: *config,
            scope_note: POLICY_SCOPE,
        })
    }
}

fn validate_inspection(inspection: &PsbtReport) -> Result<(), PolicyError> {
    if inspection.input_count != inspection.inputs.len()
        || inspection.output_count != inspection.outputs.len()
    {
        return Err(PolicyError::InconsistentInspection);
    }
    let missing = inspection
        .inputs
        .iter()
        .any(|i| i.utxo.status == PsbtUtxoStatus::Missing);
    let invalid = inspection.inputs.iter().any(|i| {
        !matches!(
            i.utxo.status,
            PsbtUtxoStatus::Missing | PsbtUtxoStatus::Valid
        )
    });
    let has_fee = inspection.fee.fee_sats.is_some();
    let consistent = match inspection.fee.status {
        PsbtFeeStatus::Available => has_fee && !missing && !invalid,
        PsbtFeeStatus::MissingUtxoContext => !has_fee && missing && !invalid,
        PsbtFeeStatus::InvalidUtxoContext => !has_fee && invalid,
        PsbtFeeStatus::NegativeFee | PsbtFeeStatus::Overflow | PsbtFeeStatus::OtherError => {
            return Err(PolicyError::UnevaluableFeeState);
        }
    };
    if consistent {
        Ok(())
    } else {
        Err(PolicyError::InconsistentInspection)
    }
}

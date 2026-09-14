use crate::PolicyConfig;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Pass,
    Review,
    Block,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FindingLocation {
    Global,
    Input { index: usize },
    Output { index: usize },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub message: String,
    pub recommendation: Option<String>,
    pub location: FindingLocation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleMetadata {
    pub code: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub default_severity: Severity,
    pub active: bool,
    pub required_context: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleEvaluationStatus {
    Evaluated,
    PartiallyEvaluated,
    NotEvaluated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleEvaluationReason {
    NoWalletContext,
    NoExpectedChangeOutput,
    NoUsableInputContext,
    SomeInputContextUnavailable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleEvaluation {
    pub code: String,
    pub status: RuleEvaluationStatus,
    pub reason: Option<RuleEvaluationReason>,
}
pub const WALLET_POLICY_SCOPE: &str = "Policy scope is incomplete. PASS means no currently evaluated active policy requires review or blocking. Wallet matches are bounded by the configured derivation window and supplied prevout facts; only explicitly declared expected change is checked. Consult rule_evaluations for skipped or partial checks. Network, on-chain existence, confirmations, balances, address reuse and signature validity are not verified.";
pub const POLICY_SCOPE: &str = "Policy scope is incomplete. PASS means no currently evaluated active policy requires review or blocking; wallet-context rules are not evaluated without wallet context; wallet ownership, expected network, change detection, blockchain confirmations, address reuse, and cryptographic signature validity are not verified.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PolicyReport {
    pub decision: PolicyDecision,
    pub risk_level: RiskLevel,
    pub highest_severity: Option<Severity>,
    pub finding_count: usize,
    pub findings: Vec<Finding>,
    pub evaluated_rules: Vec<String>,
    pub rule_evaluations: Vec<RuleEvaluation>,
    pub config: PolicyConfig,
    pub scope_note: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreflightReport {
    pub inspection: txsignx_core::PsbtReport,
    pub policy: PolicyReport,
}

impl RuleMetadata {
    pub fn finding(
        &self,
        location: FindingLocation,
        message: impl Into<String>,
        recommendation: &str,
    ) -> Finding {
        Finding {
            code: self.code.to_owned(),
            severity: self.default_severity,
            title: self.title.to_owned(),
            message: message.into(),
            recommendation: Some(recommendation.to_owned()),
            location,
        }
    }
}

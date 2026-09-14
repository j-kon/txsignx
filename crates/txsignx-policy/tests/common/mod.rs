#![allow(dead_code)]
use txsignx_core::PsbtReport;
use txsignx_policy::*;
pub fn inspection() -> PsbtReport {
    txsignx_core::analyze_psbt(include_str!("../../../../fixtures/psbt-unsigned.b64")).unwrap()
}
pub struct TestRule {
    pub code: &'static str,
    pub active: bool,
    pub findings: Vec<Finding>,
}
impl PolicyRule for TestRule {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: self.code,
            title: "Test rule",
            description: "Deterministic test rule",
            default_severity: Severity::Info,
            active: self.active,
            required_context: vec!["inspection"],
        }
    }
    fn evaluate(&self, _: &PolicyContext<'_>) -> Vec<Finding> {
        self.findings.clone()
    }
}
pub fn finding(code: &str, severity: Severity, location: FindingLocation) -> Finding {
    Finding {
        code: code.into(),
        severity,
        title: "Test finding".into(),
        message: "Fixed test fact".into(),
        recommendation: None,
        location,
    }
}
pub fn engine_for(findings: Vec<Finding>) -> PolicyEngine {
    PolicyEngine::new(vec![Box::new(TestRule {
        code: "TG900",
        active: true,
        findings,
    })])
    .unwrap()
}

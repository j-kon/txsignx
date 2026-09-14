mod common;
use txsignx_policy::*;
#[test]
fn ordinary_rules_record_evaluated_status() {
    let report = PolicyEngine::development()
        .unwrap()
        .evaluate(&common::inspection(), &PolicyConfig::default())
        .unwrap();
    let fee = report
        .rule_evaluations
        .iter()
        .find(|r| r.code == "TG002")
        .unwrap();
    assert_eq!(fee.status, RuleEvaluationStatus::Evaluated);
    assert_eq!(fee.reason, None);
}

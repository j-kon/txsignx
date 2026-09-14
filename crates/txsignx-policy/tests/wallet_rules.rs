#[path = "../../txsignx-wallet/tests/common/mod.rs"]
mod wallet;
use txsignx_core::{
    PsbtReport,
    psbt::{PsbtFeeStatus, PsbtUtxoStatus},
};
use txsignx_policy::*;
use txsignx_wallet::*;
fn inspection() -> PsbtReport {
    let mut r =
        txsignx_core::analyze_psbt(include_str!("../../../fixtures/psbt-unsigned.b64")).unwrap();
    r.inputs[0].utxo.script_pubkey_hex = Some(wallet::script(0, 7).to_hex_string());
    r.outputs[1].transaction_output.script_pubkey_hex = wallet::script(1, 3).to_hex_string();
    r
}
fn evaluate(r: &PsbtReport, expected: &[usize]) -> PolicyReport {
    let index = WalletIndex::new(wallet::config(10)).unwrap();
    let context = index.classify(r, expected).unwrap();
    PolicyEngine::development()
        .unwrap()
        .evaluate_with_wallet(r, &PolicyConfig::default(), Some(&context))
        .unwrap()
}
fn status(r: &PolicyReport, code: &str) -> RuleEvaluationStatus {
    r.rule_evaluations
        .iter()
        .find(|e| e.code == code)
        .unwrap()
        .status
}
#[test]
fn valid_wallet_payment_is_pass_and_both_rules_evaluated() {
    let report = evaluate(&inspection(), &[1]);
    assert_eq!(report.decision, PolicyDecision::Pass);
    assert_eq!(status(&report, "TG004"), RuleEvaluationStatus::Evaluated);
    assert_eq!(status(&report, "TG005"), RuleEvaluationStatus::Evaluated);
}
#[test]
fn foreign_valid_input_is_high_review() {
    let mut r = inspection();
    r.inputs[0].utxo.script_pubkey_hex = Some(wallet::script(2, 0).to_hex_string());
    let report = evaluate(&r, &[1]);
    assert_eq!(report.decision, PolicyDecision::Review);
    assert!(report.findings.iter().any(|f| f.code == "TG004"
        && f.severity == Severity::High
        && f.message.contains("within")));
}
#[test]
fn internal_input_also_matches() {
    let mut r = inspection();
    r.inputs[0].utxo.script_pubkey_hex = Some(wallet::script(1, 0).to_hex_string());
    assert_eq!(evaluate(&r, &[1]).decision, PolicyDecision::Pass);
}
#[test]
fn external_expected_change_is_high_review() {
    let mut r = inspection();
    r.outputs[1].transaction_output.script_pubkey_hex = wallet::script(0, 3).to_hex_string();
    let report = evaluate(&r, &[1]);
    assert_eq!(report.decision, PolicyDecision::Review);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "TG005" && f.severity == Severity::High)
    );
}
#[test]
fn hijacked_expected_change_is_critical_block() {
    let mut r = inspection();
    r.outputs[1].transaction_output.script_pubkey_hex = wallet::script(2, 3).to_hex_string();
    let report = evaluate(&r, &[1]);
    assert_eq!(report.decision, PolicyDecision::Block);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "TG005" && f.severity == Severity::Critical)
    );
}
#[test]
fn no_expected_change_is_not_evaluated() {
    let report = evaluate(&inspection(), &[]);
    let e = report
        .rule_evaluations
        .iter()
        .find(|e| e.code == "TG005")
        .unwrap();
    assert_eq!(e.status, RuleEvaluationStatus::NotEvaluated);
    assert_eq!(e.reason, Some(RuleEvaluationReason::NoExpectedChangeOutput));
    assert!(!report.evaluated_rules.iter().any(|c| c == "TG005"));
}
#[test]
fn no_wallet_context_keeps_old_decision_and_explicit_skips() {
    let report = PolicyEngine::development()
        .unwrap()
        .evaluate(&inspection(), &PolicyConfig::default())
        .unwrap();
    assert_eq!(report.decision, PolicyDecision::Pass);
    assert_eq!(report.evaluated_rules.len(), 8);
    for code in ["TG004", "TG005"] {
        let e = report
            .rule_evaluations
            .iter()
            .find(|e| e.code == code)
            .unwrap();
        assert_eq!(e.status, RuleEvaluationStatus::NotEvaluated);
        assert_eq!(e.reason, Some(RuleEvaluationReason::NoWalletContext));
    }
}
#[test]
fn missing_and_invalid_inputs_do_not_duplicate_findings() {
    for (utxo, fee, code) in [
        (
            PsbtUtxoStatus::Missing,
            PsbtFeeStatus::MissingUtxoContext,
            "TG010",
        ),
        (
            PsbtUtxoStatus::TxidMismatch,
            PsbtFeeStatus::InvalidUtxoContext,
            "TG009",
        ),
    ] {
        let mut r = inspection();
        r.inputs[0].utxo.status = utxo;
        r.fee.status = fee;
        r.fee.fee_sats = None;
        let report = evaluate(&r, &[1]);
        assert_eq!(status(&report, "TG004"), RuleEvaluationStatus::NotEvaluated);
        assert!(report.findings.iter().any(|f| f.code == code));
        assert!(!report.findings.iter().any(|f| f.code == "TG004"));
    }
}
#[test]
fn mixed_available_context_is_partial() {
    let mut r = inspection();
    let mut i = r.inputs[0].clone();
    i.index = 1;
    i.utxo.status = PsbtUtxoStatus::Missing;
    r.inputs.push(i);
    r.input_count = 2;
    r.fee.status = PsbtFeeStatus::MissingUtxoContext;
    r.fee.fee_sats = None;
    assert_eq!(
        status(&evaluate(&r, &[1]), "TG004"),
        RuleEvaluationStatus::PartiallyEvaluated
    );
}
#[test]
fn collaborative_foreign_input_is_review() {
    let mut r = inspection();
    let mut i = r.inputs[0].clone();
    i.index = 1;
    i.utxo.script_pubkey_hex = Some(wallet::script(2, 0).to_hex_string());
    r.inputs.push(i);
    r.input_count = 2;
    let report = evaluate(&r, &[1]);
    assert_eq!(report.decision, PolicyDecision::Review);
    assert_eq!(
        report.findings.iter().filter(|f| f.code == "TG004").count(),
        1
    );
}
#[test]
fn multiple_expected_change_findings_are_sorted_by_output() {
    let mut r = inspection();
    r.outputs[0].transaction_output.script_pubkey_hex = wallet::script(0, 0).to_hex_string();
    r.outputs[1].transaction_output.script_pubkey_hex = wallet::script(2, 0).to_hex_string();
    let report = evaluate(&r, &[1, 0]);
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|f| f.code == "TG005")
            .map(|f| f.location)
            .collect::<Vec<_>>(),
        vec![
            FindingLocation::Output { index: 0 },
            FindingLocation::Output { index: 1 }
        ]
    );
}
#[test]
fn mismatched_wallet_report_is_rejected_by_policy() {
    let mut r = inspection();
    let context = WalletIndex::new(wallet::config(10))
        .unwrap()
        .classify(&r, &[1])
        .unwrap();
    r.outputs[1].transaction_output.script_pubkey_hex = wallet::script(2, 3).to_hex_string();
    assert_eq!(
        PolicyEngine::development().unwrap().evaluate_with_wallet(
            &r,
            &PolicyConfig::default(),
            Some(&context)
        ),
        Err(PolicyError::InconsistentWalletContext)
    );
}
#[test]
fn policy_reports_are_deterministic_with_wallet_context() {
    let r = inspection();
    let first = serde_json::to_string(&evaluate(&r, &[1])).unwrap();
    for _ in 0..3 {
        assert_eq!(serde_json::to_string(&evaluate(&r, &[1])).unwrap(), first);
    }
}

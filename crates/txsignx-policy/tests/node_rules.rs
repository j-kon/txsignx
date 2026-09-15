#[path = "../../txsignx-node/tests/common/mod.rs"]
mod common;
use bitcoin::Network;
use common::*;
use txsignx_node::*;
use txsignx_policy::*;
fn eval(f: &Fake, n: Network) -> PolicyReport {
    let r = inspection();
    let node = build_node_context(f, &r, n).unwrap();
    PolicyEngine::development()
        .unwrap()
        .evaluate_with_context(&r, &PolicyConfig::default(), None, Some(&node))
        .unwrap()
}
#[test]
fn wrong_configured_network_is_critical_without_claiming_psbt_network() {
    let r = eval(&Fake::new(), Network::Bitcoin);
    assert_eq!(r.decision, PolicyDecision::Block);
    let f = r.findings.iter().find(|f| f.code == "TG001").unwrap();
    assert_eq!(f.severity, Severity::Critical);
    assert!(f.message.contains("explicitly configured"));
    assert!(!f.message.contains("PSBT is"));
}
#[test]
fn coinbase_boundary_99_100_101() {
    for count in [99, 100, 101] {
        let mut f = Fake::new();
        for o in [&mut f.chain, &mut f.mempool] {
            let o = o.as_mut().unwrap();
            o.coinbase = true;
            o.confirmations = count;
        }
        let r = eval(&f, Network::Regtest);
        assert_eq!(r.findings.iter().any(|f| f.code == "TG006"), count < 100);
    }
}
#[test]
fn unavailable_utxo_blocks_without_historical_claim() {
    let mut f = Fake::new();
    f.chain = None;
    f.mempool = None;
    let r = eval(&f, Network::Regtest);
    assert_eq!(r.decision, PolicyDecision::Block);
    assert!(r.findings.iter().any(|f| f.code == "TG015"));
    assert!(!serde_json::to_string(&r).unwrap().contains("never existed"));
}
#[test]
fn node_prevout_mismatch_blocks() {
    let mut f = Fake::new();
    for o in [&mut f.chain, &mut f.mempool] {
        o.as_mut().unwrap().output.value += bitcoin::Amount::from_sat(1);
    }
    let r = eval(&f, Network::Regtest);
    assert_eq!(r.decision, PolicyDecision::Block);
    assert!(r.findings.iter().any(|f| f.code == "TG016"));
}
#[test]
fn mempool_conflict_requires_review() {
    let mut f = Fake::new();
    f.mempool = None;
    let r = eval(&f, Network::Regtest);
    assert_eq!(r.decision, PolicyDecision::Review);
    let finding = r.findings.iter().find(|f| f.code == "TG017").unwrap();
    assert_eq!(finding.severity, Severity::High);
    assert!(!finding.message.contains("malicious"));
}
#[test]
fn absent_node_skips_all_node_rules() {
    let r = PolicyEngine::development()
        .unwrap()
        .evaluate(&inspection(), &PolicyConfig::default())
        .unwrap();
    for code in ["TG001", "TG006", "TG015", "TG016", "TG017"] {
        assert_eq!(
            r.rule_evaluations
                .iter()
                .find(|e| e.code == code)
                .unwrap()
                .status,
            RuleEvaluationStatus::NotEvaluated
        );
    }
}
#[test]
fn complete_node_context_evaluates_all_node_rules() {
    let r = eval(&Fake::new(), Network::Regtest);
    assert_eq!(r.decision, PolicyDecision::Pass);
    for code in ["TG001", "TG006", "TG015", "TG016", "TG017"] {
        assert_eq!(
            r.rule_evaluations
                .iter()
                .find(|e| e.code == code)
                .unwrap()
                .status,
            RuleEvaluationStatus::Evaluated
        );
    }
}
#[test]
fn policy_rejects_stale_node_context() {
    let mut r = inspection();
    let n = build_node_context(&Fake::new(), &r, Network::Regtest).unwrap();
    r.inputs[0].utxo.value_sats = Some(2);
    assert_eq!(
        PolicyEngine::development()
            .unwrap()
            .evaluate_with_context(&r, &PolicyConfig::default(), None, Some(&n))
            .unwrap_err(),
        PolicyError::InconsistentNodeContext
    );
}

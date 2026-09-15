use txsignx_policy::*;
fn evaluate(text: &str) -> PreflightReport {
    let inspection = txsignx_core::analyze_psbt(text).unwrap();
    let before = serde_json::to_string(&inspection).unwrap();
    let engine = PolicyEngine::development().unwrap();
    let policy = engine
        .evaluate(&inspection, &PolicyConfig::default())
        .unwrap();
    assert_eq!(serde_json::to_string(&inspection).unwrap(), before);
    let json = serde_json::to_string(&policy).unwrap();
    for _ in 0..3 {
        assert_eq!(
            serde_json::to_string(
                &engine
                    .evaluate(&inspection, &PolicyConfig::default())
                    .unwrap()
            )
            .unwrap(),
            json
        );
    }
    PreflightReport {
        inspection,
        wallet_context: None,
        policy,
    }
}
macro_rules! demo {
    ($test:ident,$file:literal,$decision:ident,$risk:ident,[$($code:literal),*])=>{
        #[test] fn $test() {
            let result=evaluate(include_str!(concat!("../../../fixtures/policy/",$file,".b64")));
            assert_eq!(result.policy.decision,PolicyDecision::$decision);
            assert_eq!(result.policy.risk_level,RiskLevel::$risk);
            assert_eq!(result.policy.findings.iter().map(|f|f.code.as_str()).collect::<Vec<_>>(),vec![$($code),*] as Vec<&str>);
            let json=serde_json::to_value(result).unwrap();
            assert!(json.get("inspection").is_some() && json.get("policy").is_some());
        }
    }
}
demo!(default_pass, "pass", Pass, Low, []);
demo!(
    absolute_fee_block,
    "absolute-fee",
    Block,
    Critical,
    ["TG002"]
);
demo!(
    percentage_fee_block,
    "percentage-fee",
    Block,
    Critical,
    ["TG003"]
);
demo!(
    missing_context_review,
    "missing-utxo",
    Review,
    High,
    ["TG010"]
);
demo!(
    invalid_context_block,
    "invalid-utxo",
    Block,
    Critical,
    ["TG009"]
);
demo!(
    unusual_sighash_review,
    "unusual-sighash",
    Review,
    High,
    ["TG011"]
);
demo!(op_return_block, "op-return", Block, Critical, ["TG014"]);
demo!(
    unknown_script_review,
    "unknown-script",
    Review,
    Medium,
    ["TG013"]
);
#[test]
fn capstone_has_exact_payment_fee_and_critical_findings() {
    let result = evaluate(include_str!("../../../fixtures/policy/800k-fee.b64"));
    assert_eq!(result.inspection.total_output_sats, 100_000);
    assert_eq!(result.inspection.fee.fee_sats, Some(800_000));
    assert_eq!(result.policy.decision, PolicyDecision::Block);
    assert_eq!(result.policy.risk_level, RiskLevel::Critical);
    assert_eq!(
        result
            .policy
            .findings
            .iter()
            .map(|f| (f.code.as_str(), f.severity))
            .collect::<Vec<_>>(),
        vec![("TG002", Severity::Critical), ("TG003", Severity::Critical)]
    );
}
#[test]
fn negative_fee_inspection_cannot_become_successful_policy_evaluation() {
    let inspection =
        txsignx_core::analyze_psbt(include_str!("../../../fixtures/policy/negative-fee.b64"))
            .unwrap();
    assert_eq!(
        PolicyEngine::development()
            .unwrap()
            .evaluate(&inspection, &PolicyConfig::default()),
        Err(PolicyError::UnevaluableFeeState)
    );
}

demo!(
    extension_metadata_pass,
    "extension-metadata",
    Pass,
    Low,
    ["TG012"]
);

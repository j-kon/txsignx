mod common;
use common::*;
use txsignx_core::psbt::{PsbtFeeStatus, PsbtUtxoStatus};
use txsignx_policy::*;

#[test]
fn empty_registry_is_scoped_pass() {
    let report = PolicyEngine::new(vec![])
        .unwrap()
        .evaluate(&inspection(), &PolicyConfig::default())
        .unwrap();
    assert_eq!(report.decision, PolicyDecision::Pass);
    assert_eq!(report.risk_level, RiskLevel::Low);
    assert_eq!(report.highest_severity, None);
    assert_eq!(report.finding_count, 0);
    assert!(report.scope_note.contains("scope is incomplete"));
}
macro_rules! severity_case {
    ($name:ident,$severity:ident,$decision:ident,$risk:ident) => {
        #[test]
        fn $name() {
            let report = engine_for(vec![finding(
                "TG900",
                Severity::$severity,
                FindingLocation::Global,
            )])
            .evaluate(&inspection(), &PolicyConfig::default())
            .unwrap();
            assert_eq!(
                (report.decision, report.risk_level, report.highest_severity),
                (
                    PolicyDecision::$decision,
                    RiskLevel::$risk,
                    Some(Severity::$severity)
                )
            );
        }
    };
}
severity_case!(info_is_pass_low, Info, Pass, Low);
severity_case!(low_is_pass_low, Low, Pass, Low);
severity_case!(medium_is_review, Medium, Review, Medium);
severity_case!(high_is_review, High, Review, High);
severity_case!(critical_is_block, Critical, Block, Critical);
#[test]
fn critical_dominates_without_additive_scoring() {
    let report = engine_for(vec![
        finding("TG900", Severity::Critical, FindingLocation::Global),
        finding("TG900", Severity::High, FindingLocation::Input { index: 0 }),
    ])
    .evaluate(&inspection(), &PolicyConfig::default())
    .unwrap();
    assert_eq!(report.decision, PolicyDecision::Block);
    assert_eq!(report.finding_count, 2);
}
#[test]
fn duplicate_codes_rejected_even_if_inactive() {
    let rules: Vec<Box<dyn PolicyRule>> = vec![
        Box::new(TestRule {
            code: "TG900",
            active: true,
            findings: vec![],
        }),
        Box::new(TestRule {
            code: "TG900",
            active: false,
            findings: vec![],
        }),
    ];
    assert!(matches!(
        PolicyEngine::new(rules),
        Err(PolicyError::DuplicateRuleCode)
    ));
}
#[test]
fn invalid_codes_and_mismatched_finding_ids_rejected() {
    assert!(matches!(
        PolicyEngine::new(vec![Box::new(TestRule {
            code: "TG900\n",
            active: true,
            findings: vec![]
        })]),
        Err(PolicyError::InvalidRuleCode)
    ));
    let engine = engine_for(vec![finding(
        "TG901",
        Severity::Info,
        FindingLocation::Global,
    )]);
    assert_eq!(
        engine.evaluate(&inspection(), &PolicyConfig::default()),
        Err(PolicyError::FindingCodeMismatch)
    );
}
#[test]
fn registration_then_location_order_is_stable_without_mutation() {
    let input = inspection();
    let before = serde_json::to_string(&input).unwrap();
    let engine = PolicyEngine::new(vec![
        Box::new(TestRule {
            code: "TG901",
            active: true,
            findings: vec![
                finding(
                    "TG901",
                    Severity::Info,
                    FindingLocation::Output { index: 1 },
                ),
                finding("TG901", Severity::Info, FindingLocation::Input { index: 0 }),
                finding("TG901", Severity::Info, FindingLocation::Global),
            ],
        }),
        Box::new(TestRule {
            code: "TG900",
            active: true,
            findings: vec![finding("TG900", Severity::Low, FindingLocation::Global)],
        }),
        Box::new(TestRule {
            code: "TG902",
            active: false,
            findings: vec![finding(
                "TG902",
                Severity::Critical,
                FindingLocation::Global,
            )],
        }),
    ])
    .unwrap();
    let report = engine.evaluate(&input, &PolicyConfig::default()).unwrap();
    assert_eq!(report.evaluated_rules, vec!["TG901", "TG900"]);
    assert_eq!(
        report
            .findings
            .iter()
            .map(|f| f.location)
            .collect::<Vec<_>>(),
        vec![
            FindingLocation::Global,
            FindingLocation::Input { index: 0 },
            FindingLocation::Output { index: 1 },
            FindingLocation::Global
        ]
    );
    let json = serde_json::to_string(&report).unwrap();
    for _ in 0..10 {
        assert_eq!(
            serde_json::to_string(&engine.evaluate(&input, &PolicyConfig::default()).unwrap())
                .unwrap(),
            json
        );
    }
    assert_eq!(serde_json::to_string(&input).unwrap(), before);
}
#[test]
fn invalid_config_rejected_before_evaluation() {
    assert_eq!(
        engine_for(vec![]).evaluate(
            &inspection(),
            &PolicyConfig {
                max_absolute_fee_sats: 0,
                max_fee_ratio_bps: 10001
            }
        ),
        Err(PolicyError::InvalidFeeRatio)
    );
}
#[test]
fn unsupported_fee_errors_cannot_become_a_pass() {
    for status in [
        PsbtFeeStatus::NegativeFee,
        PsbtFeeStatus::Overflow,
        PsbtFeeStatus::OtherError,
    ] {
        let mut input = inspection();
        input.fee.status = status;
        input.fee.fee_sats = None;
        assert_eq!(
            engine_for(vec![]).evaluate(&input, &PolicyConfig::default()),
            Err(PolicyError::UnevaluableFeeState)
        );
    }
}
#[test]
fn inconsistent_fee_summaries_are_rejected() {
    let engine = engine_for(vec![]);
    let mut input = inspection();
    input.fee.fee_sats = None;
    assert_eq!(
        engine.evaluate(&input, &PolicyConfig::default()),
        Err(PolicyError::InconsistentInspection)
    );
    input.fee.status = PsbtFeeStatus::MissingUtxoContext;
    assert_eq!(
        engine.evaluate(&input, &PolicyConfig::default()),
        Err(PolicyError::InconsistentInspection)
    );
    input.inputs[0].utxo.status = PsbtUtxoStatus::Missing;
    assert!(engine.evaluate(&input, &PolicyConfig::default()).is_ok());
    input.fee.status = PsbtFeeStatus::InvalidUtxoContext;
    assert_eq!(
        engine.evaluate(&input, &PolicyConfig::default()),
        Err(PolicyError::InconsistentInspection)
    );
}

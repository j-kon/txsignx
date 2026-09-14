mod common;
use txsignx_core::psbt::{PsbtFeeStatus, PsbtSighashReport, PsbtUtxoStatus};
use txsignx_policy::{
    rules::{InvalidUtxoContext, MissingUtxoContext, UnusualSighashType},
    *,
};
fn findings(rule: &dyn PolicyRule, status: PsbtUtxoStatus) -> Vec<Finding> {
    let mut input = common::inspection();
    input.inputs[0].utxo.status = status;
    rule.evaluate(&PolicyContext {
        inspection: &input,
        config: &PolicyConfig::default(),
    })
}
#[test]
fn each_invalid_utxo_status_is_input_scoped_and_critical() {
    for status in [
        PsbtUtxoStatus::TxidMismatch,
        PsbtUtxoStatus::VoutOutOfRange,
        PsbtUtxoStatus::WitnessNonWitnessMismatch,
    ] {
        let result = findings(&InvalidUtxoContext, status);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].code, "TG009");
        assert_eq!(result[0].location, FindingLocation::Input { index: 0 });
        assert_eq!(result[0].severity, Severity::Critical);
        assert!(findings(&MissingUtxoContext, status).is_empty());
    }
}
#[test]
fn missing_utxo_requires_review_without_calling_it_invalid() {
    let result = findings(&MissingUtxoContext, PsbtUtxoStatus::Missing);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code, "TG010");
    assert_eq!(result[0].severity, Severity::High);
    assert!(findings(&InvalidUtxoContext, PsbtUtxoStatus::Missing).is_empty());
}
#[test]
fn consistent_utxo_has_no_context_findings() {
    assert!(findings(&InvalidUtxoContext, PsbtUtxoStatus::Valid).is_empty());
    assert!(findings(&MissingUtxoContext, PsbtUtxoStatus::Valid).is_empty());
}
#[test]
fn mixed_invalid_missing_context_keeps_each_reason_and_skips_fee_rules() {
    use txsignx_policy::rules::{ExcessiveAbsoluteFee, ExcessiveFeePercentage};
    let mut input = common::inspection();
    let mut second = input.inputs[0].clone();
    second.index = 1;
    second.utxo.status = PsbtUtxoStatus::Missing;
    input.inputs.push(second);
    input.input_count = 2;
    input.inputs[0].utxo.status = PsbtUtxoStatus::TxidMismatch;
    input.fee.status = PsbtFeeStatus::InvalidUtxoContext;
    input.fee.fee_sats = None;
    let engine = PolicyEngine::new(vec![
        Box::new(ExcessiveAbsoluteFee),
        Box::new(ExcessiveFeePercentage),
        Box::new(InvalidUtxoContext),
        Box::new(MissingUtxoContext),
    ])
    .unwrap();
    let result = engine.evaluate(&input, &PolicyConfig::default()).unwrap();
    assert_eq!(result.decision, PolicyDecision::Block);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| f.code.as_str())
            .collect::<Vec<_>>(),
        vec!["TG009", "TG010"]
    );
    assert_eq!(
        result.findings[1].location,
        FindingLocation::Input { index: 1 }
    );
}
#[test]
fn absent_default_and_all_sighashes_are_ordinary() {
    let mut input = common::inspection();
    for value in [None, Some(0), Some(1)] {
        input.inputs[0].sighash_type = value.map(|v| PsbtSighashReport {
            value: v,
            name: "irrelevant display text".into(),
        });
        assert!(
            UnusualSighashType
                .evaluate(&PolicyContext {
                    inspection: &input,
                    config: &PolicyConfig::default()
                })
                .is_empty()
        );
    }
}
#[test]
fn numeric_sighash_variants_require_review_regardless_of_name() {
    for value in [2, 3, 0x81, 0x82, 0x83, 0x80, 0xdeadbeef, u32::MAX] {
        let mut input = common::inspection();
        input.inputs[0].sighash_type = Some(PsbtSighashReport {
            value,
            name: "\x1b[31mPRIVATE_SENTINEL".into(),
        });
        let result = UnusualSighashType.evaluate(&PolicyContext {
            inspection: &input,
            config: &PolicyConfig::default(),
        });
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].severity, Severity::High);
        assert_eq!(result[0].location, FindingLocation::Input { index: 0 });
        assert!(!result[0].message.contains("PRIVATE_SENTINEL"));
        assert!(result[0].message.contains(&value.to_string()));
    }
}

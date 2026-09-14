mod common;
use txsignx_core::psbt::PsbtFeeStatus;
use txsignx_policy::{
    rules::{ExcessiveAbsoluteFee, ExcessiveFeePercentage},
    *,
};
fn evaluate(
    rule: &dyn PolicyRule,
    outputs: u64,
    fee: Option<u64>,
    config: PolicyConfig,
) -> Vec<Finding> {
    let mut input = common::inspection();
    input.total_output_sats = outputs;
    input.fee.fee_sats = fee;
    input.fee.status = if fee.is_some() {
        PsbtFeeStatus::Available
    } else {
        PsbtFeeStatus::MissingUtxoContext
    };
    rule.evaluate(&PolicyContext {
        wallet: None,
        inspection: &input,
        config: &config,
    })
}
#[test]
fn absolute_fee_below_and_at_threshold_do_not_trigger() {
    for fee in [0, 99_999, 100_000] {
        assert!(
            evaluate(
                &ExcessiveAbsoluteFee,
                1_000_000,
                Some(fee),
                PolicyConfig::default()
            )
            .is_empty()
        );
    }
}
#[test]
fn absolute_fee_one_sat_above_threshold_is_critical() {
    let findings = evaluate(
        &ExcessiveAbsoluteFee,
        1_000_000,
        Some(100_001),
        PolicyConfig::default(),
    );
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "TG002");
    assert_eq!(findings[0].severity, Severity::Critical);
    assert!(findings[0].message.contains("100001") && findings[0].message.contains("100000"));
}
#[test]
fn absolute_fee_configuration_including_zero_is_respected() {
    for (threshold, fee, trigger) in [
        (0, 0, false),
        (0, 1, true),
        (1, 1, false),
        (u64::MAX, u64::MAX, false),
    ] {
        assert_eq!(
            !evaluate(
                &ExcessiveAbsoluteFee,
                0,
                Some(fee),
                PolicyConfig {
                    max_absolute_fee_sats: threshold,
                    ..PolicyConfig::default()
                }
            )
            .is_empty(),
            trigger
        );
    }
}
#[test]
fn percentage_below_equal_and_above_boundary() {
    for (outputs, fee, trigger) in [(91, 9, false), (90, 10, false), (89, 11, true)] {
        assert_eq!(
            !evaluate(
                &ExcessiveFeePercentage,
                outputs,
                Some(fee),
                PolicyConfig::default()
            )
            .is_empty(),
            trigger
        );
    }
}
#[test]
fn capstone_800k_fee_over_900k_inputs_triggers_both_rules() {
    for rule in [
        &ExcessiveAbsoluteFee as &dyn PolicyRule,
        &ExcessiveFeePercentage,
    ] {
        let findings = evaluate(rule, 100_000, Some(800_000), PolicyConfig::default());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Critical);
    }
    let findings = evaluate(
        &ExcessiveFeePercentage,
        100_000,
        Some(800_000),
        PolicyConfig::default(),
    );
    assert!(
        findings[0].message.contains("900000") && findings[0].message.contains("total input value")
    );
}
#[test]
fn percentage_zero_fee_zero_outputs_and_zero_threshold() {
    for (outputs, fee, bps, trigger) in [
        (0, 0, 0, false),
        (100, 0, 0, false),
        (100, 1, 0, true),
        (0, 1, 1000, true),
        (0, 1, 10000, false),
    ] {
        assert_eq!(
            !evaluate(
                &ExcessiveFeePercentage,
                outputs,
                Some(fee),
                PolicyConfig {
                    max_fee_ratio_bps: bps,
                    ..PolicyConfig::default()
                }
            )
            .is_empty(),
            trigger
        );
    }
}
#[test]
fn percentage_widens_addition_and_products_at_u64_boundaries() {
    for (outputs, fee, bps, trigger) in [
        (u64::MAX, u64::MAX, 5000, false),
        (u64::MAX, u64::MAX, 4999, true),
        (u64::MAX, 1, 0, true),
        (0, u64::MAX, 9999, true),
        (u64::MAX, u64::MAX, 10000, false),
    ] {
        assert_eq!(
            !evaluate(
                &ExcessiveFeePercentage,
                outputs,
                Some(fee),
                PolicyConfig {
                    max_fee_ratio_bps: bps,
                    ..PolicyConfig::default()
                }
            )
            .is_empty(),
            trigger
        );
    }
}
#[test]
fn unavailable_fee_never_fabricates_threshold_findings() {
    for rule in [
        &ExcessiveAbsoluteFee as &dyn PolicyRule,
        &ExcessiveFeePercentage,
    ] {
        assert!(evaluate(rule, 0, None, PolicyConfig::default()).is_empty());
        for status in [
            PsbtFeeStatus::MissingUtxoContext,
            PsbtFeeStatus::InvalidUtxoContext,
            PsbtFeeStatus::NegativeFee,
            PsbtFeeStatus::Overflow,
            PsbtFeeStatus::OtherError,
        ] {
            let mut input = common::inspection();
            input.fee.status = status;
            input.fee.fee_sats = Some(u64::MAX);
            assert!(
                rule.evaluate(&PolicyContext {
                    wallet: None,
                    inspection: &input,
                    config: &PolicyConfig::default()
                })
                .is_empty()
            );
        }
    }
}

mod common;
use txsignx_core::psbt::{PsbtFeeStatus, PsbtUtxoStatus};
use txsignx_policy::*;
#[test]
fn boundary_grid_uses_strict_integer_fee_share_without_rounding() {
    use txsignx_policy::rules::ExcessiveFeePercentage;
    let mut input = common::inspection();
    // Independent small-domain rational comparison; catches threshold rounding,
    // fee/output denominator mistakes, and accidental >= boundaries.
    for output in 0u64..=30 {
        for fee in 0u64..=30 {
            for bps in [0, 1, 999, 1000, 3333, 5000, 9999, 10000] {
                input.total_output_sats = output;
                input.fee.fee_sats = Some(fee);
                let config = PolicyConfig {
                    max_fee_ratio_bps: bps,
                    ..PolicyConfig::default()
                };
                let expected = fee * 10_000 > (output + fee) * u64::from(bps);
                assert_eq!(
                    !ExcessiveFeePercentage
                        .evaluate(&PolicyContext {
                            inspection: &input,
                            config: &config
                        })
                        .is_empty(),
                    expected
                );
            }
        }
    }
}
#[test]
fn input_and_output_location_sorting_survives_permuted_vectors() {
    use txsignx_core::transaction::ScriptType;
    let mut input = common::inspection();
    let mut second = input.inputs[0].clone();
    second.index = 1;
    input.inputs.push(second);
    input.input_count = 2;
    for i in &mut input.inputs {
        i.utxo.script_type = Some(ScriptType::Unknown);
    }
    for o in &mut input.outputs {
        o.transaction_output.script_type = ScriptType::Unknown;
    }
    let engine = PolicyEngine::development().unwrap();
    let expected = engine.evaluate(&input, &PolicyConfig::default()).unwrap();
    input.inputs.reverse();
    input.outputs.reverse();
    assert_eq!(
        engine.evaluate(&input, &PolicyConfig::default()).unwrap(),
        expected
    );
    assert_eq!(
        expected
            .findings
            .iter()
            .map(|f| f.location)
            .collect::<Vec<_>>(),
        vec![
            FindingLocation::Input { index: 0 },
            FindingLocation::Input { index: 1 },
            FindingLocation::Output { index: 0 },
            FindingLocation::Output { index: 1 }
        ]
    );
}
#[test]
fn all_unavailable_fee_states_have_explicit_outcomes() {
    let engine = PolicyEngine::development().unwrap();
    for (status, utxo, expected) in [
        (
            PsbtFeeStatus::MissingUtxoContext,
            PsbtUtxoStatus::Missing,
            Some(PolicyDecision::Review),
        ),
        (
            PsbtFeeStatus::InvalidUtxoContext,
            PsbtUtxoStatus::TxidMismatch,
            Some(PolicyDecision::Block),
        ),
        (PsbtFeeStatus::NegativeFee, PsbtUtxoStatus::Valid, None),
        (PsbtFeeStatus::Overflow, PsbtUtxoStatus::Valid, None),
        (PsbtFeeStatus::OtherError, PsbtUtxoStatus::Valid, None),
    ] {
        let mut input = common::inspection();
        input.fee.status = status;
        input.fee.fee_sats = None;
        input.inputs[0].utxo.status = utxo;
        let result = engine.evaluate(&input, &PolicyConfig::default());
        if let Some(decision) = expected {
            let report = result.unwrap();
            assert_eq!(report.decision, decision);
            assert!(
                !report
                    .findings
                    .iter()
                    .any(|f| matches!(f.code.as_str(), "TG002" | "TG003"))
            );
        } else {
            assert_eq!(result, Err(PolicyError::UnevaluableFeeState));
        }
    }
}
#[test]
fn inconsistent_map_counts_are_rejected_before_rules_run() {
    let mut input = common::inspection();
    input.input_count += 1;
    assert_eq!(
        PolicyEngine::development()
            .unwrap()
            .evaluate(&input, &PolicyConfig::default()),
        Err(PolicyError::InconsistentInspection)
    );
}
#[test]
fn extension_info_does_not_escalate_and_policy_never_uses_metadata_text() {
    let mut input = common::inspection();
    input.unknown_count = 1;
    input.unsigned_txid = "PRIVATE_SENTINEL\x1b[31m".into();
    input.format = "PRIVATE_SENTINEL".into();
    input.inputs[0].previous_txid = "PRIVATE_SENTINEL".into();
    input.inputs[0].utxo.script_pubkey_hex = Some("PRIVATE_SENTINEL".into());
    let report = PolicyEngine::development()
        .unwrap()
        .evaluate(&input, &PolicyConfig::default())
        .unwrap();
    assert_eq!(report.decision, PolicyDecision::Pass);
    assert_eq!(report.highest_severity, Some(Severity::Info));
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("PRIVATE_SENTINEL"));
    for claim in [
        "safe to sign",
        "transaction is safe",
        "signing_allowed",
        "feerate",
    ] {
        assert!(!json.contains(claim));
    }
}

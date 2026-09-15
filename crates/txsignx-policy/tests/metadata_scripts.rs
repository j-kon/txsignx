mod common;
use txsignx_core::psbt::PsbtUtxoStatus;
use txsignx_core::transaction::ScriptType;
use txsignx_policy::{
    rules::{NonZeroOpReturnValue, UnknownOrProprietaryMetadata, UnrecognizedScriptType},
    *,
};
fn evaluate(rule: &dyn PolicyRule, input: &txsignx_core::PsbtReport) -> Vec<Finding> {
    rule.evaluate(&PolicyContext {
        wallet: None,
        inspection: input,
        config: &PolicyConfig::default(),
    })
}
#[test]
fn no_extension_metadata_has_no_finding() {
    assert!(evaluate(&UnknownOrProprietaryMetadata, &common::inspection()).is_empty());
}
#[test]
fn metadata_at_every_location_yields_one_count_only_info_finding() {
    for location in 0..6 {
        let mut input = common::inspection();
        match location {
            0 => input.unknown_count = 1,
            1 => input.proprietary_count = 1,
            2 => input.inputs[0].unknown_count = 1,
            3 => input.inputs[0].proprietary_count = 1,
            4 => input.outputs[0].unknown_count = 1,
            _ => input.outputs[0].proprietary_count = 1,
        }
        let result = evaluate(&UnknownOrProprietaryMetadata, &input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].severity, Severity::Info);
        assert_eq!(result[0].location, FindingLocation::Global);
        assert!(result[0].message.contains("does not interpret"));
    }
}
#[test]
fn metadata_counts_widen_and_unrelated_strings_do_not_leak() {
    let mut input = common::inspection();
    input.unknown_count = usize::MAX;
    input.proprietary_count = usize::MAX;
    input.inputs[0].unknown_count = usize::MAX;
    input.outputs[0].proprietary_count = usize::MAX;
    input.unsigned_txid = "PRIVATE_SENTINEL\x1b[31m".into();
    input.inputs[0].utxo.script_pubkey_hex = Some("PRIVATE_SENTINEL".into());
    let result = evaluate(&UnknownOrProprietaryMetadata, &input);
    assert_eq!(result.len(), 1);
    assert!(!result[0].message.contains("PRIVATE_SENTINEL"));
    assert!(
        result[0]
            .message
            .contains(&(2 * (usize::MAX as u128)).to_string())
    );
}
#[test]
fn recognized_templates_do_not_trigger_unknown_script_rule() {
    for script in [
        ScriptType::P2pkh,
        ScriptType::P2sh,
        ScriptType::P2wpkh,
        ScriptType::P2wsh,
        ScriptType::P2tr,
        ScriptType::OpReturn,
    ] {
        let mut input = common::inspection();
        for output in &mut input.outputs {
            output.transaction_output.script_type = script;
        }
        input.inputs[0].utxo.script_type = Some(script);
        assert!(evaluate(&UnrecognizedScriptType, &input).is_empty());
    }
}
#[test]
fn unknown_outputs_and_valid_resolved_prevouts_have_scoped_findings() {
    let mut input = common::inspection();
    input.outputs[1].transaction_output.script_type = ScriptType::Unknown;
    input.inputs[0].utxo.script_type = Some(ScriptType::Unknown);
    let result = evaluate(&UnrecognizedScriptType, &input);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].location, FindingLocation::Input { index: 0 });
    assert_eq!(result[1].location, FindingLocation::Output { index: 1 });
    assert!(result.iter().all(|f| f.severity == Severity::Medium));
    input.inputs[0].utxo.status = PsbtUtxoStatus::TxidMismatch;
    assert_eq!(evaluate(&UnrecognizedScriptType, &input).len(), 1);
}
#[test]
fn zero_value_op_return_is_not_flagged() {
    let mut input = common::inspection();
    input.outputs[0].transaction_output.script_type = ScriptType::OpReturn;
    input.outputs[0].transaction_output.value_sats = 0;
    assert!(evaluate(&NonZeroOpReturnValue, &input).is_empty());
}
#[test]
fn positive_op_return_is_critical_and_non_op_return_is_not() {
    for sats in [1, u64::MAX] {
        let mut input = common::inspection();
        input.outputs[0].transaction_output.script_type = ScriptType::OpReturn;
        input.outputs[0].transaction_output.value_sats = sats;
        let result = evaluate(&NonZeroOpReturnValue, &input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].severity, Severity::Critical);
        assert_eq!(result[0].location, FindingLocation::Output { index: 0 });
        assert!(result[0].message.contains(&sats.to_string()));
        input.outputs[0].transaction_output.script_type = ScriptType::P2wpkh;
        assert!(evaluate(&NonZeroOpReturnValue, &input).is_empty());
    }
}

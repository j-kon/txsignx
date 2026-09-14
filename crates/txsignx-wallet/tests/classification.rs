mod common;
use txsignx_core::psbt::PsbtUtxoStatus;
use txsignx_wallet::*;
fn inspection() -> txsignx_core::PsbtReport {
    let mut r =
        txsignx_core::analyze_psbt(include_str!("../../../fixtures/psbt-unsigned.b64")).unwrap();
    r.inputs[0].utxo.script_pubkey_hex = Some(common::script(0, 7).to_hex_string());
    r.outputs[1].transaction_output.script_pubkey_hex = common::script(1, 3).to_hex_string();
    r
}
#[test]
fn classifies_inputs_outputs_and_sorted_explicit_change() {
    let index = WalletIndex::new(common::config(10)).unwrap();
    let report = index.classify(&inspection(), &[1, 0]).unwrap();
    assert_eq!(
        report.inputs()[0].ownership,
        WalletOwnership::External {
            derivation_index: 7
        }
    );
    assert_eq!(
        report.outputs()[1].ownership,
        WalletOwnership::Internal {
            derivation_index: 3
        }
    );
    assert!(report.outputs()[1].expected_change);
    assert_eq!(report.expected_change_outputs(), [0, 1]);
}
#[test]
fn missing_invalid_context_is_unavailable_even_if_script_is_present() {
    let index = WalletIndex::new(common::config(10)).unwrap();
    for (status, reason) in [
        (
            PsbtUtxoStatus::Missing,
            WalletContextUnavailableReason::MissingPrevoutContext,
        ),
        (
            PsbtUtxoStatus::TxidMismatch,
            WalletContextUnavailableReason::InvalidPrevoutContext,
        ),
    ] {
        let mut r = inspection();
        r.inputs[0].utxo.status = status;
        let report = index.classify(&r, &[]).unwrap();
        assert_eq!(
            report.inputs()[0].ownership,
            WalletOwnership::Unavailable { reason }
        );
    }
}
#[test]
fn rejects_bad_change_intent() {
    let index = WalletIndex::new(common::config(1)).unwrap();
    assert_eq!(
        index.classify(&inspection(), &[0, 1, 2]).err(),
        Some(WalletError::InvalidExpectedChange)
    );
    assert_eq!(
        index.classify(&inspection(), &[1, 1]).err(),
        Some(WalletError::DuplicateExpectedChange)
    );
    assert_eq!(
        index.classify(&inspection(), &[2]).err(),
        Some(WalletError::InvalidExpectedChange)
    );
}
#[test]
fn rejects_cross_inspection_context_reuse() {
    let index = WalletIndex::new(common::config(10)).unwrap();
    let mut r = inspection();
    let context = index.classify(&r, &[1]).unwrap();
    assert!(context.validate_for(&r).is_ok());
    r.inputs[0].utxo.script_pubkey_hex = Some(common::script(0, 8).to_hex_string());
    assert_eq!(context.validate_for(&r), Err(WalletError::ContextMismatch));
}
#[test]
fn malformed_valid_script_and_inconsistent_indexes_are_errors() {
    let index = WalletIndex::new(common::config(1)).unwrap();
    let mut r = inspection();
    r.inputs[0].utxo.script_pubkey_hex = None;
    assert_eq!(
        index.classify(&r, &[]).err(),
        Some(WalletError::InconsistentInspection)
    );
    r.inputs[0].utxo.script_pubkey_hex = Some("not-hex".into());
    assert!(index.classify(&r, &[]).is_err());
    let mut r = inspection();
    r.outputs[0].transaction_output.index = 99;
    assert!(index.classify(&r, &[]).is_err());
}
#[test]
fn reports_are_deterministic_and_descriptor_free() {
    let index = WalletIndex::new(common::config(10)).unwrap();
    let r = inspection();
    let before = serde_json::to_string(&r).unwrap();
    let first = serde_json::to_string(&index.classify(&r, &[1]).unwrap()).unwrap();
    for _ in 0..5 {
        assert_eq!(
            serde_json::to_string(&index.classify(&r, &[1]).unwrap()).unwrap(),
            first
        );
    }
    assert!(!first.contains("tpub") && !first.contains("wpkh(") && !first.contains("binding"));
    assert_eq!(serde_json::to_string(&r).unwrap(), before);
}

mod psbt_common;
use bitcoin::{ScriptBuf, Witness, psbt::raw};
use psbt_common::*;
use txsignx_core::{
    analyze_psbt,
    psbt::{PsbtFeeStatus, PsbtSigningState},
    transaction::{ScriptType, analyze_decoded_transaction},
};

#[test]
fn unsigned_summary_reuses_transaction_facts_and_resolves_fee() {
    let psbt = unsigned();
    let tx = analyze_decoded_transaction(&psbt.unsigned_tx).unwrap();
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(report.psbt_version, 0);
    assert_eq!(report.format, "BIP174");
    assert_eq!(report.unsigned_txid, tx.txid);
    assert_eq!((report.transaction_version, report.locktime), (2, 42));
    assert_eq!((report.input_count, report.output_count), (1, 2));
    assert_eq!(report.total_output_sats, 150_000);
    assert!(report.explicit_rbf && report.inputs[0].explicit_rbf);
    assert_eq!(report.inputs[0].sequence, 0xffff_fffd);
    assert_eq!(report.fee.status, PsbtFeeStatus::Available);
    assert_eq!(report.fee.fee_sats, Some(1000));
    assert_eq!(report.signing_state, PsbtSigningState::Unsigned);
    assert_eq!(report.inputs[0].utxo.script_type, Some(ScriptType::P2wpkh));
    assert_eq!(
        serde_json::to_value(&report.outputs[0].transaction_output).unwrap(),
        serde_json::to_value(&tx.outputs[0]).unwrap()
    );
}
#[test]
fn ecdsa_presence_and_explicit_sighash_are_structural() {
    let report = analyze_psbt(&encode(&partial())).unwrap();
    assert_eq!(report.signing_state, PsbtSigningState::PartiallySigned);
    assert_eq!(report.inputs[0].partial_ecdsa_signature_count, 1);
    assert_eq!(report.inputs[0].sighash_type.as_ref().unwrap().value, 1);
    assert_eq!(
        report.inputs[0].sighash_type.as_ref().unwrap().name,
        "SIGHASH_ALL"
    );
    assert!(
        analyze_psbt(&encode(&unsigned())).unwrap().inputs[0]
            .sighash_type
            .is_none()
    );
}
#[test]
fn final_markers_and_mixed_inputs_survive_roundtrip() {
    let mut psbt = partial();
    psbt.inputs[0].final_script_sig = Some(ScriptBuf::new());
    psbt.inputs[0].final_script_witness = Some(Witness::new());
    assert_eq!(
        analyze_psbt(&encode(&psbt)).unwrap().signing_state,
        PsbtSigningState::Finalized
    );
    psbt.unsigned_tx
        .input
        .push(psbt.unsigned_tx.input[0].clone());
    psbt.inputs.push(bitcoin::psbt::Input::default());
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(report.signing_state, PsbtSigningState::Mixed);
    assert!(
        report.inputs[0].final_script_sig_present && report.inputs[0].final_script_witness_present
    );
    assert_eq!(report.fee.status, PsbtFeeStatus::MissingUtxoContext);
}
#[test]
fn taproot_signatures_and_origins_are_counted_without_keys() {
    let mut psbt = unsigned();
    let xonly = public_key().inner.x_only_public_key().0;
    let signature = bitcoin::taproot::Signature::from_slice(&[1; 64]).unwrap();
    let leaf = bitcoin::taproot::TapLeafHash::from_script(
        &ScriptBuf::new(),
        bitcoin::taproot::LeafVersion::TapScript,
    );
    let origin = (
        bitcoin::bip32::Fingerprint::from([1; 4]),
        bitcoin::bip32::DerivationPath::from(vec![
            bitcoin::bip32::ChildNumber::from_normal_idx(7).unwrap(),
        ]),
    );
    psbt.inputs[0]
        .tap_script_sigs
        .insert((xonly, leaf), signature);
    psbt.inputs[0]
        .tap_key_origins
        .insert(xonly, (vec![leaf], origin.clone()));
    psbt.inputs[0]
        .bip32_derivation
        .insert(public_key().inner, origin.clone());
    psbt.outputs[0]
        .tap_key_origins
        .insert(xonly, (vec![], origin.clone()));
    psbt.outputs[0]
        .bip32_derivation
        .insert(public_key().inner, origin);
    psbt.outputs[0].tap_internal_key = Some(xonly);
    psbt.outputs[0].tap_tree = Some(
        bitcoin::taproot::TaprootBuilder::new()
            .add_leaf(0, ScriptBuf::from_bytes(vec![0x51]))
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(report.signing_state, PsbtSigningState::PartiallySigned);
    assert_eq!(report.inputs[0].tap_script_signature_count, 1);
    assert_eq!(report.inputs[0].tap_key_origin_count, 1);
    assert_eq!(report.inputs[0].bip32_derivation_count, 1);
    assert_eq!(report.outputs[0].tap_key_origin_count, 1);
    assert_eq!(report.outputs[0].bip32_derivation_count, 1);
    assert!(report.outputs[0].tap_internal_key_present && report.outputs[0].tap_tree_present);
    psbt.inputs[0].tap_script_sigs.clear();
    psbt.inputs[0].tap_key_sig = Some(signature);
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert!(report.inputs[0].tap_key_signature_present);
    assert_eq!(report.signing_state, PsbtSigningState::PartiallySigned);
    assert!(
        !serde_json::to_string(&report)
            .unwrap()
            .contains(&xonly.to_string())
    );
}
#[test]
fn unknown_proprietary_and_scripts_are_preserved_but_not_dumped() {
    let mut psbt = unsigned();
    let key = raw::Key {
        type_value: 0xee,
        key: b"PRIVATE_METADATA_SENTINEL".to_vec(),
    };
    let prop = raw::ProprietaryKey {
        prefix: b"PRIVATE_METADATA_SENTINEL".to_vec(),
        subtype: 0,
        key: vec![],
    };
    let value = b"\x1b[31mPRIVATE_METADATA_SENTINEL".to_vec();
    psbt.unknown.insert(key.clone(), value.clone());
    psbt.proprietary.insert(prop.clone(), value.clone());
    psbt.inputs[0].unknown.insert(key.clone(), value.clone());
    psbt.inputs[0]
        .proprietary
        .insert(prop.clone(), value.clone());
    psbt.outputs[0].unknown.insert(key, value.clone());
    psbt.outputs[0].proprietary.insert(prop, value.clone());
    psbt.inputs[0].redeem_script = Some(ScriptBuf::from_bytes(value.clone()));
    psbt.inputs[0].witness_script = Some(ScriptBuf::from_bytes(value.clone()));
    psbt.outputs[0].redeem_script = Some(ScriptBuf::from_bytes(value.clone()));
    psbt.outputs[0].witness_script = Some(ScriptBuf::from_bytes(value));
    let text = encode(&psbt);
    assert_eq!(txsignx_core::psbt::decode_psbt(&text).unwrap(), psbt);
    let report = analyze_psbt(&text).unwrap();
    assert_eq!((report.unknown_count, report.proprietary_count), (1, 1));
    assert_eq!(
        (
            report.inputs[0].unknown_count,
            report.inputs[0].proprietary_count
        ),
        (1, 1)
    );
    assert_eq!(
        (
            report.outputs[0].unknown_count,
            report.outputs[0].proprietary_count
        ),
        (1, 1)
    );
    assert!(report.inputs[0].redeem_script_present && report.inputs[0].witness_script_present);
    assert!(report.outputs[0].redeem_script_present && report.outputs[0].witness_script_present);
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("PRIVATE_METADATA_SENTINEL"));
    assert!(!json.contains(&text));
    for field in [
        "network",
        "address",
        "feerate",
        "ownership",
        "signature_valid",
    ] {
        assert!(
            serde_json::from_str::<serde_json::Value>(&json)
                .unwrap()
                .get(field)
                .is_none()
        );
    }
}

#[test]
fn synthetic_global_xpub_is_counted_without_exposing_key_or_network() {
    use bitcoin::bip32::{ChainCode, ChildNumber, DerivationPath, Fingerprint, Xpub};
    let mut psbt = unsigned();
    let xpub = Xpub {
        network: bitcoin::NetworkKind::Test,
        depth: 0,
        parent_fingerprint: Fingerprint::default(),
        child_number: ChildNumber::from_normal_idx(0).unwrap(),
        public_key: public_key().inner,
        chain_code: ChainCode::from([2; 32]),
    };
    psbt.xpub
        .insert(xpub, (Fingerprint::from([1; 4]), DerivationPath::default()));
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(report.global_xpub_count, 1);
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains(&xpub.to_string()));
    assert!(!json.contains("testnet") && !json.contains("network"));
}
#[test]
fn checked_in_fixtures_match_documented_builders() {
    assert_eq!(
        encode(&unsigned()),
        include_str!("../../../fixtures/psbt-unsigned.b64").trim()
    );
    assert_eq!(
        encode(&partial()),
        include_str!("../../../fixtures/psbt-partial.b64").trim()
    );
}
#[test]
fn nonstandard_explicit_sighash_is_preserved_without_guessing() {
    let mut psbt = unsigned();
    psbt.inputs[0].sighash_type = Some(bitcoin::psbt::PsbtSighashType::from_u32(0xdeadbeef));
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(
        report.inputs[0].sighash_type.as_ref().unwrap().value,
        0xdeadbeef
    );
    assert_eq!(
        report.inputs[0].sighash_type.as_ref().unwrap().name,
        "0xdeadbeef"
    );
}

#[test]
fn serialized_nonwitness_context_checks_txid_vout_and_both_forms() {
    use txsignx_core::psbt::{PsbtUtxoSource, PsbtUtxoStatus};
    let mut psbt = unsigned();
    let mut previous = psbt.unsigned_tx.clone();
    previous.output[1].value = bitcoin::Amount::from_sat(151_000);
    psbt.unsigned_tx.input[0].previous_output.txid = previous.compute_txid();
    psbt.inputs[0].non_witness_utxo = Some(previous.clone());
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(report.inputs[0].utxo.source, PsbtUtxoSource::Both);
    assert_eq!(report.inputs[0].utxo.status, PsbtUtxoStatus::Valid);
    assert_eq!(report.fee.fee_sats, Some(1000));
    psbt.inputs[0].witness_utxo = None;
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(report.inputs[0].utxo.source, PsbtUtxoSource::NonWitnessUtxo);
    assert_eq!(report.fee.fee_sats, Some(1000));
    psbt.unsigned_tx.input[0].previous_output.vout = u32::MAX;
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(report.inputs[0].utxo.status, PsbtUtxoStatus::VoutOutOfRange);
    assert_eq!(report.fee.status, PsbtFeeStatus::InvalidUtxoContext);
    assert!(report.inputs[0].utxo.value_sats.is_none());
    psbt.unsigned_tx.input[0].previous_output.vout = 1;
    psbt.inputs[0].witness_utxo = Some(previous.output[0].clone());
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(
        report.inputs[0].utxo.status,
        PsbtUtxoStatus::WitnessNonWitnessMismatch
    );
    assert_eq!(report.fee.fee_sats, None);
    previous.lock_time = bitcoin::absolute::LockTime::ZERO;
    psbt.inputs[0].non_witness_utxo = Some(previous);
    let report = analyze_psbt(&encode(&psbt)).unwrap();
    assert_eq!(report.inputs[0].utxo.status, PsbtUtxoStatus::TxidMismatch);
    assert_eq!(report.fee.status, PsbtFeeStatus::InvalidUtxoContext);
}
#[test]
fn independent_unsigned_fixture_txid_is_stable() {
    // Double SHA256 of the 116-byte global unsigned transaction, independently
    // computed using Python hashlib, reversing digest bytes for conventional TXID.
    assert_eq!(
        analyze_psbt(include_str!("../../../fixtures/psbt-unsigned.b64"))
            .unwrap()
            .unsigned_txid,
        "a6375ce044efb3b817642d5b3c584e63e6e5de2b32cbaed1a34b72c1156d6381"
    );
}

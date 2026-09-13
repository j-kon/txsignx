mod common;

use bitcoin::{Amount, Sequence, consensus::encode::serialize_hex};
use txsignx_core::{AnalysisError, analyze_transaction, transaction::ScriptType};

const LEGACY: &str = include_str!("fixtures/legacy.hex");
const SEGWIT: &str = include_str!("fixtures/segwit.hex");
const TXID: &str = "15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6";
const WTXID: &str = "561d35cd60944685cbc9155bb5ea54de63aa4ec39c4ac3f2aa936f127cbeccd1";

#[test]
fn fixed_fixtures_match_deterministically_constructed_transactions() {
    for (hex, segwit) in [(LEGACY, false), (SEGWIT, true)] {
        assert_eq!(
            serialize_hex(&common::fixture_transaction(segwit)),
            hex.trim()
        );
    }
}

#[test]
fn legacy_identifiers_match_independent_hash_vector() {
    let report = analyze_transaction(LEGACY.trim()).unwrap();
    assert_eq!(report.txid, TXID);
    assert_eq!(report.wtxid, TXID);
    assert!(!report.has_witness);
}

#[test]
fn segwit_identifiers_distinguish_witness() {
    let report = analyze_transaction(SEGWIT.trim()).unwrap();
    assert_eq!(report.txid, TXID);
    assert_eq!(report.wtxid, WTXID);
    assert_ne!(report.txid, report.wtxid);
    assert!(report.has_witness);
}

#[test]
fn legacy_metrics_have_four_weight_units_per_byte() {
    let report = analyze_transaction(LEGACY.trim()).unwrap();
    assert_eq!(report.size_bytes, 118);
    assert_eq!(report.weight_wu, 472);
    assert_eq!(report.vsize_vb, 118);
}

#[test]
fn segwit_metrics_discount_witness_and_round_vsize_up() {
    let report = analyze_transaction(SEGWIT.trim()).unwrap();
    assert_eq!(report.size_bytes, 129);
    assert_eq!(report.weight_wu, 483);
    assert_eq!(report.vsize_vb, 121);
}

#[test]
fn reports_summary_and_multiple_output_total() {
    let report = analyze_transaction(LEGACY.trim()).unwrap();
    assert_eq!(report.version, 2);
    assert_eq!(report.locktime, 42);
    assert_eq!(report.input_count, 1);
    assert_eq!(report.output_count, 2);
    assert_eq!(report.total_output_sats, 150_000);
    assert!(report.fee_sats.is_none());
}

#[test]
fn reports_input_outpoint_sequence_and_script() {
    let report = analyze_transaction(LEGACY.trim()).unwrap();
    let input = &report.inputs[0];
    assert_eq!(input.index, 0);
    assert_eq!(input.previous_txid, "11".repeat(32));
    assert_eq!(input.previous_vout, 1);
    assert_eq!(input.sequence, 0xffff_fffd);
    assert_eq!(input.script_sig_hex, "0151");
    assert_eq!(input.script_sig_size_bytes, 2);
    assert!(input.explicit_rbf);
    assert_eq!(input.witness_item_count, 0);
    assert!(input.witness_items.is_empty());
}

#[test]
fn reports_witness_items_in_order_including_empty_item() {
    let report = analyze_transaction(SEGWIT.trim()).unwrap();
    let input = &report.inputs[0];
    assert_eq!(input.witness_item_count, 3);
    let actual: Vec<_> = input
        .witness_items
        .iter()
        .map(|w| (w.index, w.size_bytes, w.hex.as_str()))
        .collect();
    assert_eq!(actual, [(0, 3, "010203"), (1, 0, ""), (2, 2, "abcd")]);
}

#[test]
fn reports_output_values_scripts_and_types() {
    let report = analyze_transaction(LEGACY.trim()).unwrap();
    let output = &report.outputs[0];
    assert_eq!(output.index, 0);
    assert_eq!(output.value_sats, 100_000);
    assert_eq!(
        output.script_pubkey_hex,
        format!("76a914{}88ac", "22".repeat(20))
    );
    assert_eq!(output.script_pubkey_size_bytes, 25);
    assert_eq!(output.script_type, ScriptType::P2pkh);
    let output = &report.outputs[1];
    assert_eq!(output.index, 1);
    assert_eq!(output.value_sats, 50_000);
    assert_eq!(output.script_pubkey_size_bytes, 22);
    assert_eq!(output.script_type, ScriptType::P2wpkh);
}

#[test]
fn any_input_can_explicitly_signal_rbf() {
    let mut tx = common::fixture_transaction(false);
    tx.input[0].sequence = Sequence::MAX;
    tx.input.push(tx.input[0].clone());
    tx.input[1].sequence = Sequence(0xffff_fffd);
    let report = analyze_transaction(&serialize_hex(&tx)).unwrap();
    assert!(report.explicit_rbf);
    assert!(!report.inputs[0].explicit_rbf);
    assert!(report.inputs[1].explicit_rbf);
    assert_eq!(report.inputs[1].index, 1);
}

#[test]
fn no_explicit_rbf_when_all_inputs_are_final_or_locktime_only() {
    let mut tx = common::fixture_transaction(false);
    tx.input[0].sequence = Sequence::MAX;
    tx.input.push(tx.input[0].clone());
    tx.input[1].sequence = Sequence(0xffff_fffe);
    let report = analyze_transaction(&serialize_hex(&tx)).unwrap();
    assert!(!report.explicit_rbf);
}

#[test]
fn rejects_output_total_overflow() {
    let mut tx = common::fixture_transaction(false);
    tx.output[0].value = Amount::from_sat(u64::MAX);
    tx.output[1].value = Amount::from_sat(1);
    assert!(matches!(
        analyze_transaction(&serialize_hex(&tx)),
        Err(AnalysisError::OutputValueOverflow)
    ));
}

#[test]
fn json_uses_primitives_lowercase_hex_and_null_fee_without_network_or_addresses() {
    let report = analyze_transaction(&SEGWIT.trim().to_uppercase()).unwrap();
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["txid"], TXID);
    assert_eq!(json["total_output_sats"].as_u64(), Some(150_000));
    assert_eq!(json["inputs"][0]["sequence"].as_u64(), Some(4_294_967_293));
    assert_eq!(json["inputs"][0]["witness_items"][2]["hex"], "abcd");
    assert_eq!(json["outputs"][0]["script_type"], "p2pkh");
    assert!(json["fee_sats"].is_null());
    assert!(json.get("network").is_none());
    assert!(json["outputs"][0].get("address").is_none());
    let serialized = serde_json::to_string(&report).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&serialized).unwrap(),
        json
    );
    assert_eq!(serde_json::to_string(&report).unwrap(), serialized);
}

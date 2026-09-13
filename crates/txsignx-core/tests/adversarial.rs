mod common;

use bitcoin::{ScriptBuf, Witness, consensus::encode::serialize_hex};
use txsignx_core::{
    AnalysisError, analyze_transaction, limits::MAX_REPORT_WITNESS_ITEMS,
    transaction::decode_transaction,
};

#[test]
fn excessive_empty_witness_items_are_rejected_before_report_expansion() {
    let mut tx = common::fixture_transaction(true);
    tx.input[0].witness =
        Witness::from_slice(&vec![Vec::<u8>::new(); MAX_REPORT_WITNESS_ITEMS + 1]);
    assert!(matches!(
        analyze_transaction(&serialize_hex(&tx)),
        Err(AnalysisError::WitnessItemLimitExceeded { .. })
    ));
}

#[test]
fn witness_item_limit_applies_across_inputs() {
    let mut tx = common::fixture_transaction(true);
    tx.input[0].witness =
        Witness::from_slice(&vec![Vec::<u8>::new(); MAX_REPORT_WITNESS_ITEMS / 2 + 1]);
    tx.input.push(tx.input[0].clone());
    assert!(matches!(
        analyze_transaction(&serialize_hex(&tx)),
        Err(AnalysisError::WitnessItemLimitExceeded { .. })
    ));
}

#[test]
fn accepts_witness_count_at_report_limit() {
    let mut tx = common::fixture_transaction(true);
    tx.input[0].witness = Witness::from_slice(&vec![Vec::<u8>::new(); MAX_REPORT_WITNESS_ITEMS]);
    let report = analyze_transaction(&serialize_hex(&tx)).unwrap();
    assert_eq!(
        report.inputs[0].witness_items.len(),
        MAX_REPORT_WITNESS_ITEMS
    );
}

#[test]
fn accepts_exact_weight_ceiling_and_rejects_next_byte() {
    let mut tx = common::fixture_transaction(false);
    // 118 original bytes - 2 script bytes + 4 extra CompactSize bytes = 120.
    tx.input[0].script_sig = ScriptBuf::from_bytes(vec![0; 999_880]);
    let report = analyze_transaction(&serialize_hex(&tx)).unwrap();
    assert_eq!(report.weight_wu, 4_000_000);
    tx.input[0].script_sig = ScriptBuf::from_bytes(vec![0; 999_881]);
    assert!(matches!(
        analyze_transaction(&serialize_hex(&tx)),
        Err(AnalysisError::WeightExceeded { .. })
    ));
}

#[test]
fn every_truncated_fixture_is_rejected_without_panic() {
    for hex in [
        include_str!("fixtures/legacy.hex").trim(),
        include_str!("fixtures/segwit.hex").trim(),
    ] {
        for length in (0..hex.len()).step_by(2) {
            assert!(
                analyze_transaction(&hex[..length]).is_err(),
                "prefix length {length}"
            );
        }
    }
}

#[test]
fn all_one_and_two_byte_inputs_are_errors_without_panic() {
    for byte in 0..=255 {
        assert!(analyze_transaction(&format!("{byte:02x}")).is_err());
    }
    for bytes in 0..=65535 {
        assert!(analyze_transaction(&format!("{bytes:04x}")).is_err());
    }
}

#[test]
fn deterministic_fuzz_like_short_buffers_are_errors_without_panic() {
    let mut state = 0x1234_5678_u32;
    for length in 3..60 {
        for _ in 0..32 {
            let mut hex = String::new();
            for _ in 0..length {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                hex.push_str(&format!("{:02x}", state & 0xff));
            }
            assert!(analyze_transaction(&hex).is_err(), "length {length}");
        }
    }
}

#[test]
fn impossible_compactsize_counts_and_nonminimal_encodings_are_rejected() {
    for hex in [
        "02000000ffffffffffffffffff", // impossible input count
        "02000000fd0100",             // non-minimal input count
        "0200000001ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00000000ffffffffffffffffff", // impossible script length
    ] {
        assert!(decode_transaction(hex).is_err());
    }
    let witness_hex = include_str!("fixtures/segwit.hex").trim();
    // Witness starts at byte 116; replace its item count with an impossible u64.
    let hex = format!(
        "{}ffffffffffffffffff{}",
        &witness_hex[..232],
        &witness_hex[234..]
    );
    assert!(decode_transaction(&hex).is_err());
}

#[test]
fn witness_presence_is_based_on_stack_items_even_when_empty() {
    let mut tx = common::fixture_transaction(false);
    tx.input[0].witness = Witness::from_slice(&[Vec::<u8>::new()]);
    let report = analyze_transaction(&serialize_hex(&tx)).unwrap();
    assert!(report.has_witness);
    assert_ne!(report.txid, report.wtxid);
    assert_eq!(report.inputs[0].witness_items[0].hex, "");
}

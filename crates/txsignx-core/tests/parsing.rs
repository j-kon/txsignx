use txsignx_core::{
    AnalysisError, limits::MAX_TRANSACTION_HEX_CHARS, transaction::decode_transaction,
};

const LEGACY: &str = include_str!("fixtures/legacy.hex");
const SEGWIT: &str = include_str!("fixtures/segwit.hex");

#[test]
fn accepts_legacy_consensus_transaction() {
    let tx = decode_transaction(LEGACY.trim()).unwrap();
    assert_eq!(tx.input.len(), 1);
    assert_eq!(tx.output.len(), 2);
}

#[test]
fn accepts_segwit_consensus_transaction() {
    let tx = decode_transaction(SEGWIT.trim()).unwrap();
    assert_eq!(tx.input[0].witness.len(), 3);
}

#[test]
fn accepts_uppercase_hex() {
    assert!(decode_transaction(&LEGACY.trim().to_uppercase()).is_ok());
}

#[test]
fn rejects_empty_input() {
    assert!(matches!(
        decode_transaction(""),
        Err(AnalysisError::EmptyInput)
    ));
}

#[test]
fn rejects_odd_length_hex() {
    for hex in ["0", "abc", "00000"] {
        assert!(matches!(
            decode_transaction(hex),
            Err(AnalysisError::OddLength { .. })
        ));
    }
}

#[test]
fn rejects_invalid_hex_without_trimming_or_prefixes() {
    for hex in ["gg", "01zz", "0x00", "  ", "é", "\n\n"] {
        assert!(
            matches!(decode_transaction(hex), Err(AnalysisError::InvalidHex(_))),
            "{hex:?}"
        );
    }
}

#[test]
fn rejects_invalid_transaction_bytes() {
    for hex in ["00", "deadbeef", "0200000001"] {
        assert!(matches!(
            decode_transaction(hex),
            Err(AnalysisError::InvalidTransaction(_))
        ));
    }
}

#[test]
fn rejects_trailing_bytes_and_concatenated_transactions() {
    for suffix in ["00", "deadbeef", LEGACY.trim()] {
        assert!(matches!(
            decode_transaction(&format!("{}{suffix}", LEGACY.trim())),
            Err(AnalysisError::InvalidTransaction(_))
        ));
    }
}

#[test]
fn rejects_oversize_before_hex_decoding() {
    let input = "g".repeat(MAX_TRANSACTION_HEX_CHARS + 2);
    assert!(matches!(
        decode_transaction(&input),
        Err(AnalysisError::InputTooLarge { .. })
    ));
}

#[test]
fn size_limit_is_inclusive() {
    let input = "g".repeat(MAX_TRANSACTION_HEX_CHARS);
    assert!(matches!(
        decode_transaction(&input),
        Err(AnalysisError::InvalidHex(_))
    ));
}

#[test]
fn rejects_weight_above_block_ceiling() {
    use bitcoin::{
        ScriptBuf, Transaction,
        consensus::encode::{deserialize, serialize_hex},
        hex::FromHex,
    };
    let mut tx: Transaction = deserialize(&Vec::from_hex(LEGACY.trim()).unwrap()).unwrap();
    tx.input[0].script_sig = ScriptBuf::from_bytes(vec![0; 1_000_000]);
    assert!(matches!(
        decode_transaction(&serialize_hex(&tx)),
        Err(AnalysisError::WeightExceeded { .. })
    ));
}

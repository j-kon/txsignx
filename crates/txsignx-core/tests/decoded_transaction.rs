use txsignx_core::{
    analyze_transaction,
    transaction::{analyze_decoded_transaction, decode_transaction},
};

#[test]
fn decoded_analysis_preserves_raw_reports() {
    for raw in [
        include_str!("fixtures/legacy.hex"),
        include_str!("fixtures/segwit.hex"),
    ] {
        let raw = raw.trim();
        let decoded = decode_transaction(raw).unwrap();
        assert_eq!(
            serde_json::to_value(analyze_decoded_transaction(&decoded).unwrap()).unwrap(),
            serde_json::to_value(analyze_transaction(raw).unwrap()).unwrap()
        );
    }
}

#[test]
fn decoded_analysis_keeps_weight_safety_limit() {
    let mut decoded = decode_transaction(include_str!("fixtures/legacy.hex").trim()).unwrap();
    decoded.input[0].script_sig = bitcoin::ScriptBuf::from_bytes(vec![0; 1_000_000]);
    assert!(matches!(
        analyze_decoded_transaction(&decoded),
        Err(txsignx_core::AnalysisError::WeightExceeded { .. })
    ));
}

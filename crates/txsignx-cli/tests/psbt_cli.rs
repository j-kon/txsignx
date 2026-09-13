const UNSIGNED: &str = include_str!("../../../fixtures/psbt-unsigned.b64");
const PARTIAL: &str = include_str!("../../../fixtures/psbt-partial.b64");
use std::process::{Command, Output};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn human_unsigned_and_partial_reports() {
    for (psbt, state) in [(UNSIGNED, "Unsigned"), (PARTIAL, "Partially Signed")] {
        let output = run(&["psbt", "inspect", psbt]);
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        for part in [
            "TxSignX PSBT Inspection",
            "0 (BIP174)",
            state,
            "Fee: 1000 sats",
            "P2WPKH",
            "Fee rate: unavailable",
            "Inspection is structural",
        ] {
            assert!(text.contains(part), "{part}: {text}");
        }
    }
}
#[test]
fn json_is_only_stdout_and_uses_primitive_fields() {
    let output = run(&["psbt", "inspect", PARTIAL, "--json"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["signing_state"], "partially_signed");
    assert_eq!(json["fee"]["status"], "available");
    assert_eq!(json["fee"]["fee_sats"], 1000);
}
#[test]
fn malformed_and_argument_errors_never_echo_input() {
    for args in [
        vec!["psbt", "inspect", "PRIVATE_PSBT_SENTINEL", "--json"],
        vec![
            "psbt",
            "inspect",
            "PRIVATE_PSBT_SENTINEL",
            "EXTRA_PRIVATE_PSBT_SENTINEL",
        ],
    ] {
        let output = run(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("error:"));
        assert!(!stderr.contains("PRIVATE_PSBT_SENTINEL"));
    }
}

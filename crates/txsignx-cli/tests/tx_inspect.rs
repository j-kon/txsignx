use std::process::{Command, Output};

const LEGACY: &str = include_str!("../../txsignx-core/tests/fixtures/legacy.hex");
const SEGWIT: &str = include_str!("../../txsignx-core/tests/fixtures/segwit.hex");

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn raw_positional_syntax_preserved() {
    let output = run(&["tx", "inspect", LEGACY.trim()]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("TXID: 15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6")
    );
    assert!(stdout.contains("Size: 118 bytes"));
    assert!(stdout.contains("Fee: unavailable without prevout context"));
    assert!(!stdout.contains("Network:"));
    assert!(!stdout.contains("Address:"));
}

#[test]
fn raw_hex_with_explicit_network_renders_addresses() {
    let output = run(&["tx", "inspect", LEGACY.trim(), "--network", "bitcoin"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Address: 147Us9aEq2PvBC5wobBJw1yEpQEbPKzssA"));
    assert!(stdout.contains("Address: bc1qxvenxvenxvenxvenxvenxvenxvenxven2ymjt8"));

    // Regtest address prefixes
    let output_rt = run(&["tx", "inspect", LEGACY.trim(), "--network", "regtest"]);
    assert!(output_rt.status.success());
    let stdout_rt = String::from_utf8(output_rt.stdout).unwrap();
    assert!(stdout_rt.contains("Address: bcrt1qxvenxvenxvenxvenxvenxvenxvenxvenztev8a"));
    assert!(stdout_rt.contains("Address: midSACfDe3qAxJZZXA9gkwBZgPqJJUpy1w"));
}

#[test]
fn raw_hex_script_disassembly_renders_opcodes_and_push_data() {
    let output = run(&["tx", "inspect", LEGACY.trim()]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("scriptSig asm: PUSHBYTES_1 51"));
    assert!(stdout.contains("scriptPubKey asm: OP_DUP OP_HASH160 PUSHBYTES_20"));
}

#[test]
fn raw_and_txid_simultaneously_rejected() {
    let output = run(&[
        "tx",
        "inspect",
        LEGACY.trim(),
        "--txid",
        "15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("cannot specify both raw transaction hex and --txid"));
}

#[test]
fn missing_both_raw_and_txid_rejected() {
    let output = run(&["tx", "inspect", "--json"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("must specify either raw transaction hex or --txid"));
}

#[test]
fn txid_without_node_configuration_rejected() {
    let output = run(&[
        "tx",
        "inspect",
        "--txid",
        "15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--txid requires explicit --node-url, --cookie-file, and --network"));
}

#[test]
fn node_options_with_raw_hex_rejected() {
    let output = run(&[
        "tx",
        "inspect",
        LEGACY.trim(),
        "--node-url",
        "http://127.0.0.1:8332",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("node options are only supported with --txid"));
}

#[test]
fn json_remains_pure_with_network_addresses_and_disassembly() {
    let output = run(&[
        "tx",
        "inspect",
        LEGACY.trim(),
        "--network",
        "bitcoin",
        "--json",
    ]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json["txid"],
        "15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6"
    );
    assert_eq!(
        json["outputs"][0]["address"],
        "147Us9aEq2PvBC5wobBJw1yEpQEbPKzssA"
    );
    assert!(
        json["outputs"][0]["script_pubkey_asm"]
            .as_str()
            .unwrap()
            .contains("OP_DUP OP_HASH160")
    );
    assert!(
        json["inputs"][0]["script_sig_asm"]
            .as_str()
            .unwrap()
            .contains("PUSHBYTES_1 51")
    );
    assert!(json["fee_sats"].is_null());
}

#[test]
fn segwit_raw_report_preserves_discounted_metrics() {
    let output = run(&["tx", "inspect", SEGWIT.trim()]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Size: 129 bytes"));
    assert!(stdout.contains("Weight: 483 WU"));
    assert!(stdout.contains("Virtual size: 121 vB"));
    assert!(stdout.contains("Witness: yes"));
    assert!(stdout.contains("SegWit: yes"));
}

#[test]
fn malformed_hex_fails_without_secret_leakage() {
    let secret = "sensitive_data_not_to_leak";
    let output = run(&["tx", "inspect", secret]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains(secret));
}

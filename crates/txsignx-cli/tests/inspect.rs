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
fn help_exposes_transaction_inspect_command() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("tx"));
    assert!(!text.contains('\u{1b}'));
    let output = run(&["tx", "inspect", "--help"]);
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("RAW_TX_HEX")
    );
}

#[test]
fn version_reports_binary_name_and_package_version() {
    let output = run(&["--version"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        concat!("txsignx ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn human_legacy_report_has_metrics_inputs_outputs_and_context_limits() {
    let output = run(&["tx", "inspect", LEGACY.trim()]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "TxSignX Transaction Analysis",
        "15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6",
        "118 bytes",
        "472 WU",
        "118 vB",
        "150000 sats",
        "Explicit RBF signaling: yes",
        "Fee: unavailable without prevout context",
        "P2PKH",
        "P2WPKH",
        "Inputs",
        "Outputs",
        "scriptSig (2 bytes): 0151",
        "Witness: no",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
    assert!(!text.contains('\u{1b}'));
    assert!(!text.contains("Network:"));
    assert!(!text.contains("Address:"));
}

#[test]
fn human_segwit_report_shows_witness_items_and_discounted_metrics() {
    let output = run(&["tx", "inspect", SEGWIT.trim()]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "Witness: yes",
        "129 bytes",
        "483 WU",
        "121 vB",
        "Witness items: 3",
        "010203",
        "abcd",
        "1 (0 bytes):",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
}

#[test]
fn malformed_inputs_fail_on_stderr_without_echoing_input() {
    for hex in [
        "",
        "z-not-a-transaction",
        "abc",
        "0000",
        "0xdeadbeef",
        "this-is-sensitive-do-not-echo",
    ] {
        let output = run(&["tx", "inspect", hex]);
        assert!(!output.status.success(), "{hex}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("error:"));
        if hex.len() > 8 {
            assert!(!stderr.contains(hex));
        }
        assert!(!stderr.contains("panicked"));
    }
}

#[test]
fn missing_transaction_argument_fails() {
    let output = run(&["tx", "inspect"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

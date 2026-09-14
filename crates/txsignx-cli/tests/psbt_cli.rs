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
    let raw_error = run(&["tx", "inspect"]);
    assert!(!raw_error.status.success());
    assert!(
        !String::from_utf8(raw_error.stderr)
            .unwrap()
            .contains("PSBT")
    );
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

fn stdin(bytes: &[u8], json: bool) -> Output {
    use std::{io::Write, process::Stdio};
    let mut command = Command::new(env!("CARGO_BIN_EXE_txsignx"));
    command.args(["psbt", "inspect", "--stdin"]);
    if json {
        command.arg("--json");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // An over-limit reader may stop before the writer completes; broken pipe is expected.
    let _ = child.stdin.take().unwrap().write_all(bytes);
    child.wait_with_output().unwrap()
}
#[test]
fn file_and_stdin_match_positional_json() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/psbt-unsigned.b64"
    );
    let expected = run(&["psbt", "inspect", UNSIGNED, "--json"]);
    for output in [
        run(&["psbt", "inspect", "--file", path, "--json"]),
        stdin(UNSIGNED.as_bytes(), true),
    ] {
        assert!(output.status.success(), "{:?}", output);
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.stdout);
    }
    assert!(stdin(PARTIAL.as_bytes(), false).status.success());
    assert!(run(&["psbt", "inspect", "--file", path]).status.success());
}
#[test]
fn sources_are_required_and_mutually_exclusive_without_echo() {
    for args in [
        vec!["psbt", "inspect"],
        vec!["psbt", "inspect", "PRIVATE_SENTINEL", "--stdin"],
        vec![
            "psbt",
            "inspect",
            "PRIVATE_SENTINEL",
            "--file",
            "PRIVATE_SENTINEL",
        ],
        vec!["psbt", "inspect", "--file", "PRIVATE_SENTINEL", "--stdin"],
    ] {
        let output = run(&args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            !String::from_utf8(output.stderr)
                .unwrap()
                .contains("PRIVATE_SENTINEL")
        );
    }
}
#[test]
fn bounded_stdin_rejects_excess_binary_and_empty_input() {
    for bytes in [
        vec![],
        vec![0xff, 0xfe],
        vec![b'A'; txsignx_core::limits::MAX_PSBT_TEXT_BYTES + 1],
    ] {
        let output = stdin(&bytes, true);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("error:"));
    }
}
#[test]
fn missing_file_reports_safe_error() {
    let output = run(&[
        "psbt",
        "inspect",
        "--file",
        "/nonexistent/PRIVATE_SENTINEL",
        "--json",
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        !String::from_utf8(output.stderr)
            .unwrap()
            .contains("PRIVATE_SENTINEL")
    );
}

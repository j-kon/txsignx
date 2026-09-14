use std::{
    io::Write,
    process::{Command, Output, Stdio},
};
const PASS: &str = include_str!("../../../fixtures/policy/pass.b64");
const BLOCK: &str = include_str!("../../../fixtures/policy/800k-fee.b64");
const REVIEW: &str = include_str!("../../../fixtures/policy/missing-utxo.b64");
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args(args)
        .output()
        .unwrap()
}
fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn policy_list_human_and_json_come_from_registry() {
    let out = run(&["policy", "list"]);
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.contains("TG002")
            && text.contains("CRITICAL")
            && text.contains("TG014")
            && text.contains("DEFERRED")
    );
    let out = run(&["policy", "list", "--json"]);
    assert!(out.status.success());
    let catalog = json(&out);
    assert_eq!(catalog["active_rules"].as_array().unwrap().len(), 8);
    assert_eq!(catalog["deferred_rules"].as_array().unwrap().len(), 6);
}
#[test]
fn pass_positional_human_reports_scope_and_development_thresholds() {
    let out = run(&["psbt", "preflight", PASS]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    let text = String::from_utf8(out.stdout).unwrap();
    for expected in [
        "TxSignX PSBT Preflight",
        "Policy decision: PASS",
        "Risk level: LOW",
        "100000 sats",
        "10.00%",
        "scope is incomplete",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    assert!(!text.contains("safe to sign"));
}
#[test]
fn json_is_complete_before_each_decision_exit_code() {
    for (input, decision, code) in [
        (PASS, "pass", 0),
        (REVIEW, "review", 2),
        (BLOCK, "block", 3),
    ] {
        let out = run(&["psbt", "preflight", input, "--json"]);
        assert_eq!(out.status.code(), Some(code));
        assert!(out.stderr.is_empty());
        let report = json(&out);
        assert_eq!(report["policy"]["decision"], decision);
        assert!(report["inspection"].is_object());
        assert!(report["policy"]["config"].is_object());
    }
}
#[test]
fn file_and_stdin_match_positional_preflight() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/policy/pass.b64"
    );
    let expected = run(&["psbt", "preflight", PASS, "--json"]);
    let file = run(&["psbt", "preflight", "--file", path, "--json"]);
    assert_eq!(file.status.code(), Some(0));
    assert_eq!(file.stdout, expected.stdout);
    let mut child = Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args(["psbt", "preflight", "--stdin", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(PASS.as_bytes())
        .unwrap();
    let stdin = child.wait_with_output().unwrap();
    assert_eq!(stdin.status.code(), Some(0));
    assert_eq!(stdin.stdout, expected.stdout);
    assert!(stdin.stderr.is_empty());
}
#[test]
fn absolute_override_has_exact_boundary_and_is_reported() {
    for (threshold, decision) in [("1000", "pass"), ("999", "block")] {
        let out = run(&[
            "psbt",
            "preflight",
            PASS,
            "--max-absolute-fee-sats",
            threshold,
            "--json",
        ]);
        let report = json(&out);
        assert_eq!(report["policy"]["decision"], decision);
        assert_eq!(
            report["policy"]["config"]["max_absolute_fee_sats"],
            threshold.parse::<u64>().unwrap()
        );
        if decision == "block" {
            assert_eq!(report["policy"]["findings"][0]["code"], "TG002");
        }
    }
}
#[test]
fn ratio_override_including_zero_changes_policy_deterministically() {
    for (threshold, decision) in [
        ("0", "block"),
        ("66", "block"),
        ("67", "pass"),
        ("10000", "pass"),
    ] {
        let out = run(&[
            "psbt",
            "preflight",
            PASS,
            "--max-fee-ratio-bps",
            threshold,
            "--json",
        ]);
        let report = json(&out);
        assert_eq!(report["policy"]["decision"], decision);
        assert_eq!(
            report["policy"]["config"]["max_fee_ratio_bps"],
            threshold.parse::<u16>().unwrap()
        );
    }
}
#[test]
fn invalid_config_input_and_sources_use_error_exit_without_json_or_echo() {
    for args in [
        vec![
            "psbt",
            "preflight",
            PASS,
            "--max-fee-ratio-bps",
            "10001",
            "--json",
        ],
        vec![
            "psbt",
            "preflight",
            PASS,
            "--max-fee-ratio-bps",
            "65536",
            "--json",
        ],
        vec!["psbt", "preflight", "PRIVATE_SENTINEL", "--json"],
        vec!["psbt", "preflight", "PRIVATE_SENTINEL", "--stdin"],
        vec!["psbt", "preflight", "--file", "PRIVATE_SENTINEL", "--stdin"],
        vec!["psbt", "preflight"],
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(stderr.contains("error:"));
        assert!(!stderr.contains("PRIVATE_SENTINEL"));
        assert!(!stderr.contains(PASS.trim()));
    }
}
#[test]
fn negative_fee_preflight_fails_without_a_pass_report() {
    let input = include_str!("../../../fixtures/policy/negative-fee.b64");
    let out = run(&["psbt", "preflight", input, "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("fee calculation failed")
    );
}
#[test]
fn capstone_human_output_has_both_critical_rules() {
    let out = run(&["psbt", "preflight", BLOCK]);
    assert_eq!(out.status.code(), Some(3));
    let text = String::from_utf8(out.stdout).unwrap();
    for expected in [
        "Policy decision: BLOCK",
        "Risk level: CRITICAL",
        "[CRITICAL] TG002",
        "[CRITICAL] TG003",
        "800000 sats",
        "900000 sats total input value",
    ] {
        assert!(text.contains(expected), "{text}");
    }
}
#[test]
fn existing_inspect_remains_successful_for_policy_blocking_input() {
    let out = run(&["psbt", "inspect", BLOCK, "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let report = json(&out);
    assert_eq!(report["fee"]["fee_sats"], 800000);
    assert!(report.get("policy").is_none());
}

#[test]
fn preflight_reuses_bounded_stdin_and_rejects_binary() {
    for bytes in [
        vec![0xff],
        vec![b'A'; txsignx_core::limits::MAX_PSBT_TEXT_BYTES + 1],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_txsignx"))
            .args(["psbt", "preflight", "--stdin", "--json"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let _ = child.stdin.take().unwrap().write_all(&bytes);
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
    }
}
#[test]
fn privacy_extension_fixture_does_not_emit_arbitrary_terminal_text() {
    let input = include_str!("../../../fixtures/policy/extension-metadata.b64");
    for mode in [vec![], vec!["--json"]] {
        let mut args = vec!["psbt", "preflight", input];
        args.extend(mode);
        let out = run(&args);
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("TG012"));
        assert!(
            !text.contains("PRIVATE_SENTINEL")
                && !text.contains("PUBLIC_DUMMY_KEY")
                && !text.contains('\x1b')
        );
        assert!(!text.contains(input.trim()));
    }
}

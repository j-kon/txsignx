use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};
fn fixture(name: &str) -> String {
    std::fs::read_to_string(path(name)).unwrap()
}
fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/wallet")
        .join(name)
}
fn args(name: &str) -> Vec<String> {
    vec![
        "psbt".into(),
        "preflight".into(),
        fixture(name),
        "--external-descriptor-file".into(),
        path("external.desc").to_str().unwrap().into(),
        "--internal-descriptor-file".into(),
        path("internal.desc").to_str().unwrap().into(),
        "--network".into(),
        "regtest".into(),
        "--derivation-window".into(),
        "10".into(),
        "--expected-change-output".into(),
        "1".into(),
    ]
}
fn run(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args(args)
        .output()
        .unwrap()
}
fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn wallet_payment_human_and_json_do_not_expose_descriptors() {
    for json_mode in [false, true] {
        let mut a = args("payment.b64");
        if json_mode {
            a.push("--json".into());
        }
        let out = run(&a);
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stderr.is_empty());
        let text = String::from_utf8(out.stdout.clone()).unwrap();
        for name in ["external.desc", "internal.desc"] {
            assert!(!text.contains(fixture(name).trim()));
        }
        assert!(!text.contains("tpub"));
        if json_mode {
            let r = json(&out);
            assert_eq!(r["wallet_context"]["configured_network"], "regtest");
            assert_eq!(r["policy"]["decision"], "pass");
        } else {
            assert!(text.contains("Configured network: regtest"));
            assert!(text.contains("expected change"));
        }
    }
}
#[test]
fn wallet_decisions_have_complete_json_before_expected_exit() {
    for (name, code, rule, severity) in [
        ("foreign-input.b64", 2, "TG004", "high"),
        ("external-change.b64", 2, "TG005", "high"),
        ("change-hijack.b64", 3, "TG005", "critical"),
        ("collaborative.b64", 2, "TG004", "high"),
    ] {
        let mut a = args(name);
        a.push("--json".into());
        let out = run(&a);
        assert_eq!(out.status.code(), Some(code));
        assert!(
            json(&out)["policy"]["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["code"] == rule && f["severity"] == severity)
        );
    }
}
#[test]
fn descriptor_file_and_direct_inputs_match() {
    let mut a = args("payment.b64");
    a.push("--json".into());
    let file = run(&a);
    a[3] = "--external-descriptor".into();
    a[4] = fixture("external.desc");
    a[5] = "--internal-descriptor".into();
    a[6] = fixture("internal.desc");
    let direct = run(&a);
    assert_eq!(direct.status.code(), Some(0));
    assert_eq!(direct.stdout, file.stdout);
}
#[test]
fn wallet_psbt_file_and_stdin_match_positional() {
    let mut a = args("payment.b64");
    a.push("--json".into());
    let positional = run(&a);
    a.splice(
        2..3,
        [
            "--file".into(),
            path("payment.b64").to_str().unwrap().into(),
        ],
    );
    let file = run(&a);
    assert_eq!(file.status.code(), Some(0));
    assert_eq!(file.stdout, positional.stdout);
    a.splice(2..4, ["--stdin".into()]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args(a)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(fixture("payment.b64").as_bytes())
        .unwrap();
    let stdin = child.wait_with_output().unwrap();
    assert_eq!(stdin.status.code(), Some(0));
    assert_eq!(stdin.stdout, positional.stdout);
}
struct Temp {
    path: PathBuf,
}
impl Temp {
    fn new(bytes: &[u8]) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "txsignx-m4-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        Self { path }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
fn fails_safely(a: &[String]) {
    let out = run(a);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("error:"));
    for token in [
        "TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO",
        "tpub",
        "xpub",
        "wpkh(",
    ] {
        assert!(!stderr.contains(token));
    }
}
#[test]
fn any_wallet_option_requires_complete_configuration() {
    for pair in [
        ("--network", "regtest"),
        ("--derivation-window", "1"),
        ("--expected-change-output", "1"),
        (
            "--external-descriptor",
            "TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO",
        ),
    ] {
        fails_safely(&[
            "psbt".into(),
            "preflight".into(),
            fixture("payment.b64"),
            pair.0.into(),
            pair.1.into(),
        ]);
    }
    for range in [3..5, 5..7, 7..9] {
        let mut a = args("payment.b64");
        a.drain(range);
        fails_safely(&a);
    }
}
#[test]
fn same_keychain_direct_file_conflicts_are_sanitized() {
    for flag in ["--external-descriptor", "--internal-descriptor"] {
        let mut a = args("payment.b64");
        a.extend([flag.into(), "TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO".into()]);
        fails_safely(&a);
    }
}
#[test]
fn direct_descriptor_errors_do_not_echo_marker() {
    for pos in [3, 5] {
        let mut a = args("payment.b64");
        a[pos] = a[pos].replace("-file", "");
        a[pos + 1] = "TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO".into();
        fails_safely(&a);
    }
}
#[test]
fn file_descriptor_errors_do_not_echo_marker() {
    let file = Temp::new(b"TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO");
    for pos in [4, 6] {
        let mut a = args("payment.b64");
        a[pos] = file.path.to_str().unwrap().into();
        fails_safely(&a);
    }
}
#[test]
fn bounded_file_reader_rejects_oversize_and_binary() {
    for bytes in [vec![b'A'; 65537], vec![0xff]] {
        let file = Temp::new(&bytes);
        let mut a = args("payment.b64");
        a[4] = file.path.to_str().unwrap().into();
        fails_safely(&a);
    }
}
#[test]
fn descriptor_file_accepts_exact_limit_and_surrounding_whitespace() {
    let mut bytes = fixture("external.desc").into_bytes();
    bytes.resize(65536, b' ');
    let file = Temp::new(&bytes);
    let mut a = args("payment.b64");
    a[4] = file.path.to_str().unwrap().into();
    assert_eq!(run(&a).status.code(), Some(0));
}
#[test]
fn missing_descriptor_file_has_sanitized_error() {
    let mut a = args("payment.b64");
    a[4] = "/TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO/absent".into();
    fails_safely(&a);
}
#[test]
fn network_and_window_errors_are_runtime_or_argument_exit_one() {
    for (pos, value) in [
        (8, "TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO"),
        (10, "0"),
        (10, "10001"),
        (10, "4294967296"),
    ] {
        let mut a = args("payment.b64");
        a[pos] = value.into();
        fails_safely(&a);
    }
}
#[test]
fn window_min_default_and_max_are_supported() {
    for window in [Some("1"), None, Some("10000")] {
        let mut a = args("index-zero.b64");
        if let Some(window) = window {
            a[10] = window.into();
        } else {
            a.drain(9..11);
        }
        a.push("--json".into());
        let out = run(&a);
        assert_eq!(out.status.code(), Some(0));
        assert_eq!(
            json(&out)["wallet_context"]["derivation_window"],
            window.unwrap_or("1000").parse::<u32>().unwrap()
        );
    }
}
#[test]
fn change_indexes_reject_duplicates_and_out_of_range() {
    let mut a = args("payment.b64");
    a[12] = "2".into();
    fails_safely(&a);
    a[12] = "1".into();
    a.extend(["--expected-change-output".into(), "1".into()]);
    fails_safely(&a);
}
#[test]
fn skipped_change_is_visible_in_json_and_human() {
    let mut a = args("payment.b64");
    a.drain(11..13);
    let human = run(&a);
    assert_eq!(human.status.code(), Some(0));
    assert!(
        String::from_utf8(human.stdout)
            .unwrap()
            .contains("NoExpectedChangeOutput")
    );
    a.push("--json".into());
    let out = run(&a);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        json(&out)["policy"]["rule_evaluations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["code"] == "TG005" && e["status"] == "not_evaluated")
    );
}
#[test]
fn missing_invalid_wallet_inputs_keep_core_findings_only() {
    for (name, code, rule, reason) in [
        ("missing-utxo.b64", 2, "TG010", "missing_prevout_context"),
        ("invalid-utxo.b64", 3, "TG009", "invalid_prevout_context"),
    ] {
        let mut a = args(name);
        a.push("--json".into());
        let out = run(&a);
        assert_eq!(out.status.code(), Some(code));
        let r = json(&out);
        assert_eq!(
            r["wallet_context"]["inputs"][0]["ownership"]["reason"],
            reason
        );
        assert!(
            r["policy"]["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["code"] == rule)
        );
        assert!(
            !r["policy"]["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["code"] == "TG004")
        );
    }
}
#[test]
fn no_wallet_context_omits_optional_report_and_keeps_m3_decisions() {
    for (fixture_name, code) in [
        ("pass", 0),
        ("800k-fee", 3),
        ("missing-utxo", 2),
        ("invalid-utxo", 3),
        ("unusual-sighash", 2),
        ("op-return", 3),
        ("unknown-script", 2),
        ("extension-metadata", 0),
    ] {
        let text = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../fixtures/policy/{fixture_name}.b64")),
        )
        .unwrap();
        let out = run(&["psbt".into(), "preflight".into(), text, "--json".into()]);
        assert_eq!(out.status.code(), Some(code));
        assert!(json(&out).get("wallet_context").is_none());
    }
}
#[test]
fn hardened_public_descriptor_suffixes_fail_safely_in_cli() {
    for suffix in ["/0h/*", "/0/*h"] {
        let mut a = args("payment.b64");
        a[3] = "--external-descriptor".into();
        a[4] = fixture("external.desc").replace("/0/*", suffix);
        fails_safely(&a);
    }
}

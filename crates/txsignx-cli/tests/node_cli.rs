use std::process::Command;
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn rpc_configuration_errors_are_sanitized_and_leave_stdout_empty() {
    for args in [
        vec!["--rpc-url", "http://127.0.0.1:28443"],
        vec!["--rpc-cookie-file", "SECRET_MARKER"],
        vec![
            "--rpc-url",
            "http://user:SECRET_MARKER@remote:80",
            "--rpc-cookie-file",
            "SECRET_MARKER",
            "--network",
            "regtest",
        ],
        vec![
            "--rpc-url",
            "http://127.0.0.1:28443",
            "--rpc-cookie-file",
            "SECRET_MARKER",
            "--network",
            "regtest",
        ],
    ] {
        let mut a = vec![
            "psbt",
            "preflight",
            include_str!("../../../fixtures/policy/pass.b64"),
        ];
        a.extend(args);
        let o = run(&a);
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stdout.is_empty());
        assert!(
            !String::from_utf8(o.stderr)
                .unwrap()
                .contains("SECRET_MARKER")
        );
    }
}
#[test]
fn preflight_help_exposes_cookie_only_node_configuration() {
    let o = run(&["psbt", "preflight", "--help"]);
    let s = String::from_utf8(o.stdout).unwrap();
    assert!(s.contains("--rpc-url"));
    assert!(s.contains("--rpc-cookie-file"));
    assert!(!s.contains("--rpc-password"));
}
#[test]
fn no_node_json_omits_node_context() {
    let o = run(&[
        "psbt",
        "preflight",
        include_str!("../../../fixtures/policy/pass.b64"),
        "--json",
    ]);
    assert_eq!(o.status.code(), Some(0));
    let r: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(r.get("node_context").is_none());
    assert_eq!(r["policy"]["decision"], "pass");
}

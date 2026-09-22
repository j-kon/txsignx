use std::process::Command;
#[test]
fn startup_rejects_external_bind_and_incomplete_node_configuration_without_echoing() {
    for args in [
        vec!["--bind", "0.0.0.0:8080"],
        vec!["--rpc-url", "PRIVATE_MARKER"],
        vec!["--unknown", "PRIVATE_MARKER"],
        vec!["--allowed-origin", "*"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_txsignx-api"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let text = String::from_utf8_lossy(&output.stderr);
        assert!(!text.contains("PRIVATE_MARKER"));
        assert!(text.contains("error:"));
    }
}
#[test]
fn help_documents_owned_node_and_local_default() {
    let output = Command::new(env!("CARGO_BIN_EXE_txsignx-api"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for option in [
        "127.0.0.1:8080",
        "--rpc-cookie-file",
        "--rpc-url",
        "--network",
        "--allow-external",
        "--allowed-origin",
    ] {
        assert!(text.contains(option));
    }
}

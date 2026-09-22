use std::process::{Command, Output};

const LEGACY: &str = include_str!("../../txsignx-core/tests/fixtures/legacy.hex");

fn fixture_path(relative: &str) -> String {
    format!("{}/../../fixtures/{}", env!("CARGO_MANIFEST_DIR"), relative)
}

fn run_cli(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_txsignx"));
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

#[test]
fn no_argument_cli_invocation_exits_zero_with_complete_banner() {
    let output = run_cli(&[], &[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Block wordmark content
    assert!(stdout.contains("████████╗██╗  ██╗███████╗██╗ ██████╗ ███╗   ██╗██╗  ██╗"));
    assert!(stdout.contains("╚══██╔══╝╚██╗██╔╝██╔════╝██║██╔════╝ ████╗  ██║╚██╗██╔╝"));
    assert!(stdout.contains("   ██║    ╚███╔╝ ███████╗██║██║  ███╗██╔██╗ ██║ ╚███╔╝"));
    assert!(stdout.contains("   ██║    ██╔██╗ ╚════██║██║██║   ██║██║╚██╗██║ ██╔██╗"));
    assert!(stdout.contains("   ██║   ██╔╝ ██╗███████║██║╚██████╔╝██║ ╚████║██╔╝ ██╗"));
    assert!(stdout.contains("   ╚═╝   ╚═╝  ╚═╝╚══════╝╚═╝ ╚═════╝ ╚═╝  ╚═══╝╚═╝  ╚═╝"));

    // Positioning and taglines
    assert!(stdout.contains("Bitcoin transaction security before signing."));
    assert!(stdout.contains("Inspect. Verify. Sign with Confidence."));

    // Version
    assert!(stdout.contains(concat!("v", env!("CARGO_PKG_VERSION"))));

    // Quick start & help note
    assert!(stdout.contains("Quick start:"));
    assert!(stdout.contains("txsignx policy list"));
    assert!(stdout.contains("txsignx tx inspect <RAW_TX_HEX>"));
    assert!(stdout.contains("txsignx psbt inspect --file payment.b64"));
    assert!(stdout.contains("txsignx psbt preflight --file payment.b64"));
    assert!(stdout.contains("Run `txsignx --help` for all commands."));
}

#[test]
fn redirected_no_argument_output_contains_no_ansi_escapes() {
    let output = run_cli(&[], &[]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains('\x1b'),
        "redirected stdout must not contain ANSI escape sequences"
    );
}

#[test]
fn no_color_mode_contains_no_ansi_escapes() {
    let output = run_cli(&[], &[("NO_COLOR", "1")]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains('\x1b'),
        "NO_COLOR stdout must not contain ANSI escape sequences"
    );
    assert!(stdout.contains("████████╗"));
    assert!(stdout.contains(concat!("v", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn help_and_version_flags_exit_zero_without_banner() {
    let help = run_cli(&["--help"], &[]);
    assert!(help.status.success());
    let help_text = String::from_utf8(help.stdout).unwrap();
    assert!(help_text.contains("Usage: txsignx [COMMAND]"));
    assert!(!help_text.contains("████████╗"));

    let version = run_cli(&["--version"], &[]);
    assert!(version.status.success());
    let version_text = String::from_utf8(version.stdout).unwrap();
    assert_eq!(
        version_text.trim(),
        concat!("txsignx ", env!("CARGO_PKG_VERSION"))
    );
    assert!(!version_text.contains("████████╗"));
}

#[test]
fn machine_readable_json_purity_across_all_commands() {
    // 1. Policy list --json
    let output = run_cli(&["policy", "list", "--json"], &[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("████████╗"));
    let val: serde_json::Value =
        serde_json::from_str(&text).expect("policy list must be pure valid JSON");
    assert!(val.get("active_rules").is_some());

    // 2. Transaction inspect --json
    let output = run_cli(&["tx", "inspect", LEGACY.trim(), "--json"], &[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("████████╗"));
    let val: serde_json::Value =
        serde_json::from_str(&text).expect("tx inspect must be pure valid JSON");
    assert_eq!(
        val["txid"],
        "15a82427768ac422c8ec5e05866b1ec533064d3c242e4d5295171fba113917c6"
    );

    // 3. PSBT inspect --json
    let psbt_unsigned = fixture_path("psbt-unsigned.b64");
    let output = run_cli(
        &["psbt", "inspect", "--file", &psbt_unsigned, "--json"],
        &[],
    );
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("████████╗"));
    let val: serde_json::Value =
        serde_json::from_str(&text).expect("psbt inspect must be pure valid JSON");
    assert_eq!(val["format"], "BIP174");

    // 4. PSBT preflight --json (PASS)
    let pass_path = fixture_path("policy/pass.b64");
    let output = run_cli(&["psbt", "preflight", "--file", &pass_path, "--json"], &[]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("████████╗"));
    let val: serde_json::Value =
        serde_json::from_str(&text).expect("psbt preflight must be pure valid JSON");
    assert_eq!(val["policy"]["decision"], "pass");

    // 5. PSBT preflight --json (REVIEW)
    let review_path = fixture_path("policy/unusual-sighash.b64");
    let output = run_cli(
        &["psbt", "preflight", "--file", &review_path, "--json"],
        &[],
    );
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("████████╗"));
    let val: serde_json::Value =
        serde_json::from_str(&text).expect("psbt preflight must be pure valid JSON");
    assert_eq!(val["policy"]["decision"], "review");

    // 6. PSBT preflight --json (BLOCK)
    let block_path = fixture_path("policy/800k-fee.b64");
    let output = run_cli(&["psbt", "preflight", "--file", &block_path, "--json"], &[]);
    assert_eq!(output.status.code(), Some(3));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains("████████╗"));
    let val: serde_json::Value =
        serde_json::from_str(&text).expect("psbt preflight must be pure valid JSON");
    assert_eq!(val["policy"]["decision"], "block");
}

#[test]
fn policy_preflight_exit_codes_remain_regression_free() {
    let pass_path = fixture_path("policy/pass.b64");
    let pass = run_cli(&["psbt", "preflight", "--file", &pass_path], &[]);
    assert_eq!(pass.status.code(), Some(0));

    let review_path = fixture_path("policy/unusual-sighash.b64");
    let review = run_cli(&["psbt", "preflight", "--file", &review_path], &[]);
    assert_eq!(review.status.code(), Some(2));

    let block_path = fixture_path("policy/800k-fee.b64");
    let block = run_cli(&["psbt", "preflight", "--file", &block_path], &[]);
    assert_eq!(block.status.code(), Some(3));
}

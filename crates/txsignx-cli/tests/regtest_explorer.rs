use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn find_bitcoind() -> Option<PathBuf> {
    for path in [
        "/opt/homebrew/bin/bitcoind",
        "/usr/local/bin/bitcoind",
        "/usr/bin/bitcoind",
    ] {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn find_bitcoin_cli() -> Option<PathBuf> {
    for path in [
        "/opt/homebrew/bin/bitcoin-cli",
        "/usr/local/bin/bitcoin-cli",
        "/usr/bin/bitcoin-cli",
    ] {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn unused_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

struct NodeGuard {
    child: Child,
    datadir: PathBuf,
}

impl Drop for NodeGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.datadir);
    }
}

#[test]
fn regtest_live_transaction_inspection_and_prevout_resolution() {
    let (Some(bitcoind), Some(bitcoin_cli)) = (find_bitcoind(), find_bitcoin_cli()) else {
        eprintln!("bitcoind or bitcoin-cli not found, skipping live regtest integration test");
        return;
    };

    let datadir = std::env::temp_dir().join(format!("txsignx-cli-regtest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&datadir);
    std::fs::create_dir_all(&datadir).unwrap();

    let rpc_port = unused_port();
    let p2p_port = unused_port();

    let child = Command::new(&bitcoind)
        .arg("-regtest")
        .arg(format!("-datadir={}", datadir.display()))
        .arg("-server=1")
        .arg("-daemon=0")
        .arg("-connect=0")
        .arg("-dnsseed=0")
        .arg("-listen=0")
        .arg("-networkactive=0")
        .arg("-discover=0")
        .arg("-rpcbind=127.0.0.1")
        .arg("-rpcallowip=127.0.0.1")
        .arg(format!("-rpcport={rpc_port}"))
        .arg(format!("-port={p2p_port}"))
        .arg("-txindex=1")
        .arg("-fallbackfee=0.0001")
        .arg("-dbcache=16")
        .arg("-maxmempool=5")
        .arg("-persistmempool=0")
        .arg("-debuglogfile=0")
        .arg("-printtoconsole=0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let _guard = NodeGuard {
        child,
        datadir: datadir.clone(),
    };

    let cli_cmd = |args: &[&str]| -> Result<String, String> {
        let output = Command::new(&bitcoin_cli)
            .arg("-regtest")
            .arg(format!("-datadir={}", datadir.display()))
            .arg("-rpcconnect=127.0.0.1")
            .arg(format!("-rpcport={rpc_port}"))
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8(output.stdout).unwrap().trim().to_string())
        } else {
            Err(String::from_utf8(output.stderr).unwrap())
        }
    };

    // Wait for node readiness
    let started = Instant::now();
    let mut ready = false;
    while started.elapsed() < Duration::from_secs(10) {
        if let Ok(info_json) = cli_cmd(&["getblockchaininfo"]) {
            if info_json.contains("\"chain\": \"regtest\"") {
                ready = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(ready, "regtest node readiness timeout");

    // Create test wallet
    cli_cmd(&["createwallet", "testwallet"]).unwrap();

    // Mine 101 blocks to mature coins
    let mining_addr = cli_cmd(&["-rpcwallet=testwallet", "getnewaddress"]).unwrap();
    cli_cmd(&[
        "-rpcwallet=testwallet",
        "generatetoaddress",
        "101",
        &mining_addr,
    ])
    .unwrap();

    // Create confirmed Transaction A
    let dest_addr_a = cli_cmd(&["-rpcwallet=testwallet", "getnewaddress"]).unwrap();
    let txid_a = cli_cmd(&[
        "-rpcwallet=testwallet",
        "sendtoaddress",
        &dest_addr_a,
        "5.0",
    ])
    .unwrap();
    cli_cmd(&[
        "-rpcwallet=testwallet",
        "generatetoaddress",
        "2",
        &mining_addr,
    ])
    .unwrap();

    // Create unconfirmed Transaction B
    let dest_addr_b = cli_cmd(&["-rpcwallet=testwallet", "getnewaddress"]).unwrap();
    let txid_b = cli_cmd(&[
        "-rpcwallet=testwallet",
        "sendtoaddress",
        &dest_addr_b,
        "1.0",
    ])
    .unwrap();

    let cookie_file = datadir.join("regtest/.cookie");
    let node_url = format!("http://127.0.0.1:{rpc_port}");

    // Inspect Transaction A (Confirmed)
    let out_a = Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args([
            "tx",
            "inspect",
            "--txid",
            &txid_a,
            "--node-url",
            &node_url,
            "--cookie-file",
            cookie_file.to_str().unwrap(),
            "--network",
            "regtest",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out_a.status.success(), "tx inspect A failed");
    let json_a: serde_json::Value = serde_json::from_slice(&out_a.stdout).unwrap();
    assert_eq!(json_a["txid"], txid_a);
    assert_eq!(json_a["chain_context"]["status"], "confirmed");
    assert_eq!(json_a["chain_context"]["confirmations"], 2);
    assert!(json_a["chain_context"]["block_hash"].is_string());
    assert!(json_a["total_input_sats"].as_u64().unwrap() > 0);
    assert!(json_a["fee_sats"].as_u64().unwrap() > 0);
    assert!(json_a["fee_rate"]["sat_per_vb"].as_f64().unwrap() > 0.0);
    assert!(
        json_a["inputs"][0]["resolved_prevout"]["value_sats"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(json_a["outputs"][0]["address"].is_string());

    // Inspect Transaction B (Unconfirmed / Mempool)
    let out_b = Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args([
            "tx",
            "inspect",
            "--txid",
            &txid_b,
            "--node-url",
            &node_url,
            "--cookie-file",
            cookie_file.to_str().unwrap(),
            "--network",
            "regtest",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out_b.status.success(), "tx inspect B failed");
    let json_b: serde_json::Value = serde_json::from_slice(&out_b.stdout).unwrap();
    assert_eq!(json_b["txid"], txid_b);
    assert_eq!(json_b["chain_context"]["status"], "mempool");
    assert!(
        json_b["chain_context"]["confirmations"].is_null()
            || json_b["chain_context"]["confirmations"] == 0
    );
    assert!(json_b["chain_context"]["block_hash"].is_null());
    assert!(json_b["total_input_sats"].as_u64().unwrap() > 0);
    assert!(json_b["fee_sats"].as_u64().unwrap() > 0);

    // Inspect Human output for Transaction A
    let out_human = Command::new(env!("CARGO_BIN_EXE_txsignx"))
        .args([
            "tx",
            "inspect",
            "--txid",
            &txid_a,
            "--node-url",
            &node_url,
            "--cookie-file",
            cookie_file.to_str().unwrap(),
            "--network",
            "regtest",
        ])
        .output()
        .unwrap();
    assert!(out_human.status.success());
    let human_text = String::from_utf8(out_human.stdout).unwrap();
    assert!(human_text.contains("TxSignX Transaction Analysis"));
    assert!(human_text.contains("Status: confirmed"));
    assert!(human_text.contains("Confirmations: 2"));
    assert!(human_text.contains("Input total:"));
    assert!(human_text.contains("Fee rate:"));
    assert!(human_text.contains("Previous output value:"));
}

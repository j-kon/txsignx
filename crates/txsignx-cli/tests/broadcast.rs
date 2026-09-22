use bitcoin::{BlockHash, Network, OutPoint, Transaction, Txid, hashes::Hash};
use std::{cell::Cell, collections::BTreeMap};
use txsignx_node::*;
use txsignx_policy::*;
struct Fake {
    outputs: BTreeMap<OutPoint, bitcoin::TxOut>,
    conflict: bool,
    missing: bool,
    immature: bool,
    mismatch: bool,
    network: Network,
    allow: bool,
    wrong_id: bool,
    switch_after_acceptance: bool,
    test: Cell<usize>,
    send: Cell<usize>,
}
impl NodeRpc for Fake {
    fn blockchain_info(&self) -> Result<BlockchainInfo, NodeError> {
        Ok(BlockchainInfo {
            network: if self.switch_after_acceptance && self.test.get() > 0 {
                Network::Bitcoin
            } else {
                self.network
            },
            blocks: 200,
            headers: 200,
            best_block_hash: BlockHash::all_zeros(),
            initial_block_download: false,
            verification_progress: Some(1.0),
        })
    }
    fn best_block_hash(&self) -> Result<BlockHash, NodeError> {
        Ok(BlockHash::all_zeros())
    }
    fn block_count(&self) -> Result<u64, NodeError> {
        Ok(200)
    }
    fn get_tx_out(&self, p: &OutPoint, m: bool) -> Result<Option<NodeTxOut>, NodeError> {
        if self.missing || (m && self.conflict) {
            return Ok(None);
        }
        Ok(self.outputs.get(p).map(|o| {
            let mut o = o.clone();
            if self.mismatch {
                o.value += bitcoin::Amount::from_sat(1);
            }
            NodeTxOut {
                best_block: BlockHash::all_zeros(),
                confirmations: if self.immature { 99 } else { 101 },
                coinbase: self.immature,
                output: o,
            }
        }))
    }
    fn test_mempool_accept(&self, t: &Transaction) -> Result<MempoolAcceptance, NodeError> {
        self.test.set(self.test.get() + 1);
        Ok(MempoolAcceptance {
            txid: if self.wrong_id {
                Txid::all_zeros()
            } else {
                t.compute_txid()
            },
            allowed: self.allow,
        })
    }
    fn send_raw_transaction(&self, t: &Transaction) -> Result<Txid, NodeError> {
        self.send.set(self.send.get() + 1);
        Ok(t.compute_txid())
    }
    fn get_raw_transaction(&self, _: &Txid) -> Result<NodeTransaction, NodeError> {
        panic!("unexpected get_raw_transaction call in broadcast test")
    }
}
fn setup(text: &str) -> Fake {
    let p = txsignx_core::psbt::decode_psbt(text).unwrap();
    let outputs = p
        .unsigned_tx
        .input
        .iter()
        .zip(&p.inputs)
        .filter_map(|(i, m)| {
            m.witness_utxo
                .clone()
                .or_else(|| {
                    m.non_witness_utxo
                        .as_ref()
                        .and_then(|t| t.output.get(i.previous_output.vout as usize).cloned())
                })
                .map(|o| (i.previous_output, o))
        })
        .collect();
    Fake {
        outputs,
        conflict: false,
        missing: false,
        immature: false,
        mismatch: false,
        network: Network::Regtest,
        allow: true,
        wrong_id: false,
        switch_after_acceptance: false,
        test: Cell::new(0),
        send: Cell::new(0),
    }
}
fn wallet(r: &txsignx_core::PsbtReport, change: &[usize]) -> txsignx_wallet::WalletContextReport {
    let c = txsignx_wallet::WalletConfig::new(
        include_str!("../../../fixtures/wallet/external.desc"),
        include_str!("../../../fixtures/wallet/internal.desc"),
        txsignx_wallet::ConfiguredNetwork::Regtest,
        10,
    )
    .unwrap();
    txsignx_wallet::WalletIndex::new(c)
        .unwrap()
        .classify(r, change)
        .unwrap()
}
fn execute(
    text: &str,
    f: &Fake,
    change: &[usize],
    has_wallet: bool,
) -> Result<txsignx_cli::broadcast::BroadcastOutcome, Box<dyn std::error::Error>> {
    let r = txsignx_core::analyze_psbt(text).unwrap();
    let w = wallet(&r, change);
    let n = build_node_context(f, &r, Network::Regtest).unwrap();
    txsignx_cli::broadcast::execute(
        f,
        text,
        &r,
        has_wallet.then_some(&w),
        Some(&n),
        &PolicyConfig::default(),
    )
}
fn finalized() -> String {
    let mut p =
        txsignx_core::psbt::decode_psbt(include_str!("../../../fixtures/wallet/payment.b64"))
            .unwrap();
    for i in &mut p.inputs {
        i.final_script_witness = Some(bitcoin::Witness::from_slice(&[vec![1, 2, 3]]));
    }
    p.to_string()
}
#[test]
fn pass_finalized_candidate_calls_acceptance_then_send() {
    let text = finalized();
    let f = setup(&text);
    let o = execute(&text, &f, &[1], true).unwrap();
    assert!(o.txid.is_some());
    assert_eq!(f.test.get(), 1);
    assert_eq!(f.send.get(), 1);
}
#[test]
fn a_changed_rpc_network_cannot_reuse_a_regtest_report() {
    let text = finalized();
    let mut f = setup(&text);
    let r = txsignx_core::analyze_psbt(&text).unwrap();
    let w = wallet(&r, &[1]);
    let n = build_node_context(&f, &r, Network::Regtest).unwrap();
    f.network = Network::Bitcoin;
    assert!(
        txsignx_cli::broadcast::execute(
            &f,
            &text,
            &r,
            Some(&w),
            Some(&n),
            &PolicyConfig::default()
        )
        .is_err()
    );
    assert_eq!((f.test.get(), f.send.get()), (0, 0));
}
#[test]
fn network_change_after_acceptance_prevents_send() {
    let text = finalized();
    let mut f = setup(&text);
    f.switch_after_acceptance = true;
    assert!(execute(&text, &f, &[1], true).is_err());
    assert_eq!((f.test.get(), f.send.get()), (1, 0));
}
#[test]
fn unfinished_psbt_never_calls_acceptance_or_send() {
    let text = include_str!("../../../fixtures/wallet/payment.b64");
    let f = setup(text);
    assert!(execute(text, &f, &[1], true).is_err());
    assert_eq!((f.test.get(), f.send.get()), (0, 0));
}
#[test]
fn mempool_rejection_or_wrong_candidate_id_never_sends() {
    for wrong in [false, true] {
        let text = finalized();
        let mut f = setup(&text);
        f.allow = wrong;
        f.wrong_id = wrong;
        assert!(execute(&text, &f, &[1], true).is_err());
        assert_eq!((f.test.get(), f.send.get()), (1, 0));
    }
}
#[test]
fn absent_wallet_or_change_never_reaches_acceptance() {
    for has_wallet in [false, true] {
        let text = finalized();
        let f = setup(&text);
        assert!(execute(&text, &f, if has_wallet { &[] } else { &[1] }, has_wallet).is_err());
        assert_eq!((f.test.get(), f.send.get()), (0, 0));
    }
}
#[test]
fn all_node_rule_findings_stop_before_acceptance() {
    for (mode, code, decision) in [
        (0, "TG001", PolicyDecision::Block),
        (1, "TG006", PolicyDecision::Block),
        (2, "TG015", PolicyDecision::Block),
        (3, "TG016", PolicyDecision::Block),
        (4, "TG017", PolicyDecision::Review),
    ] {
        let text = finalized();
        let mut f = setup(&text);
        match mode {
            0 => f.network = Network::Bitcoin,
            1 => f.immature = true,
            2 => f.missing = true,
            3 => f.mismatch = true,
            _ => f.conflict = true,
        };
        let o = execute(&text, &f, &[1], true).unwrap();
        assert_eq!(o.policy.decision, decision);
        assert!(o.policy.findings.iter().any(|f| f.code == code));
        assert_eq!((f.test.get(), f.send.get()), (0, 0));
    }
}
#[test]
fn every_prior_block_and_review_rule_prevents_send() {
    for (folder, name, code) in [
        ("policy", "800k-fee", "TG002"),
        ("policy", "800k-fee", "TG003"),
        ("wallet", "change-hijack", "TG005"),
        ("wallet", "invalid-utxo", "TG009"),
        ("policy", "op-return", "TG014"),
        ("wallet", "foreign-input", "TG004"),
        ("wallet", "external-change", "TG005"),
        ("wallet", "missing-utxo", "TG010"),
        ("policy", "unusual-sighash", "TG011"),
        ("policy", "unknown-script", "TG013"),
    ] {
        let text = std::fs::read_to_string(format!(
            "{}/../../fixtures/{folder}/{name}.b64",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let f = setup(&text);
        let o = execute(&text, &f, &[0], true).unwrap();
        assert!(o.policy.findings.iter().any(|f| f.code == code), "{code}");
        assert!(o.txid.is_none());
        assert_eq!((f.test.get(), f.send.get()), (0, 0));
    }
}

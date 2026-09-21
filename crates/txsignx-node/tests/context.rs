mod common;
use bitcoin::Network;
use common::*;
use txsignx_node::*;
#[test]
fn stable_tip_records_dual_observations_and_matches_prevout() {
    let f = Fake::new();
    let r = build_node_context(&f, &inspection(), Network::Regtest).unwrap();
    assert_eq!(
        r.inputs()[0].availability,
        NodeUtxoAvailability::ConfirmedUnspent
    );
    assert_eq!(
        r.inputs()[0].prevout_verification,
        PrevoutVerification::Match
    );
    assert_eq!(*f.calls.borrow(), vec![false, true]);
    assert_eq!(r.tip().height, 200);
}
#[test]
fn all_four_availability_states_have_precise_semantics() {
    for (c, m, a) in [
        (true, true, NodeUtxoAvailability::ConfirmedUnspent),
        (false, true, NodeUtxoAvailability::MempoolUnconfirmed),
        (true, false, NodeUtxoAvailability::SpentInMempool),
        (false, false, NodeUtxoAvailability::NotAvailable),
    ] {
        let mut f = Fake::new();
        if !c {
            f.chain = None;
        }
        if !m {
            f.mempool = None;
        }
        if !c && m {
            f.mempool.as_mut().unwrap().confirmations = 0;
        }
        let r = build_node_context(&f, &inspection(), Network::Regtest).unwrap();
        assert_eq!(r.inputs()[0].availability, a);
    }
}
#[test]
fn tip_changes_retry_whole_build_but_never_forever() {
    for changes in [1, 3] {
        let mut f = Fake::new();
        f.changes = changes;
        let r = build_node_context(&f, &inspection(), Network::Regtest);
        if changes == 1 {
            assert!(r.is_ok());
            assert_eq!(f.attempts.get(), 2);
        } else {
            assert_eq!(r.err(), Some(NodeError::UnstableChainTip));
            assert_eq!(f.attempts.get(), 3);
        }
    }
}
#[test]
fn unready_and_unavailable_nodes_fail_closed() {
    for mode in 0..4 {
        let mut f = Fake::new();
        match mode {
            0 => f.unavailable = true,
            1 => f.info.initial_block_download = true,
            2 => f.info.headers = 201,
            _ => f.info.verification_progress = Some(f64::NAN),
        };
        assert!(build_node_context(&f, &inspection(), Network::Regtest).is_err());
        assert!(f.calls.borrow().is_empty());
    }
}
#[test]
fn comparisons_cover_value_script_and_both() {
    for (value, script, expected) in [
        (true, false, PrevoutVerification::ValueMismatch),
        (false, true, PrevoutVerification::ScriptMismatch),
        (true, true, PrevoutVerification::ValueAndScriptMismatch),
    ] {
        let mut f = Fake::new();
        for o in [&mut f.chain, &mut f.mempool] {
            let o = o.as_mut().unwrap();
            if value {
                o.output.value += bitcoin::Amount::from_sat(1);
            }
            if script {
                o.output.script_pubkey = bitcoin::ScriptBuf::new();
            }
        }
        assert_eq!(
            build_node_context(&f, &inspection(), Network::Regtest)
                .unwrap()
                .inputs()[0]
                .prevout_verification,
            expected
        );
    }
}
#[test]
fn inconsistent_views_and_impossible_confirmations_are_errors() {
    for mode in 0..3 {
        let mut f = Fake::new();
        match mode {
            0 => f.mempool.as_mut().unwrap().output.value += bitcoin::Amount::from_sat(1),
            1 => f.chain.as_mut().unwrap().confirmations = 0,
            _ => f.chain.as_mut().unwrap().confirmations = 202,
        };
        assert!(build_node_context(&f, &inspection(), Network::Regtest).is_err());
    }
}
#[test]
fn report_binding_includes_prevout_value_script_and_metadata() {
    let r = inspection();
    let n = build_node_context(&Fake::new(), &r, Network::Regtest).unwrap();
    n.validate_for(&r).unwrap();
    for mode in 0..4 {
        let mut changed = r.clone();
        match mode {
            0 => changed.inputs[0].utxo.value_sats = Some(1),
            1 => changed.inputs[0].previous_vout += 1,
            2 => changed.unsigned_txid = "00".repeat(32),
            _ => {
                changed.inputs[0].sighash_type = Some(txsignx_core::psbt::PsbtSighashReport {
                    value: 2,
                    name: "NONE".into(),
                })
            }
        };
        assert_eq!(n.validate_for(&changed), Err(NodeError::ContextMismatch));
    }
}
#[test]
fn missing_metadata_is_not_repaired() {
    let mut r = inspection();
    r.inputs[0].utxo.status = txsignx_core::psbt::PsbtUtxoStatus::Missing;
    r.inputs[0].utxo.value_sats = None;
    r.inputs[0].utxo.script_pubkey_hex = None;
    let n = build_node_context(&Fake::new(), &r, Network::Regtest).unwrap();
    assert_eq!(
        n.inputs()[0].prevout_verification,
        PrevoutVerification::Unavailable
    );
}
#[test]
fn deterministic_report_and_input_limits() {
    let f = Fake::new();
    let r = inspection();
    assert_eq!(
        serde_json::to_string(&build_node_context(&f, &r, Network::Regtest).unwrap()).unwrap(),
        serde_json::to_string(&build_node_context(&f, &r, Network::Regtest).unwrap()).unwrap()
    );
    let mut r = r;
    r.inputs = vec![r.inputs[0].clone(); MAX_NODE_INPUTS + 1];
    r.input_count = r.inputs.len();
    assert!(build_node_context(&f, &r, Network::Regtest).is_err());
}
#[test]
fn stale_bestblock_in_utxo_never_escapes_as_complete_context() {
    let mut f = Fake::new();
    f.chain.as_mut().unwrap().best_block = hash(7);
    assert_eq!(
        build_node_context(&f, &inspection(), Network::Regtest).unwrap_err(),
        NodeError::UnstableChainTip
    );
    assert_eq!(f.attempts.get(), 3);
}
#[test]
fn duplicate_and_reordered_inputs_are_rejected() {
    let mut r = inspection();
    r.inputs.push(r.inputs[0].clone());
    r.inputs[1].index = 1;
    r.input_count = 2;
    assert_eq!(
        build_node_context(&Fake::new(), &r, Network::Regtest).unwrap_err(),
        NodeError::InvalidInspection
    );
    r.inputs[0].index = 1;
    assert!(build_node_context(&Fake::new(), &r, Network::Regtest).is_err());
}
#[test]
fn malformed_resolved_script_fails_without_echo() {
    let mut r = inspection();
    r.inputs[0].utxo.script_pubkey_hex = Some("SECRET_MARKER".into());
    let e = build_node_context(&Fake::new(), &r, Network::Regtest).unwrap_err();
    assert_eq!(e, NodeError::InvalidInspection);
    assert!(!e.to_string().contains("SECRET_MARKER"));
}

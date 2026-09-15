//! Public-only deterministic demo builder. Prints TSV; no file/network I/O.
#[path = "../tests/common/mod.rs"]
mod common;
use bitcoin::{
    Amount, Psbt,
    base64::{Engine, engine::general_purpose::STANDARD},
};
fn base() -> Psbt {
    let mut p =
        txsignx_core::psbt::decode_psbt(include_str!("../../../fixtures/psbt-unsigned.b64"))
            .unwrap();
    p.inputs[0].witness_utxo.as_mut().unwrap().script_pubkey = common::script(0, 7);
    p.unsigned_tx.output[0].script_pubkey = common::script(2, 0);
    p.unsigned_tx.output[1].script_pubkey = common::script(1, 3);
    p
}
fn main() {
    let (external, internal) = common::descriptors();
    println!("external.desc\t{external}\ninternal.desc\t{internal}");
    let mut cases = vec![("payment", base())];
    let mut p = base();
    p.unsigned_tx.output[1].script_pubkey = common::script(2, 3);
    cases.push(("change-hijack", p));
    let mut p = base();
    p.unsigned_tx.output[1].script_pubkey = common::script(0, 3);
    cases.push(("external-change", p));
    let mut p = base();
    p.inputs[0].witness_utxo.as_mut().unwrap().script_pubkey = common::script(2, 7);
    cases.push(("foreign-input", p));
    let mut p = base();
    let mut input = p.unsigned_tx.input[0].clone();
    input.previous_output.vout = 1;
    p.unsigned_tx.input.push(input);
    let mut metadata = p.inputs[0].clone();
    metadata.witness_utxo.as_mut().unwrap().script_pubkey = common::script(2, 7);
    p.inputs.push(metadata);
    p.unsigned_tx.output[0].value =
        Amount::from_sat(p.unsigned_tx.output[0].value.to_sat() + 151000);
    cases.push(("collaborative", p));
    let mut p = base();
    p.inputs[0].witness_utxo = None;
    cases.push(("missing-utxo", p));
    let mut p = base();
    p.inputs[0].non_witness_utxo = Some(p.unsigned_tx.clone());
    cases.push(("invalid-utxo", p));
    for (name, index) in [("boundary-999", 999), ("boundary-1000", 1000)] {
        let mut p = base();
        p.inputs[0].witness_utxo.as_mut().unwrap().script_pubkey = common::script(0, index);
        cases.push((name, p));
    }
    let mut p = base();
    p.inputs[0].witness_utxo.as_mut().unwrap().script_pubkey = common::script(0, 0);
    p.unsigned_tx.output[1].script_pubkey = common::script(1, 0);
    cases.push(("index-zero", p));
    for (name, p) in cases {
        println!("{name}.b64\t{}", STANDARD.encode(p.serialize()));
    }
}

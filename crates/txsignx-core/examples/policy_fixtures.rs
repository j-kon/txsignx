//! Deterministic public dummy PSBTs for policy demonstrations. Prints TSV to
//! stdout; no private keys, signatures, network access or file writes.
use bitcoin::{
    Amount, Psbt, ScriptBuf,
    base64::{Engine, engine::general_purpose::STANDARD},
};
fn base() -> Psbt {
    txsignx_core::psbt::decode_psbt(include_str!("../../../fixtures/psbt-unsigned.b64")).unwrap()
}
fn fee_case(outputs: u64, fee: u64) -> Psbt {
    let mut psbt = base();
    psbt.unsigned_tx.output[0].value = Amount::from_sat(outputs);
    psbt.unsigned_tx.output[1].value = Amount::ZERO;
    psbt.inputs[0].witness_utxo.as_mut().unwrap().value =
        Amount::from_sat(outputs.checked_add(fee).unwrap());
    psbt
}
fn main() {
    let mut cases = vec![
        ("pass", base()),
        ("absolute-fee", fee_case(2_000_000, 200_000)),
        ("percentage-fee", fee_case(10_000, 2_000)),
        ("800k-fee", fee_case(100_000, 800_000)),
    ];
    let mut missing = base();
    missing.inputs[0].witness_utxo = None;
    cases.push(("missing-utxo", missing));
    let mut invalid = base();
    invalid.inputs[0].non_witness_utxo = Some(invalid.unsigned_tx.clone());
    cases.push(("invalid-utxo", invalid));
    let mut sighash = base();
    sighash.inputs[0].sighash_type = Some(bitcoin::psbt::PsbtSighashType::from_u32(2));
    cases.push(("unusual-sighash", sighash));
    let mut opreturn = base();
    opreturn.unsigned_tx.output[0].script_pubkey = ScriptBuf::from_bytes(vec![0x6a]);
    cases.push(("op-return", opreturn));
    let mut unknown = base();
    unknown.unsigned_tx.output[0].script_pubkey = ScriptBuf::from_bytes(vec![0x51]);
    cases.push(("unknown-script", unknown));
    let mut negative = base();
    negative.inputs[0].witness_utxo.as_mut().unwrap().value = Amount::from_sat(1);
    cases.push(("negative-fee", negative));
    let mut extension = base();
    extension.unknown.insert(
        bitcoin::psbt::raw::Key {
            type_value: 0xee,
            key: b"PUBLIC_DUMMY_KEY".to_vec(),
        },
        b"\x1b[31mPRIVATE_SENTINEL".to_vec(),
    );
    cases.push(("extension-metadata", extension));
    for (name, psbt) in cases {
        println!("{name}\t{}", STANDARD.encode(psbt.serialize()));
    }
}

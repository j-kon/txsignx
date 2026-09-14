#![allow(dead_code)]
use bitcoin::{
    Amount, Psbt, PublicKey, ScriptBuf, TxOut,
    base64::{Engine, engine::general_purpose::STANDARD},
};
#[path = "../common/mod.rs"]
mod transaction_common;

/// Public dummy transaction and generator public key; no private keys are used.
pub fn unsigned() -> Psbt {
    let mut tx = transaction_common::fixture_transaction(false);
    tx.input[0].script_sig = ScriptBuf::new();
    let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
    psbt.inputs[0].witness_utxo = Some(TxOut {
        value: Amount::from_sat(151_000),
        script_pubkey: psbt.unsigned_tx.output[1].script_pubkey.clone(),
    });
    psbt
}
pub fn public_key() -> PublicKey {
    "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        .parse()
        .unwrap()
}
pub fn partial() -> Psbt {
    let mut psbt = unsigned();
    // Syntactically encoded r=s=0x0101..., not a valid signature for this transaction.
    let signature = bitcoin::ecdsa::Signature {
        signature: bitcoin::secp256k1::ecdsa::Signature::from_compact(&[1; 64]).unwrap(),
        sighash_type: bitcoin::sighash::EcdsaSighashType::All,
    };
    psbt.inputs[0].partial_sigs.insert(public_key(), signature);
    psbt.inputs[0].sighash_type = Some(bitcoin::sighash::EcdsaSighashType::All.into());
    psbt
}
pub fn encode(psbt: &Psbt) -> String {
    STANDARD.encode(psbt.serialize())
}

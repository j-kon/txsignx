use bitcoin::{
    Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness, absolute,
    hashes::Hash, hex::FromHex, transaction,
};

/// Deterministic public dummy data: a decodable transaction, not a spendable one.
pub fn fixture_transaction(segwit: bool) -> Transaction {
    Transaction {
        version: transaction::Version::TWO,
        lock_time: absolute::LockTime::from_consensus(42),
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::from_byte_array([0x11; 32]),
                vout: 1,
            },
            script_sig: ScriptBuf::from_bytes(vec![0x01, 0x51]),
            sequence: Sequence(0xffff_fffd),
            witness: if segwit {
                Witness::from_slice(&[&[1, 2, 3][..], &[][..], &[0xab, 0xcd][..]])
            } else {
                Witness::new()
            },
        }],
        output: vec![
            TxOut {
                value: Amount::from_sat(100_000),
                script_pubkey: ScriptBuf::from_bytes(
                    Vec::from_hex(&format!("76a914{}88ac", "22".repeat(20))).unwrap(),
                ),
            },
            TxOut {
                value: Amount::from_sat(50_000),
                script_pubkey: ScriptBuf::from_bytes(
                    Vec::from_hex(&format!("0014{}", "33".repeat(20))).unwrap(),
                ),
            },
        ],
    }
}

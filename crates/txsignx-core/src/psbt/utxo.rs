use crate::transaction::{ScriptType, classify_script};
use bitcoin::{OutPoint, TxOut, psbt::Input};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PsbtUtxoSource {
    Missing,
    WitnessUtxo,
    NonWitnessUtxo,
    Both,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PsbtUtxoStatus {
    Missing,
    Valid,
    TxidMismatch,
    VoutOutOfRange,
    WitnessNonWitnessMismatch,
}

/// Consistency of supplied metadata, not authentication against the chain.
#[derive(Debug, Clone, Serialize)]
pub struct PsbtUtxoReport {
    pub source: PsbtUtxoSource,
    pub status: PsbtUtxoStatus,
    pub value_sats: Option<u64>,
    pub script_pubkey_hex: Option<String>,
    pub script_pubkey_size_bytes: Option<usize>,
    pub script_type: Option<ScriptType>,
}

pub(super) fn resolve<'a>(
    input: &'a Input,
    outpoint: &OutPoint,
) -> (PsbtUtxoSource, PsbtUtxoStatus, Option<&'a TxOut>) {
    use PsbtUtxoStatus::*;
    let source = match (
        input.witness_utxo.is_some(),
        input.non_witness_utxo.is_some(),
    ) {
        (false, false) => PsbtUtxoSource::Missing,
        (true, false) => PsbtUtxoSource::WitnessUtxo,
        (false, true) => PsbtUtxoSource::NonWitnessUtxo,
        (true, true) => PsbtUtxoSource::Both,
    };
    let previous = if let Some(tx) = &input.non_witness_utxo {
        if tx.compute_txid() != outpoint.txid {
            return (source, TxidMismatch, None);
        }
        let output = usize::try_from(outpoint.vout)
            .ok()
            .and_then(|vout| tx.output.get(vout));
        let Some(output) = output else {
            return (source, VoutOutOfRange, None);
        };
        Some(output)
    } else {
        None
    };
    match (&input.witness_utxo, previous) {
        (Some(witness), Some(previous)) if witness != previous => {
            (source, WitnessNonWitnessMismatch, None)
        }
        (Some(output), _) | (None, Some(output)) => (source, Valid, Some(output)),
        (None, None) => (source, Missing, None),
    }
}

pub(super) fn inspect(input: &Input, outpoint: &OutPoint) -> PsbtUtxoReport {
    let (source, status, output) = resolve(input, outpoint);
    PsbtUtxoReport {
        source,
        status,
        value_sats: output.map(|o| o.value.to_sat()),
        script_pubkey_hex: output.map(|o| o.script_pubkey.to_hex_string()),
        script_pubkey_size_bytes: output.map(|o| o.script_pubkey.len()),
        script_type: output.map(|o| classify_script(&o.script_pubkey)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{Amount, ScriptBuf, Transaction, absolute, transaction};
    fn context() -> (Input, OutPoint) {
        let tx = Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![],
            output: vec![TxOut {
                value: Amount::from_sat(10),
                script_pubkey: ScriptBuf::new(),
            }],
        };
        let point = OutPoint {
            txid: tx.compute_txid(),
            vout: 0,
        };
        (
            Input {
                non_witness_utxo: Some(tx),
                ..Input::default()
            },
            point,
        )
    }
    #[test]
    fn resolves_missing_witness_nonwitness_and_both() {
        let (mut input, point) = context();
        assert_eq!(
            inspect(&Input::default(), &point).status,
            PsbtUtxoStatus::Missing
        );
        assert_eq!(
            inspect(&input, &point).source,
            PsbtUtxoSource::NonWitnessUtxo
        );
        input.witness_utxo = Some(input.non_witness_utxo.as_ref().unwrap().output[0].clone());
        assert_eq!(inspect(&input, &point).source, PsbtUtxoSource::Both);
        assert_eq!(inspect(&input, &point).value_sats, Some(10));
        input.non_witness_utxo = None;
        assert_eq!(inspect(&input, &point).source, PsbtUtxoSource::WitnessUtxo);
        assert_eq!(inspect(&input, &point).status, PsbtUtxoStatus::Valid);
    }
    #[test]
    fn rejects_wrong_txid_even_with_witness_context() {
        let (mut input, point) = context();
        input.witness_utxo = Some(input.non_witness_utxo.as_ref().unwrap().output[0].clone());
        input.non_witness_utxo.as_mut().unwrap().lock_time = absolute::LockTime::from_consensus(1);
        let report = inspect(&input, &point);
        assert_eq!(report.status, PsbtUtxoStatus::TxidMismatch);
        assert!(report.value_sats.is_none() && report.script_pubkey_hex.is_none());
    }
    #[test]
    fn rejects_out_of_range_vout() {
        let (input, mut point) = context();
        point.vout = u32::MAX;
        assert_eq!(
            inspect(&input, &point).status,
            PsbtUtxoStatus::VoutOutOfRange
        );
    }
    #[test]
    fn rejects_value_and_script_mismatches() {
        let (input, point) = context();
        for witness in [
            TxOut {
                value: Amount::from_sat(11),
                script_pubkey: ScriptBuf::new(),
            },
            TxOut {
                value: Amount::from_sat(10),
                script_pubkey: ScriptBuf::from_bytes(vec![0x51]),
            },
        ] {
            let mut input = input.clone();
            input.witness_utxo = Some(witness);
            let report = inspect(&input, &point);
            assert_eq!(report.status, PsbtUtxoStatus::WitnessNonWitnessMismatch);
            assert!(report.value_sats.is_none() && report.script_type.is_none());
        }
    }
}

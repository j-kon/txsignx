use super::{PsbtUtxoStatus, utxo};
use bitcoin::Psbt;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PsbtFeeStatus {
    Available,
    MissingUtxoContext,
    InvalidUtxoContext,
    NegativeFee,
    Overflow,
    OtherError,
}
#[derive(Debug, Clone, Serialize)]
pub struct PsbtFeeReport {
    pub status: PsbtFeeStatus,
    pub fee_sats: Option<u64>,
}

pub(super) fn inspect(psbt: &Psbt) -> PsbtFeeReport {
    use PsbtFeeStatus::*;
    // rust-bitcoin's fee helper requires matching map counts and otherwise trusts
    // witness_utxo first. Validate both forms before invoking that helper.
    if psbt.inputs.len() != psbt.unsigned_tx.input.len() {
        return PsbtFeeReport {
            status: InvalidUtxoContext,
            fee_sats: None,
        };
    }
    let mut missing = false;
    for (input, txin) in psbt.inputs.iter().zip(&psbt.unsigned_tx.input) {
        match utxo::resolve(input, &txin.previous_output).1 {
            PsbtUtxoStatus::Valid => (),
            PsbtUtxoStatus::Missing => missing = true,
            _ => {
                return PsbtFeeReport {
                    status: InvalidUtxoContext,
                    fee_sats: None,
                };
            }
        }
    }
    if missing {
        return PsbtFeeReport {
            status: MissingUtxoContext,
            fee_sats: None,
        };
    }
    match psbt.fee() {
        Ok(fee) => PsbtFeeReport {
            status: Available,
            fee_sats: Some(fee.to_sat()),
        },
        Err(error) => PsbtFeeReport {
            status: match error {
                bitcoin::psbt::Error::NegativeFee => NegativeFee,
                bitcoin::psbt::Error::FeeOverflow => Overflow,
                _ => OtherError,
            },
            fee_sats: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{Amount, ScriptBuf, Transaction, TxIn, TxOut, absolute, transaction};
    fn fixture() -> Psbt {
        let mut psbt = Psbt::from_unsigned_tx(Transaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![TxIn::default(), TxIn::default()],
            output: vec![TxOut {
                value: Amount::from_sat(100),
                script_pubkey: ScriptBuf::new(),
            }],
        })
        .unwrap();
        for input in &mut psbt.inputs {
            input.witness_utxo = Some(TxOut {
                value: Amount::from_sat(60),
                script_pubkey: ScriptBuf::new(),
            });
        }
        psbt
    }
    #[test]
    fn available_fee_is_checked_sum_minus_outputs() {
        let report = inspect(&fixture());
        assert_eq!(report.status, PsbtFeeStatus::Available);
        assert_eq!(report.fee_sats, Some(20));
    }
    #[test]
    fn missing_and_negative_fees_have_no_value() {
        let mut psbt = fixture();
        psbt.inputs[0].witness_utxo = None;
        let report = inspect(&psbt);
        assert_eq!(report.status, PsbtFeeStatus::MissingUtxoContext);
        assert_eq!(report.fee_sats, None);
        psbt.inputs[0].witness_utxo = Some(TxOut {
            value: Amount::ZERO,
            script_pubkey: ScriptBuf::new(),
        });
        let report = inspect(&psbt);
        assert_eq!(report.status, PsbtFeeStatus::NegativeFee);
        assert_eq!(report.fee_sats, None);
    }
    #[test]
    fn overflow_and_invalid_context_have_no_value() {
        let mut psbt = fixture();
        psbt.inputs[0].witness_utxo.as_mut().unwrap().value = Amount::from_sat(u64::MAX);
        let report = inspect(&psbt);
        assert_eq!(report.status, PsbtFeeStatus::Overflow);
        assert_eq!(report.fee_sats, None);
        psbt.inputs[0].non_witness_utxo = Some(psbt.unsigned_tx.clone());
        psbt.inputs[1].witness_utxo = None;
        let report = inspect(&psbt);
        assert_eq!(report.status, PsbtFeeStatus::InvalidUtxoContext);
        assert_eq!(report.fee_sats, None);
        psbt.inputs.clear(); // protect the upstream helper's assertion as well
        assert_eq!(inspect(&psbt).status, PsbtFeeStatus::InvalidUtxoContext);
    }
}

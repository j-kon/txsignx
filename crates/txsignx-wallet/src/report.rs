use crate::*;
use bitcoin::{ScriptBuf, hex::FromHex};
use serde::Serialize;
use txsignx_core::{PsbtReport, psbt::PsbtUtxoStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletInputContext {
    pub index: usize,
    pub ownership: WalletOwnership,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletOutputContext {
    pub index: usize,
    pub ownership: WalletOwnership,
    pub expected_change: bool,
}
/// Constructed only from a WalletIndex; callers get read-only facts.
#[derive(Debug, Clone, Serialize)]
pub struct WalletContextReport {
    configured_network: ConfiguredNetwork,
    derivation_window: u32,
    expected_change_outputs: Vec<usize>,
    inputs: Vec<WalletInputContext>,
    outputs: Vec<WalletOutputContext>,
    #[serde(skip)]
    binding: InspectionBinding,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectionBinding {
    txid: String,
    inputs: Vec<(usize, PsbtUtxoStatus, Option<String>)>,
    outputs: Vec<(usize, String)>,
}
impl InspectionBinding {
    fn new(r: &PsbtReport) -> Result<Self, WalletError> {
        if r.input_count != r.inputs.len()
            || r.output_count != r.outputs.len()
            || r.inputs.iter().enumerate().any(|(n, i)| n != i.index)
            || r.outputs
                .iter()
                .enumerate()
                .any(|(n, o)| n != o.transaction_output.index)
        {
            return Err(WalletError::InconsistentInspection);
        }
        Ok(Self {
            txid: r.unsigned_txid.clone(),
            inputs: r
                .inputs
                .iter()
                .map(|i| (i.index, i.utxo.status, i.utxo.script_pubkey_hex.clone()))
                .collect(),
            outputs: r
                .outputs
                .iter()
                .map(|o| {
                    (
                        o.transaction_output.index,
                        o.transaction_output.script_pubkey_hex.clone(),
                    )
                })
                .collect(),
        })
    }
}
impl WalletContextReport {
    pub fn configured_network(&self) -> ConfiguredNetwork {
        self.configured_network
    }
    pub fn derivation_window(&self) -> u32 {
        self.derivation_window
    }
    pub fn expected_change_outputs(&self) -> &[usize] {
        &self.expected_change_outputs
    }
    pub fn inputs(&self) -> &[WalletInputContext] {
        &self.inputs
    }
    pub fn outputs(&self) -> &[WalletOutputContext] {
        &self.outputs
    }
    pub fn validate_for(&self, inspection: &PsbtReport) -> Result<(), WalletError> {
        if self.binding == InspectionBinding::new(inspection)? {
            Ok(())
        } else {
            Err(WalletError::ContextMismatch)
        }
    }
}
impl WalletIndex {
    pub fn classify(
        &self,
        inspection: &PsbtReport,
        expected_change: &[usize],
    ) -> Result<WalletContextReport, WalletError> {
        let binding = InspectionBinding::new(inspection)?;
        if expected_change
            .iter()
            .any(|&i| i >= inspection.output_count)
        {
            return Err(WalletError::InvalidExpectedChange);
        }
        if expected_change.len() > inspection.output_count {
            return Err(WalletError::DuplicateExpectedChange);
        }
        let mut expected = expected_change.to_vec();
        expected.sort_unstable();
        if expected.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(WalletError::DuplicateExpectedChange);
        }
        let mut inputs = Vec::with_capacity(inspection.input_count);
        for input in &inspection.inputs {
            let ownership = match input.utxo.status {
                PsbtUtxoStatus::Valid => self.classify_hex(
                    input
                        .utxo
                        .script_pubkey_hex
                        .as_deref()
                        .ok_or(WalletError::InconsistentInspection)?,
                )?,
                PsbtUtxoStatus::Missing => WalletOwnership::Unavailable {
                    reason: WalletContextUnavailableReason::MissingPrevoutContext,
                },
                _ => WalletOwnership::Unavailable {
                    reason: WalletContextUnavailableReason::InvalidPrevoutContext,
                },
            };
            inputs.push(WalletInputContext {
                index: input.index,
                ownership,
            });
        }
        let mut outputs = Vec::with_capacity(inspection.output_count);
        for output in &inspection.outputs {
            let o = &output.transaction_output;
            outputs.push(WalletOutputContext {
                index: o.index,
                ownership: self.classify_hex(&o.script_pubkey_hex)?,
                expected_change: expected.binary_search(&o.index).is_ok(),
            });
        }
        Ok(WalletContextReport {
            configured_network: self.network,
            derivation_window: self.window,
            expected_change_outputs: expected,
            inputs,
            outputs,
            binding,
        })
    }
    fn classify_hex(&self, hex: &str) -> Result<WalletOwnership, WalletError> {
        let bytes = Vec::<u8>::from_hex(hex).map_err(|_| WalletError::InconsistentInspection)?;
        Ok(self.classify_script(&ScriptBuf::from_bytes(bytes)))
    }
}

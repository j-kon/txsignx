use crate::*;
pub struct InvalidUtxoContext;
pub struct MissingUtxoContext;
impl PolicyRule for InvalidUtxoContext {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG009",
            title: "Invalid UTXO Context",
            description: "Supplied previous-output metadata is internally inconsistent.",
            default_severity: Severity::Critical,
            active: true,
            required_context: vec!["input UTXO consistency"],
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        use txsignx_core::psbt::PsbtUtxoStatus;
        context.inspection.inputs.iter().filter(|i|!matches!(i.utxo.status,PsbtUtxoStatus::Valid|PsbtUtxoStatus::Missing)).map(|i| {
            let status=match i.utxo.status {
                PsbtUtxoStatus::TxidMismatch=>"txid_mismatch",
                PsbtUtxoStatus::VoutOutOfRange=>"vout_out_of_range",
                PsbtUtxoStatus::WitnessNonWitnessMismatch=>"witness_non_witness_mismatch",
                _=>"invalid",
            };
            self.metadata().finding(FindingLocation::Input{index:i.index},
                format!("Supplied UTXO metadata is internally inconsistent ({status})."),
                "Resolve the supplied previous-output metadata mismatch using trusted wallet/node context.")
        }).collect()
    }
}
impl PolicyRule for MissingUtxoContext {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG010",
            title: "Missing UTXO Context",
            description: "Previous-output information is missing, preventing reliable fee evaluation.",
            default_severity: Severity::High,
            active: true,
            required_context: vec!["input UTXO presence"],
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        context.inspection.inputs.iter().filter(|i|i.utxo.status==txsignx_core::psbt::PsbtUtxoStatus::Missing).map(|i|
            self.metadata().finding(FindingLocation::Input{index:i.index},
                "Previous-output context is missing; TxSignX cannot reliably evaluate the fee without it. Missing metadata alone does not establish PSBT invalidity.",
                "Provide consistent previous-output information or verify it using wallet/node context.")
        ).collect()
    }
}

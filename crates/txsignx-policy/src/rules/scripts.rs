use crate::*;
use txsignx_core::{psbt::PsbtUtxoStatus, transaction::ScriptType};
pub struct UnrecognizedScriptType;
pub struct NonZeroOpReturnValue;
impl PolicyRule for UnrecognizedScriptType {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG013",
            title: "Unrecognized Script Type",
            description: "An output or resolved prevout uses an unrecognized script template.",
            default_severity: Severity::Medium,
            active: true,
            required_context: vec![
                "output script classification",
                "resolved prevout script classification",
            ],
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        let metadata = self.metadata();
        let message = "TxSignX does not recognize this script template and cannot apply template-specific assumptions. Custom scripts may be legitimate.";
        let recommendation = "Review the script's intended spending conditions with appropriate script-aware tooling.";
        let mut findings = Vec::new();
        for input in &context.inspection.inputs {
            if input.utxo.status == PsbtUtxoStatus::Valid
                && input.utxo.value_sats.is_some()
                && input.utxo.script_type == Some(ScriptType::Unknown)
            {
                findings.push(metadata.finding(
                    FindingLocation::Input { index: input.index },
                    message,
                    recommendation,
                ));
            }
        }
        for output in &context.inspection.outputs {
            if output.transaction_output.script_type == ScriptType::Unknown {
                findings.push(metadata.finding(
                    FindingLocation::Output {
                        index: output.transaction_output.index,
                    },
                    message,
                    recommendation,
                ));
            }
        }
        findings
    }
}
impl PolicyRule for NonZeroOpReturnValue {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG014",
            title: "Non-Zero OP_RETURN Value",
            description: "Positive value is assigned to a provably unspendable OP_RETURN output.",
            default_severity: Severity::Critical,
            active: true,
            required_context: vec!["output value", "output script classification"],
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        context.inspection.outputs.iter().filter(|o|o.transaction_output.script_type==ScriptType::OpReturn && o.transaction_output.value_sats>0).map(|o|
            self.metadata().finding(FindingLocation::Output{index:o.transaction_output.index},
                format!("OP_RETURN output contains {} sats. OP_RETURN outputs are provably unspendable; the assigned value is expected to become unspendable.",o.transaction_output.value_sats),
                "Review and remove unintended value from the OP_RETURN output before proceeding.")
        ).collect()
    }
}

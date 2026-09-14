use crate::*;
pub struct UnknownOrProprietaryMetadata;
impl PolicyRule for UnknownOrProprietaryMetadata {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG012",
            title: "Unknown or Proprietary Metadata",
            description: "PSBT contains extension metadata that TxSignX does not interpret.",
            default_severity: Severity::Info,
            active: true,
            required_context: vec!["global/input/output extension counts"],
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        let report = context.inspection;
        // Widen each count before summing. Real vectors cannot hold enough
        // usize counts to exhaust u128 on supported 32-/64-bit targets.
        let unknown = (report.unknown_count as u128)
            + report
                .inputs
                .iter()
                .map(|i| i.unknown_count as u128)
                .sum::<u128>()
            + report
                .outputs
                .iter()
                .map(|o| o.unknown_count as u128)
                .sum::<u128>();
        let proprietary = (report.proprietary_count as u128)
            + report
                .inputs
                .iter()
                .map(|i| i.proprietary_count as u128)
                .sum::<u128>()
            + report
                .outputs
                .iter()
                .map(|o| o.proprietary_count as u128)
                .sum::<u128>();
        if unknown == 0 && proprietary == 0 {
            return vec![];
        }
        vec![self.metadata().finding(FindingLocation::Global,
            format!("PSBT contains extension metadata that TxSignX does not interpret. Unknown fields: {unknown}; proprietary fields: {proprietary} (all maps combined)."),
            "Confirm the purpose of extension metadata with its originating application; its presence alone is not evidence of malicious intent.")]
    }
}

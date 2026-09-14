use crate::*;
pub struct UnusualSighashType;
impl PolicyRule for UnusualSighashType {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG011",
            title: "Unusual Sighash Type",
            description: "An explicit sighash differs from SIGHASH_DEFAULT or SIGHASH_ALL.",
            default_severity: Severity::High,
            active: true,
            required_context: vec!["explicit numeric sighash"],
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        context.inspection.inputs.iter().filter_map(|i| {
            let sighash=i.sighash_type.as_ref()?;
            if matches!(sighash.value,0|1) { return None; }
            Some(self.metadata().finding(FindingLocation::Input{index:i.index},
                format!("Explicit sighash value {} differs from the common all-input/all-output commitment expectation or is unrecognized. Manual review is required; unusual sighashes can be intentional.",sighash.value),
                "Verify that the intended signature commitment matches this explicit sighash value."))
        }).collect()
    }
}

use crate::*;
pub struct ExcessiveAbsoluteFee;
pub struct ExcessiveFeePercentage;
impl PolicyRule for ExcessiveAbsoluteFee {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG002",
            title: "Excessive Absolute Fee",
            description: "Fee exceeds the configured absolute development limit.",
            default_severity: Severity::Critical,
            active: true,
            required_context: vec!["available absolute fee"],
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        let Some(fee) = available_fee(context) else {
            return vec![];
        };
        let threshold = context.config.max_absolute_fee_sats;
        if fee <= threshold {
            return vec![];
        }
        vec![self.metadata().finding(
            FindingLocation::Global,
            format!(
                "Transaction fee is {fee} sats, exceeding the configured {threshold}-sat limit."
            ),
            "Review transaction inputs, outputs, and fee configuration before signing.",
        )]
    }
}
impl PolicyRule for ExcessiveFeePercentage {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            code: "TG003",
            title: "Excessive Fee Percentage",
            description: "Fee share of total input value exceeds the configured development limit.",
            default_severity: Severity::Critical,
            active: true,
            required_context: vec!["available absolute fee", "total output value"],
        }
    }
    fn evaluate(&self, context: &PolicyContext<'_>) -> Vec<Finding> {
        let Some(fee) = available_fee(context) else {
            return vec![];
        };
        // Widen before addition and multiplication: two u64 amounts and 10,000
        // fit in u128, including hand-constructed reports at u64 boundaries.
        let input_total = u128::from(context.inspection.total_output_sats) + u128::from(fee);
        let threshold = context.config.max_fee_ratio_bps;
        if u128::from(fee) * 10_000 <= input_total * u128::from(threshold) {
            return vec![];
        }
        vec![self.metadata().finding(FindingLocation::Global,
            format!("Fee is {fee} sats out of {input_total} sats total input value, exceeding the configured {threshold}-basis-point fee share limit."),
            "Review the fee as a share of total input value and confirm the configured threshold.")]
    }
}

fn available_fee(context: &PolicyContext<'_>) -> Option<u64> {
    if context.inspection.fee.status == txsignx_core::psbt::PsbtFeeStatus::Available {
        context.inspection.fee.fee_sats
    } else {
        None
    }
}

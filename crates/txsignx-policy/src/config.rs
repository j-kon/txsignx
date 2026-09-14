use crate::PolicyError;
use serde::Serialize;

/// Development thresholds, not consensus limits or universal recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PolicyConfig {
    pub max_absolute_fee_sats: u64,
    /// Fee share of total input value, in basis points (10,000 = 100%).
    pub max_fee_ratio_bps: u16,
}
impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            max_absolute_fee_sats: 100_000,
            max_fee_ratio_bps: 1_000,
        }
    }
}
impl PolicyConfig {
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.max_fee_ratio_bps > 10_000 {
            Err(PolicyError::InvalidFeeRatio)
        } else {
            Ok(())
        }
    }
}

//! Resource ceilings; satisfying them is not proof of consensus or policy validity.

/// A transaction cannot be larger in bytes than the 4,000,000 WU block ceiling.
pub const MAX_TRANSACTION_BYTES: usize = 4_000_000;
/// Checked before allocating the decoded bytes; hex uses two characters per byte.
pub const MAX_TRANSACTION_HEX_CHARS: usize = MAX_TRANSACTION_BYTES * 2;
/// BIP141 block-weight ceiling, used as an upper bound for one transaction.
pub const MAX_TRANSACTION_WEIGHT_WU: u64 = 4_000_000;

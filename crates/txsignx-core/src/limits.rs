//! Resource ceilings; satisfying them is not proof of consensus or policy validity.

/// A transaction cannot be larger in bytes than the 4,000,000 WU block ceiling.
pub const MAX_TRANSACTION_BYTES: usize = 4_000_000;
/// Checked before allocating the decoded bytes; hex uses two characters per byte.
pub const MAX_TRANSACTION_HEX_CHARS: usize = MAX_TRANSACTION_BYTES * 2;
/// BIP141 block-weight ceiling, used as an upper bound for one transaction.
pub const MAX_TRANSACTION_WEIGHT_WU: u64 = 4_000_000;

/// Cap structured witness expansion across all inputs, independently of byte size.
pub const MAX_REPORT_WITNESS_ITEMS: usize = 100_000;

/// Application limit for decoded PSBT data, not a consensus/block limit.
pub const MAX_PSBT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PSBT_BASE64_CHARS: usize = MAX_PSBT_BYTES.div_ceil(3) * 4;
/// Permit a terminal CRLF in a maximum-size base64 file.
pub const MAX_PSBT_TEXT_BYTES: usize = MAX_PSBT_BASE64_CHARS + 2;
/// Limit metadata-map overhead before rust-bitcoin allocates parsed structures.
pub const MAX_PSBT_MAPS: usize = 100_000;
pub const MAX_PSBT_PAIRS: usize = 100_000;
/// A v0 unsigned transaction has no witness, so its bytes weigh four WU each.
pub const MAX_PSBT_UNSIGNED_TX_BYTES: usize = 1_000_000;

/// Aggregate bytes in output TapTree values. Each encoded leaf needs at least
/// three bytes; this caps dependency expansion at 4,096 leaves before parsing.
pub const MAX_PSBT_TAP_TREE_BYTES: usize = 12_288;

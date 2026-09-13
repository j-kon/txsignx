//! Reusable Bitcoin transaction inspection.

pub mod transaction;

pub mod error;
pub mod limits;
pub use error::AnalysisError;

pub use transaction::{TransactionReport, analyze_transaction};

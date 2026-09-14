//! Ephemeral, bounded public descriptor classification. No chain or signing APIs.
mod config;
mod error;
pub use config::{
    ConfiguredNetwork, DEFAULT_DERIVATION_WINDOW, MAX_DERIVATION_WINDOW, MAX_DESCRIPTOR_BYTES,
    WalletConfig,
};
pub use error::WalletError;
mod index;
mod model;
pub use index::WalletIndex;
pub use model::*;

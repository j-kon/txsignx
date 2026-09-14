use crate::WalletError;
use bdk_wallet::{
    descriptor::{ExtendedDescriptor, IntoWalletDescriptor},
    miniscript::Descriptor,
};
use bitcoin::{Network, secp256k1::Secp256k1};
use serde::Serialize;

pub const DEFAULT_DERIVATION_WINDOW: u32 = 1000;
pub const MAX_DERIVATION_WINDOW: u32 = 10000;
pub const MAX_DESCRIPTOR_BYTES: usize = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfiguredNetwork {
    Bitcoin,
    Testnet,
    Testnet4,
    Signet,
    Regtest,
}
impl ConfiguredNetwork {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bitcoin => "bitcoin",
            Self::Testnet => "testnet",
            Self::Testnet4 => "testnet4",
            Self::Signet => "signet",
            Self::Regtest => "regtest",
        }
    }
    pub fn bitcoin_network(self) -> Network {
        match self {
            Self::Bitcoin => Network::Bitcoin,
            Self::Testnet => Network::Testnet,
            Self::Testnet4 => Network::Testnet4,
            Self::Signet => Network::Signet,
            Self::Regtest => Network::Regtest,
        }
    }
}
impl std::str::FromStr for ConfiguredNetwork {
    type Err = WalletError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bitcoin" | "mainnet" => Ok(Self::Bitcoin),
            "testnet" => Ok(Self::Testnet),
            "testnet4" => Ok(Self::Testnet4),
            "signet" => Ok(Self::Signet),
            "regtest" => Ok(Self::Regtest),
            _ => Err(WalletError::InvalidNetwork),
        }
    }
}

/// Public-only configuration. No Debug/Serialize implementation: descriptor
/// text, keys and origins must never appear in diagnostic or report output.
pub struct WalletConfig {
    pub(crate) external: ExtendedDescriptor,
    pub(crate) internal: ExtendedDescriptor,
    pub(crate) network: ConfiguredNetwork,
    pub(crate) window: u32,
}
impl WalletConfig {
    pub fn new(
        external: &str,
        internal: &str,
        network: ConfiguredNetwork,
        window: u32,
    ) -> Result<Self, WalletError> {
        if !(1..=MAX_DERIVATION_WINDOW).contains(&window) {
            return Err(WalletError::InvalidWindow);
        }
        let external = parse(external, network, WalletError::InvalidExternalDescriptor)?;
        let internal = parse(internal, network, WalletError::InvalidInternalDescriptor)?;
        if external == internal {
            return Err(WalletError::AmbiguousScripts);
        }
        Ok(Self {
            external,
            internal,
            network,
            window,
        })
    }
}
fn parse(
    text: &str,
    network: ConfiguredNetwork,
    error: WalletError,
) -> Result<ExtendedDescriptor, WalletError> {
    if text.len() > MAX_DESCRIPTOR_BYTES {
        return Err(WalletError::DescriptorTooLarge);
    }
    // This FromStr only accepts DescriptorPublicKey. Do NOT replace with
    // parse_descriptor or IntoWalletDescriptor on a string (both accept secrets).
    let descriptor: ExtendedDescriptor = text.trim().parse().map_err(|_| error)?;
    descriptor.sanity_check().map_err(|_| error)?;
    if !descriptor.has_wildcard() {
        return Err(WalletError::NonRangedDescriptor);
    }
    if descriptor.is_multipath() {
        return Err(WalletError::MultipathDescriptor);
    }
    let secp = Secp256k1::new();
    let (descriptor, keys) = descriptor
        .into_wallet_descriptor(&secp, network.bitcoin_network().into())
        .map_err(|_| error)?;
    if !keys.is_empty() {
        return Err(error);
    }
    // Validate public derivability before accepting configuration.
    let definite: Descriptor<_> = descriptor.at_derivation_index(0).map_err(|_| error)?;
    definite.derived_descriptor(&secp).map_err(|_| error)?;
    Ok(descriptor)
}

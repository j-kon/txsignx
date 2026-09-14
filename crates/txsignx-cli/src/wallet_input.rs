use clap::Args;
use std::{error::Error, fmt, fs::File, io::Read, path::PathBuf};
use txsignx_wallet::{DEFAULT_DERIVATION_WINDOW, MAX_DESCRIPTOR_BYTES, WalletConfig};
#[derive(Args)]
#[group(multiple = true)]
pub struct WalletArgs {
    /// Public ranged receive descriptor. Prefer a file to avoid shell history/process listings.
    #[arg(long, conflicts_with = "external_descriptor_file")]
    external_descriptor: Option<String>,
    /// Read one public receive descriptor from a bounded text file (64 KiB).
    #[arg(long)]
    external_descriptor_file: Option<PathBuf>,
    /// Public ranged change descriptor. Prefer a file for privacy.
    #[arg(long, conflicts_with = "internal_descriptor_file")]
    internal_descriptor: Option<String>,
    /// Read one public change descriptor from a bounded text file (64 KiB).
    #[arg(long)]
    internal_descriptor_file: Option<PathBuf>,
    /// Explicit configured network: bitcoin (alias mainnet), testnet, testnet4, signet, regtest.
    #[arg(long)]
    network: Option<String>,
    /// Derive indexes 0..COUNT. Default 1000; allowed 1..=10000.
    #[arg(long, value_name = "COUNT")]
    derivation_window: Option<u32>,
    /// Explicit expected change, zero-based output index; repeatable, duplicates rejected.
    #[arg(long, value_name = "INDEX")]
    pub expected_change_output: Vec<usize>,
}
#[derive(Debug, Clone, Copy)]
enum InputError {
    Incomplete,
    Read,
}
impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Incomplete => {
                "wallet mode requires external descriptor, internal descriptor and explicit network"
            }
            Self::Read => "descriptor file could not be read as bounded UTF-8 text",
        })
    }
}
impl Error for InputError {}
impl WalletArgs {
    /// Validate the entire wallet option group before opening PSBT files/stdin.
    pub fn config(&self) -> Result<Option<WalletConfig>, Box<dyn Error>> {
        let external =
            self.external_descriptor.is_some() || self.external_descriptor_file.is_some();
        let internal =
            self.internal_descriptor.is_some() || self.internal_descriptor_file.is_some();
        let enabled = external
            || internal
            || self.network.is_some()
            || self.derivation_window.is_some()
            || !self.expected_change_output.is_empty();
        if !enabled {
            return Ok(None);
        }
        if !external || !internal {
            return Err(InputError::Incomplete.into());
        }
        let network = self
            .network
            .as_deref()
            .ok_or(InputError::Incomplete)?
            .parse()?;
        let window = self.derivation_window.unwrap_or(DEFAULT_DERIVATION_WINDOW);
        if !(1..=txsignx_wallet::MAX_DERIVATION_WINDOW).contains(&window) {
            return Err(txsignx_wallet::WalletError::InvalidWindow.into());
        }
        let external = source(&self.external_descriptor, &self.external_descriptor_file)?;
        let internal = source(&self.internal_descriptor, &self.internal_descriptor_file)?;
        Ok(Some(WalletConfig::new(
            &external, &internal, network, window,
        )?))
    }
}
fn source(direct: &Option<String>, file: &Option<PathBuf>) -> Result<String, Box<dyn Error>> {
    match (direct, file) {
        (Some(text), None) => {
            if text.len() > MAX_DESCRIPTOR_BYTES {
                return Err(txsignx_wallet::WalletError::DescriptorTooLarge.into());
            }
            Ok(text.clone())
        }
        (None, Some(path)) => bounded_text(File::open(path).map_err(|_| InputError::Read)?),
        _ => Err(InputError::Incomplete.into()),
    }
}
fn bounded_text(reader: impl Read) -> Result<String, Box<dyn Error>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_DESCRIPTOR_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| InputError::Read)?;
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(txsignx_wallet::WalletError::DescriptorTooLarge.into());
    }
    String::from_utf8(bytes).map_err(|_| InputError::Read.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_reader_stops_at_limit_plus_one() {
        let mut reader = std::io::repeat(b'A').take(MAX_DESCRIPTOR_BYTES as u64 + 100);
        assert!(bounded_text(&mut reader).is_err());
        assert_eq!(reader.limit(), 99);
    }
}

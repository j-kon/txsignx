use clap::Args;
use std::{
    error::Error,
    fs::File,
    io::{self, Read},
    path::PathBuf,
};
use txsignx_core::{limits::MAX_PSBT_TEXT_BYTES, psbt::PsbtError};

#[derive(Args)]
#[group(id = "psbt_source", required = true, multiple = false)]
pub struct PsbtSource {
    /// Standard base64 PSBT v0. Prefer --file or --stdin to avoid shell history.
    #[arg(value_name = "PSBT")]
    pub psbt: Option<String>,
    /// Read standard base64 PSBT text from a file.
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,
    /// Read standard base64 PSBT text from stdin until EOF or the safety limit.
    #[arg(long)]
    pub stdin: bool,
}

fn bounded_text(reader: impl Read) -> Result<String, Box<dyn Error>> {
    let limit = u64::try_from(MAX_PSBT_TEXT_BYTES)?
        .checked_add(1)
        .ok_or(PsbtError::ResourceLimit)?;
    let mut bytes = Vec::new();
    reader.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PSBT_TEXT_BYTES {
        return Err(PsbtError::TextTooLarge {
            limit: MAX_PSBT_TEXT_BYTES,
        }
        .into());
    }
    String::from_utf8(bytes).map_err(|_| PsbtError::InvalidBase64.into())
}

pub fn read(source: PsbtSource) -> Result<String, Box<dyn Error>> {
    match (source.psbt, source.file, source.stdin) {
        (Some(text), None, false) => Ok(text),
        (None, Some(path), false) => bounded_text(File::open(path)?),
        (None, None, true) => bounded_text(io::stdin().lock()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "choose exactly one PSBT source: positional text, --file, or --stdin",
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reader_stops_after_limit_plus_one_without_silent_truncation() {
        let mut reader = io::repeat(b'A').take((MAX_PSBT_TEXT_BYTES as u64) + 100);
        let error = bounded_text(&mut reader).unwrap_err();
        assert!(error.to_string().contains("limit"));
        assert_eq!(reader.limit(), 99);
    }
}

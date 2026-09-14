use super::PsbtError;
use crate::limits::{MAX_PSBT_BASE64_CHARS, MAX_PSBT_BYTES, MAX_PSBT_TEXT_BYTES};
use bitcoin::{
    Psbt,
    base64::{Engine, engine::general_purpose::STANDARD},
};

/// Parse standard-base64 PSBT v0. Only surrounding ASCII whitespace is accepted.
/// No input data or upstream privacy-bearing error is retained in boundary errors.
pub fn decode_psbt(text: &str) -> Result<Psbt, PsbtError> {
    if text.len() > MAX_PSBT_TEXT_BYTES {
        return Err(PsbtError::TextTooLarge {
            limit: MAX_PSBT_TEXT_BYTES,
        });
    }
    let text = text.trim_ascii();
    if text.is_empty() {
        return Err(PsbtError::EmptyInput);
    }
    if text.len() > MAX_PSBT_BASE64_CHARS {
        return Err(PsbtError::TextTooLarge {
            limit: MAX_PSBT_BASE64_CHARS,
        });
    }
    let bytes = STANDARD
        .decode(text)
        .map_err(|_| PsbtError::InvalidBase64)?;
    if bytes.len() > MAX_PSBT_BYTES {
        return Err(PsbtError::DecodedTooLarge {
            limit: MAX_PSBT_BYTES,
        });
    }
    super::framing::check_framing(&bytes)?;
    let mut unread = bytes.as_slice();
    let psbt = Psbt::deserialize_from_reader(&mut unread).map_err(|error| match error {
        // rust-bitcoin 0.32 provides a static reason, not the unsupported version number.
        bitcoin::psbt::Error::Version("PSBT versions greater than 0 are not supported") => {
            PsbtError::UnsupportedVersion
        }
        _ => PsbtError::InvalidPsbt,
    })?;
    if !unread.is_empty() {
        return Err(PsbtError::TrailingData);
    }
    Ok(psbt)
}

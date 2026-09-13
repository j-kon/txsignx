//! Allocation-free resource preflight, not a semantic PSBT implementation.
//! rust-bitcoin validates the keys, values, required fields and supported version.
use super::PsbtError;
use crate::limits::{
    MAX_PSBT_MAPS, MAX_PSBT_PAIRS, MAX_PSBT_TAP_TREE_BYTES, MAX_PSBT_UNSIGNED_TX_BYTES,
};
use bitcoin::consensus::{Decodable, encode::VarInt};

fn length(bytes: &mut &[u8]) -> Result<usize, PsbtError> {
    let size = VarInt::consensus_decode(bytes)
        .map_err(|_| PsbtError::InvalidPsbt)?
        .0;
    usize::try_from(size).map_err(|_| PsbtError::ResourceLimit)
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], PsbtError> {
    let (field, rest) = bytes.split_at_checked(len).ok_or(PsbtError::InvalidPsbt)?;
    *bytes = rest;
    Ok(field)
}

pub(super) fn check_framing(bytes: &[u8]) -> Result<(), PsbtError> {
    let mut unread = bytes
        .strip_prefix(b"psbt\xff")
        .ok_or(PsbtError::InvalidMagic)?;
    let mut maps = 0;
    let mut pairs = 0;
    let mut ended_map = false;
    let mut tree_bytes: usize = 0;
    while !unread.is_empty() {
        let key_len = length(&mut unread)?;
        if key_len == 0 {
            maps += 1;
            if maps > MAX_PSBT_MAPS {
                return Err(PsbtError::ResourceLimit);
            }
            ended_map = true;
            continue;
        }
        ended_map = false;
        pairs += 1;
        if pairs > MAX_PSBT_PAIRS {
            return Err(PsbtError::ResourceLimit);
        }
        let key = take(&mut unread, key_len)?;
        let value_len = length(&mut unread)?;
        // Bound allocation of per-input/output PSBT maps derived from this transaction.
        // A v0 unsigned transaction uses stripped serialization (4 WU per byte).
        if maps == 0 && key == [0] && value_len > MAX_PSBT_UNSIGNED_TX_BYTES {
            return Err(PsbtError::ResourceLimit);
        }
        // The key with exactly one byte 0x06 is an output TapTree. In an input
        // map, 0x06 requires public-key keydata, so this shape is invalid there.
        // Bound all such non-global fields without needing semantic map parsing.
        if maps > 0 && key == [6] {
            tree_bytes = tree_bytes
                .checked_add(value_len)
                .ok_or(PsbtError::ResourceLimit)?;
            if tree_bytes > MAX_PSBT_TAP_TREE_BYTES {
                return Err(PsbtError::ResourceLimit);
            }
        }
        take(&mut unread, value_len)?;
    }
    if !ended_map {
        return Err(PsbtError::InvalidPsbt);
    }
    Ok(())
}

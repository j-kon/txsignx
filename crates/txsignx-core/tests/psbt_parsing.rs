mod common;
use bitcoin::{
    Psbt,
    base64::{Engine, engine::general_purpose::STANDARD},
};
use txsignx_core::{
    limits::*,
    psbt::{PsbtError, decode_psbt},
};

fn valid() -> String {
    let mut tx = common::fixture_transaction(false);
    tx.input[0].script_sig = bitcoin::ScriptBuf::new();
    STANDARD.encode(Psbt::from_unsigned_tx(tx).unwrap().serialize())
}

#[test]
fn standard_base64_v0_and_surrounding_ascii_whitespace() {
    let text = valid();
    let psbt = decode_psbt(&format!(" \n{text}\r\n")).unwrap();
    assert_eq!(psbt.version, 0);
    assert_eq!(psbt.inputs.len(), 1);
    assert_eq!(psbt.outputs.len(), 2);
}
#[test]
fn empty_psbt_is_rejected() {
    for text in ["", " \r\n\t"] {
        assert!(matches!(decode_psbt(text), Err(PsbtError::EmptyInput)));
    }
}
#[test]
fn malformed_base64_and_random_formats_are_rejected() {
    for text in ["%%%", "cHNidP8", "cHNidP8=\nAA==", "é", "cHNidP_="] {
        assert!(
            matches!(decode_psbt(text), Err(PsbtError::InvalidBase64)),
            "{text}"
        );
    }
}
#[test]
fn invalid_magic_is_rejected() {
    assert!(decode_psbt("0x70736274ff").is_err());
    assert!(matches!(
        decode_psbt(&STANDARD.encode(b"nope!")),
        Err(PsbtError::InvalidMagic)
    ));
}
#[test]
fn truncated_maps_and_every_fixture_prefix_are_rejected() {
    let bytes = STANDARD.decode(valid()).unwrap();
    for n in 0..bytes.len() {
        assert!(decode_psbt(&STANDARD.encode(&bytes[..n])).is_err(), "{n}");
    }
}
#[test]
fn trailing_bytes_and_extra_maps_are_rejected() {
    let bytes = STANDARD.decode(valid()).unwrap();
    for suffix in [vec![0], vec![1, 0, 0], bytes.clone()] {
        assert!(decode_psbt(&STANDARD.encode([bytes.clone(), suffix].concat())).is_err());
    }
}
#[test]
fn unsupported_version_is_typed_but_bad_version_length_is_malformed() {
    let v2 = b"psbt\xff\x01\xfb\x04\x02\x00\x00\x00\x00";
    assert!(matches!(
        decode_psbt(&STANDARD.encode(v2)),
        Err(PsbtError::UnsupportedVersion)
    ));
    let malformed = b"psbt\xff\x01\xfb\x01\x02\x00";
    assert!(matches!(
        decode_psbt(&STANDARD.encode(malformed)),
        Err(PsbtError::InvalidPsbt)
    ));
}
#[test]
fn text_limit_precedes_base64_allocation() {
    assert!(matches!(
        decode_psbt(&"!".repeat(MAX_PSBT_TEXT_BYTES + 1)),
        Err(PsbtError::TextTooLarge { .. })
    ));
}
#[test]
fn decoded_limit_cannot_be_bypassed_by_base64_padding() {
    let bytes = vec![0; MAX_PSBT_BYTES + 1];
    assert!(matches!(
        decode_psbt(&STANDARD.encode(bytes)),
        Err(PsbtError::DecodedTooLarge { .. })
    ));
}
#[test]
fn errors_never_include_unknown_keys_or_values() {
    let marker = b"PRIVATE_METADATA_SENTINEL";
    let mut bytes = STANDARD.decode(valid()).unwrap();
    bytes.truncate(bytes.len() - 2); // replace an output map with a duplicate unknown key
    for _ in 0..2 {
        bytes.extend([u8::try_from(marker.len() + 1).unwrap(), 0xfa]);
        bytes.extend(marker);
        bytes.push(u8::try_from(marker.len()).unwrap());
        bytes.extend(marker);
    }
    bytes.push(0);
    let err = decode_psbt(&STANDARD.encode(bytes)).unwrap_err();
    assert!(!format!("{err} {err:?}").contains("PRIVATE_METADATA_SENTINEL"));
}

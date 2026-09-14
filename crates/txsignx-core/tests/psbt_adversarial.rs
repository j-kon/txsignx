mod psbt_common;
use bitcoin::{
    base64::{Engine, engine::general_purpose::STANDARD},
    consensus::{encode::VarInt, serialize},
    psbt::raw,
};
use txsignx_core::{
    analyze_psbt,
    limits::*,
    psbt::{PsbtError, decode_psbt},
};

#[test]
fn arbitrary_short_buffers_and_mutations_never_panic() {
    for a in 0u8..=255 {
        assert!(decode_psbt(&STANDARD.encode([a])).is_err());
        for b in 0u8..=255 {
            assert!(decode_psbt(&STANDARD.encode([a, b])).is_err());
        }
    }
    let bytes = psbt_common::unsigned().serialize();
    for index in 0..bytes.len() {
        for byte in [0, 1, 0x7f, 0xff] {
            let mut changed = bytes.clone();
            changed[index] = byte;
            // Some mutations are valid alternate PSBTs: only absence of panic is asserted.
            let _ = analyze_psbt(&STANDARD.encode(changed));
        }
    }
}
#[test]
fn huge_declared_lengths_and_malformed_map_keys_fail() {
    for suffix in [
        vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        vec![
            1, 0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ],
        vec![0xfd, 1, 0, 0xee, 0, 0], // nonminimal key length
        vec![1, 0xee, 1],             // absent value
        vec![2, 0],                   // truncated key
    ] {
        let bytes = [b"psbt\xff".to_vec(), suffix].concat();
        assert!(decode_psbt(&STANDARD.encode(bytes)).is_err());
    }
    let mut bytes = psbt_common::unsigned().serialize();
    bytes[4] = 0;
    assert!(matches!(
        decode_psbt(&STANDARD.encode(bytes)),
        Err(PsbtError::InvalidMagic)
    ));
}
#[test]
fn map_pair_and_unsigned_transaction_limits_precede_semantic_allocation() {
    let mut maps = b"psbt\xff".to_vec();
    maps.resize(maps.len() + MAX_PSBT_MAPS + 1, 0);
    assert!(matches!(
        decode_psbt(&STANDARD.encode(maps)),
        Err(PsbtError::ResourceLimit)
    ));
    let mut pairs = b"psbt\xff".to_vec();
    for _ in 0..=MAX_PSBT_PAIRS {
        pairs.extend([1, 0xee, 0]);
    }
    pairs.push(0);
    assert!(matches!(
        decode_psbt(&STANDARD.encode(pairs)),
        Err(PsbtError::ResourceLimit)
    ));
    let mut transaction = b"psbt\xff\x01\x00".to_vec();
    transaction.extend(serialize(&VarInt((MAX_PSBT_UNSIGNED_TX_BYTES + 1) as u64)));
    assert!(matches!(
        decode_psbt(&STANDARD.encode(transaction)),
        Err(PsbtError::ResourceLimit)
    ));
}
#[test]
fn metadata_can_exceed_transaction_block_bytes_without_being_rejected() {
    let mut psbt = psbt_common::unsigned();
    // Distinct 3 MiB fields comply with the dependency's per-value decoder bound.
    for key in [1, 2] {
        psbt.inputs[0].unknown.insert(
            raw::Key {
                type_value: 0xee,
                key: vec![key],
            },
            vec![0x41; 3 * 1024 * 1024],
        );
    }
    let report = analyze_psbt(&psbt_common::encode(&psbt)).unwrap();
    assert_eq!(report.inputs[0].unknown_count, 2);
    assert!(serde_json::to_string(&report).unwrap().len() < 10_000);
}
#[test]
fn psbt_exact_decoded_limit_is_accepted_and_one_byte_more_rejected() {
    let mut psbt = psbt_common::unsigned();
    for key in 0..5 {
        psbt.inputs[0].unknown.insert(
            raw::Key {
                type_value: 0xee,
                key: vec![key],
            },
            vec![0; 3 * 1024 * 1024],
        );
    }
    let base = psbt.serialize().len();
    // key length(1)+key(2)+value CompactSize(5) = eight bytes overhead.
    let remaining = MAX_PSBT_BYTES - base - 8;
    psbt.inputs[0].unknown.insert(
        raw::Key {
            type_value: 0xee,
            key: vec![5],
        },
        vec![0; remaining],
    );
    assert_eq!(psbt.serialize().len(), MAX_PSBT_BYTES);
    let text = psbt_common::encode(&psbt);
    assert!(decode_psbt(&format!("{text}\r\n")).is_ok());
    let mut bytes = psbt.serialize();
    bytes.push(0);
    assert!(matches!(
        decode_psbt(&STANDARD.encode(bytes)),
        Err(PsbtError::DecodedTooLarge { .. })
    ));
}

#[test]
fn tap_tree_expansion_is_bounded_before_dependency_allocation() {
    let mut psbt = psbt_common::unsigned();
    // A valid balanced tree of 8,192 empty scripts fits in only 24,576 bytes.
    psbt.outputs[0].unknown.insert(
        raw::Key {
            type_value: 6,
            key: vec![],
        },
        [13, 0xc0, 0].repeat(8192),
    );
    assert!(matches!(
        decode_psbt(&psbt_common::encode(&psbt)),
        Err(PsbtError::ResourceLimit)
    ));
}

#[test]
fn tap_tree_budget_is_aggregate_and_accepts_boundary() {
    let mut psbt = psbt_common::unsigned();
    let key = raw::Key {
        type_value: 6,
        key: vec![],
    };
    psbt.outputs[0]
        .unknown
        .insert(key.clone(), [12, 0xc0, 0].repeat(4096));
    assert!(decode_psbt(&psbt_common::encode(&psbt)).is_ok());
    psbt.outputs[1].unknown.insert(key, vec![0, 0xc0, 0]);
    assert!(matches!(
        decode_psbt(&psbt_common::encode(&psbt)),
        Err(PsbtError::ResourceLimit)
    ));
}

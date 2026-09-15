use txsignx_wallet::{ConfiguredNetwork, WalletConfig, WalletError};
#[test]
fn invalid_public_descriptor_is_sanitized() {
    let error = WalletConfig::new(
        "TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO",
        "bad",
        ConfiguredNetwork::Regtest,
        1,
    )
    .err()
    .unwrap();
    assert_eq!(error, WalletError::InvalidExternalDescriptor);
    assert!(!format!("{error:?} {error}").contains("SENTINEL"));
}
#[test]
fn invalid_windows_rejected_before_parsing() {
    for window in [0, 10001, u32::MAX] {
        assert_eq!(
            WalletConfig::new("bad", "bad", ConfiguredNetwork::Regtest, window).err(),
            Some(WalletError::InvalidWindow)
        );
    }
}
mod common;
#[test]
fn public_ranged_descriptors_accept_all_test_networks() {
    let (e, i) = common::descriptors();
    for network in [
        ConfiguredNetwork::Testnet,
        ConfiguredNetwork::Testnet4,
        ConfiguredNetwork::Signet,
        ConfiguredNetwork::Regtest,
    ] {
        assert!(WalletConfig::new(&e, &i, network, 1).is_ok());
    }
    assert!(WalletConfig::new(&e, &i, ConfiguredNetwork::Bitcoin, 1).is_err());
    let mut key = common::public_key();
    key.network = bitcoin::NetworkKind::Main;
    assert!(
        WalletConfig::new(
            &format!("wpkh({key}/0/*)"),
            &format!("wpkh({key}/1/*)"),
            ConfiguredNetwork::Bitcoin,
            1
        )
        .is_ok()
    );
}
#[test]
fn fixed_descriptors_are_rejected() {
    let (_, i) = common::descriptors();
    let e = format!("wpkh({}/0/0)", common::public_key());
    assert_eq!(
        WalletConfig::new(&e, &i, ConfiguredNetwork::Regtest, 1).err(),
        Some(WalletError::NonRangedDescriptor)
    );
}
#[test]
fn identical_descriptors_are_rejected() {
    let (e, _) = common::descriptors();
    assert_eq!(
        WalletConfig::new(&e, &e, ConfiguredNetwork::Regtest, 1).err(),
        Some(WalletError::AmbiguousScripts)
    );
}
#[test]
fn hardened_public_derivation_is_rejected() {
    let (_, i) = common::descriptors();
    for suffix in ["/0h/*", "/0/*h"] {
        let e = format!("wpkh({}{suffix})", common::public_key());
        assert!(WalletConfig::new(&e, &i, ConfiguredNetwork::Regtest, 1).is_err());
    }
}
#[test]
fn multipath_descriptors_are_rejected() {
    let (_, i) = common::descriptors();
    let e = format!("wpkh({}/<0;1>/*)", common::public_key());
    assert_eq!(
        WalletConfig::new(&e, &i, ConfiguredNetwork::Regtest, 1).err(),
        Some(WalletError::MultipathDescriptor)
    );
}
#[test]
fn checksum_is_validated_and_errors_are_sanitized() {
    let (e, i) = common::descriptors();
    let d: bdk_wallet::descriptor::ExtendedDescriptor = e.parse().unwrap();
    assert!(WalletConfig::new(&d.to_string(), &i, ConfiguredNetwork::Regtest, 1).is_ok());
    let invalid = format!("{e}#aaaaaaaa");
    let error = WalletConfig::new(&invalid, &i, ConfiguredNetwork::Regtest, 1)
        .err()
        .unwrap();
    assert!(!error.to_string().contains("aaaaaaaa"));
}
#[test]
fn descriptor_size_bound_is_inclusive() {
    let (e, i) = common::descriptors();
    let at = format!(
        "{e}{}",
        " ".repeat(txsignx_wallet::MAX_DESCRIPTOR_BYTES - e.len())
    );
    assert!(WalletConfig::new(&at, &i, ConfiguredNetwork::Regtest, 1).is_ok());
    assert_eq!(
        WalletConfig::new(&(at + " "), &i, ConfiguredNetwork::Regtest, 1).err(),
        Some(WalletError::DescriptorTooLarge)
    );
}
#[test]
fn generated_dummy_extended_private_keys_are_rejected_without_echo() {
    // TEST ONLY: construct public dummy rejected encodings; no real secret or seed.
    use bitcoin::{NetworkKind, bip32::Xpriv, secp256k1::SecretKey};
    let (_, internal) = common::descriptors();
    for network in [NetworkKind::Main, NetworkKind::Test] {
        let dummy = Xpriv {
            network,
            depth: 0,
            parent_fingerprint: Default::default(),
            child_number: bitcoin::bip32::ChildNumber::Normal { index: 0 },
            private_key: SecretKey::from_slice(&[1; 32]).unwrap(),
            chain_code: bitcoin::bip32::ChainCode::from([42; 32]),
        };
        let text = format!("wpkh({dummy}/0/*)");
        let error = WalletConfig::new(&text, &internal, ConfiguredNetwork::Regtest, 1)
            .err()
            .unwrap();
        assert!(!format!("{error:?} {error}").contains(&dummy.to_string()));
    }
}
#[test]
fn dummy_wif_and_raw_private_keys_are_rejected() {
    let (_, i) = common::descriptors();
    let secret = bitcoin::secp256k1::SecretKey::from_slice(&[1; 32]).unwrap();
    for key in [
        bitcoin::PrivateKey::new(secret, bitcoin::NetworkKind::Test).to_wif(),
        "01".repeat(32),
    ] {
        let e = format!("wpkh({key})");
        assert!(WalletConfig::new(&e, &i, ConfiguredNetwork::Regtest, 1).is_err());
        // A secret hidden among public ranged keys also fails the public parser.
        let e = format!("wsh(sortedmulti(1,{key},{}/0/*))", common::public_key());
        assert!(WalletConfig::new(&e, &i, ConfiguredNetwork::Regtest, 1).is_err());
    }
}
#[test]
fn invalid_internal_descriptor_does_not_echo_input() {
    let (e, _) = common::descriptors();
    let error = WalletConfig::new(
        &e,
        "TXSIGNX_SECRET_SENTINEL_DO_NOT_ECHO",
        ConfiguredNetwork::Regtest,
        1,
    )
    .err()
    .unwrap();
    assert_eq!(error, WalletError::InvalidInternalDescriptor);
    assert!(!error.to_string().contains("SENTINEL"));
}
#[test]
fn descriptor_derivation_work_is_bounded_before_expansion() {
    let key = common::public_key();
    let descriptor = |chain| {
        format!(
            "wsh(sortedmulti(1,{}))",
            (0..20)
                .map(|i| format!("{key}/{chain}/{i}/*"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    assert_eq!(
        WalletConfig::new(
            &descriptor(0),
            &descriptor(1),
            ConfiguredNetwork::Regtest,
            10000
        )
        .err(),
        Some(WalletError::ResourceLimit)
    );
}
#[test]
fn depth_exhaustion_returns_error_without_panic() {
    let mut key = common::public_key();
    key.depth = 255;
    let (_, i) = common::descriptors();
    assert!(
        WalletConfig::new(&format!("wpkh({key}/*)"), &i, ConfiguredNetwork::Regtest, 1).is_err()
    );
}
#[test]
fn hardened_origins_are_metadata_and_remain_accepted() {
    let (_, i) = common::descriptors();
    let e = format!("wpkh([deadbeef/84h/1h/0h]{}/0/*)", common::public_key());
    assert!(WalletConfig::new(&e, &i, ConfiguredNetwork::Regtest, 1).is_ok());
}

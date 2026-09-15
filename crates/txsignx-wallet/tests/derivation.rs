mod common;
use txsignx_wallet::{ConfiguredNetwork, WalletConfig, WalletError, WalletIndex, WalletOwnership};
#[test]
fn half_open_window_and_both_keychains() {
    let index = WalletIndex::new(common::config(1000)).unwrap();
    assert_eq!(
        index.classify_script(&common::script(0, 0)),
        WalletOwnership::External {
            derivation_index: 0
        }
    );
    assert_eq!(
        index.classify_script(&common::script(1, 999)),
        WalletOwnership::Internal {
            derivation_index: 999
        }
    );
    assert_eq!(
        index.classify_script(&common::script(0, 1000)),
        WalletOwnership::NoMatchWithinWindow
    );
    let expanded = WalletIndex::new(common::config(1001)).unwrap();
    assert_eq!(
        expanded.classify_script(&common::script(0, 1000)),
        WalletOwnership::External {
            derivation_index: 1000
        }
    );
}
#[test]
fn distinct_text_with_overlapping_scripts_fails() {
    let (external, _) = common::descriptors();
    let internal = format!("wpkh([deadbeef]{}/0/*)", common::public_key());
    let config = WalletConfig::new(&external, &internal, ConfiguredNetwork::Regtest, 2).unwrap();
    assert!(matches!(
        WalletIndex::new(config),
        Err(WalletError::AmbiguousScripts)
    ));
}
#[test]
fn taproot_and_multisig_public_descriptors_are_supported() {
    use bdk_wallet::descriptor::ExtendedDescriptor;
    let key = common::public_key();
    for (e, i) in [
        (format!("tr({key}/0/*)"), format!("tr({key}/1/*)")),
        (
            format!("wsh(sortedmulti(2,{key}/0/*,{key}/2/*))"),
            format!("wsh(sortedmulti(2,{key}/1/*,{key}/3/*))"),
        ),
    ] {
        let config = WalletConfig::new(&e, &i, ConfiguredNetwork::Regtest, 2).unwrap();
        let index = WalletIndex::new(config).unwrap();
        let d: ExtendedDescriptor = e.parse().unwrap();
        let script = d
            .at_derivation_index(1)
            .unwrap()
            .derived_descriptor(&bitcoin::secp256k1::Secp256k1::verification_only())
            .unwrap()
            .script_pubkey();
        assert_eq!(
            index.classify_script(&script),
            WalletOwnership::External {
                derivation_index: 1
            }
        );
    }
}

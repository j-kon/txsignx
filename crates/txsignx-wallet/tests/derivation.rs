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

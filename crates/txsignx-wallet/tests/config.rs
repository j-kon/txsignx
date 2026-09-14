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

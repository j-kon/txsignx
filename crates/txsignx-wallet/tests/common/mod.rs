#![allow(dead_code)]
use bdk_wallet::descriptor::ExtendedDescriptor;
use bitcoin::{
    NetworkKind, ScriptBuf,
    bip32::{ChainCode, ChildNumber, Fingerprint, Xpub},
    secp256k1::PublicKey,
};
use txsignx_wallet::{ConfiguredNetwork, WalletConfig};
// Public generator point and dummy chain code; no seed or private derivation.
pub fn public_key() -> Xpub {
    Xpub {
        network: NetworkKind::Test,
        depth: 0,
        parent_fingerprint: Fingerprint::default(),
        child_number: ChildNumber::Normal { index: 0 },
        public_key: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
            .parse::<PublicKey>()
            .unwrap(),
        chain_code: ChainCode::from([42; 32]),
    }
}
pub fn descriptors() -> (String, String) {
    let key = public_key();
    (format!("wpkh({key}/0/*)"), format!("wpkh({key}/1/*)"))
}
pub fn config(window: u32) -> WalletConfig {
    let (external, internal) = descriptors();
    WalletConfig::new(&external, &internal, ConfiguredNetwork::Regtest, window).unwrap()
}
pub fn script(keychain: u32, index: u32) -> ScriptBuf {
    let d: ExtendedDescriptor = format!("wpkh({}/{keychain}/*)", public_key())
        .parse()
        .unwrap();
    d.at_derivation_index(index)
        .unwrap()
        .derived_descriptor(&bitcoin::secp256k1::Secp256k1::verification_only())
        .unwrap()
        .script_pubkey()
}

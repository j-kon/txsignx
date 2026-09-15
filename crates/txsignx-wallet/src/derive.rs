use crate::WalletError;
use bdk_wallet::{
    descriptor::ExtendedDescriptor,
    miniscript::{
        TranslatePk, Translator,
        descriptor::{DescriptorPublicKey, SinglePubKey, Wildcard},
    },
};
use bitcoin::{
    PublicKey, ScriptBuf,
    bip32::ChildNumber,
    secp256k1::{Secp256k1, Verification},
};

/// Fallible public-key translation avoids miniscript's assumptions that every
/// BIP32 error other than hardened derivation is cryptographically unreachable.
pub(crate) fn script<C: Verification>(
    descriptor: &ExtendedDescriptor,
    index: u32,
    secp: &Secp256k1<C>,
) -> Result<ScriptBuf, WalletError> {
    struct Derive<'a, C: Verification> {
        index: u32,
        secp: &'a Secp256k1<C>,
    }
    impl<C: Verification> Translator<DescriptorPublicKey, PublicKey, WalletError> for Derive<'_, C> {
        fn pk(&mut self, key: &DescriptorPublicKey) -> Result<PublicKey, WalletError> {
            match key {
                DescriptorPublicKey::Single(key) => Ok(match key.key {
                    SinglePubKey::FullKey(key) => key,
                    SinglePubKey::XOnly(key) => {
                        PublicKey::new(key.public_key(bitcoin::secp256k1::Parity::Even))
                    }
                }),
                DescriptorPublicKey::XPub(key) => {
                    let mut path = key.derivation_path.clone();
                    match key.wildcard {
                        Wildcard::None => {}
                        Wildcard::Unhardened => {
                            path = path.into_child(
                                ChildNumber::from_normal_idx(self.index)
                                    .map_err(|_| WalletError::DerivationFailed)?,
                            );
                        }
                        Wildcard::Hardened => return Err(WalletError::DerivationFailed),
                    }
                    key.xkey
                        .derive_pub(self.secp, &path)
                        .map(|key| PublicKey::new(key.public_key))
                        .map_err(|_| WalletError::DerivationFailed)
                }
                DescriptorPublicKey::MultiXPub(_) => Err(WalletError::MultipathDescriptor),
            }
        }
        bdk_wallet::miniscript::translate_hash_clone!(DescriptorPublicKey, PublicKey, WalletError);
    }
    let derived = descriptor
        .translate_pk(&mut Derive { index, secp })
        .map_err(|_| WalletError::DerivationFailed)?;
    Ok(derived.script_pubkey())
}

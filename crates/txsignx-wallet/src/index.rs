use crate::{ConfiguredNetwork, WalletConfig, WalletError, WalletOwnership};
use bitcoin::{ScriptBuf, secp256k1::Secp256k1};
use std::collections::BTreeMap;
/// Ephemeral script index; deliberately no Debug or serialization of script/key material.
pub struct WalletIndex {
    scripts: BTreeMap<ScriptBuf, WalletOwnership>,
    pub(crate) network: ConfiguredNetwork,
    pub(crate) window: u32,
}
impl WalletIndex {
    pub fn new(config: WalletConfig) -> Result<Self, WalletError> {
        let secp = Secp256k1::verification_only();
        let mut scripts = BTreeMap::new();
        let mut script_bytes = 0usize;
        for (descriptor, internal) in [(&config.external, false), (&config.internal, true)] {
            for derivation_index in 0..config.window {
                let script = crate::derive::script(descriptor, derivation_index, &secp)?;
                script_bytes = script_bytes
                    .checked_add(script.len())
                    .ok_or(WalletError::ResourceLimit)?;
                if script_bytes > 16 * 1024 * 1024 {
                    return Err(WalletError::ResourceLimit);
                }
                let ownership = if internal {
                    WalletOwnership::Internal { derivation_index }
                } else {
                    WalletOwnership::External { derivation_index }
                };
                if scripts.insert(script, ownership).is_some() {
                    return Err(WalletError::AmbiguousScripts);
                }
            }
        }
        Ok(Self {
            scripts,
            network: config.network,
            window: config.window,
        })
    }
    pub fn configured_network(&self) -> ConfiguredNetwork {
        self.network
    }
    pub fn derivation_window(&self) -> u32 {
        self.window
    }
    pub fn classify_script(&self, script: &bitcoin::Script) -> WalletOwnership {
        self.scripts
            .get(script)
            .copied()
            .unwrap_or(WalletOwnership::NoMatchWithinWindow)
    }
}

# Synthetic PSBT v0 fixtures

`psbt-unsigned.b64` and `psbt-partial.b64` are standard padded base64 with a
terminal newline. Regenerate both, in that order, with:

```sh
cargo run --quiet -p txsignx-core --example psbt_fixtures
```

The builder lives in `crates/txsignx-core/tests/psbt_common/mod.rs`, reusing the
Milestone 1 synthetic transaction builder. It clears scriptSig, retains version
2, locktime 42, a dummy `11…11:1` outpoint and sequence `0xfffffffd`, and pays
100,000 sats to a dummy P2PKH script and 50,000 sats to a dummy P2WPKH script.
A witness_utxo supplies 151,000 sats and that P2WPKH script: the arithmetic fee
is 1,000 sats, conditional on this unauthenticated supplied context.

The partial fixture adds the public secp256k1 generator point and a syntactically
encoded ECDSA signature with r and s each equal to 32 bytes of `01`, followed
by SIGHASH_ALL. It also explicitly sets SIGHASH_ALL in the input map. This is
not a verified signature for this transaction. No private key is constructed,
used, or stored. Neither fixture is intended to spend funds.

Additional deterministic test builders exercise Taproot signatures, origins,
and a synthetic xpub assembled directly from a public generator point and
constant dummy chain code. These are not personal wallet data. No network,
wallet, seed, signing operation, or production transaction is involved.

Tests compare the checked-in text with generated serialization and validate
reports and CLI behavior. The legacy/SegWit raw fixtures remain under
`crates/txsignx-core/tests/fixtures/`.

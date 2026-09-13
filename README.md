# TxSignX

Bitcoin transaction and PSBT security preflight engine built in Rust.

## Status

Initial workspace scaffold only. Milestone 1 implementation has not started.
The library has no transaction parsing, analysis, signing, or RPC behavior yet.

## Development

Install a stable Rust toolchain supporting edition 2024, including rustfmt and Clippy.
Run from this repository:

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

`crates/txsignx` is the initial library crate. No third-party Rust dependencies are included.

## Related repositories

- [Web application](https://github.com/j-kon/txsignx-web)
- [Documentation](https://github.com/j-kon/txsignx-docs)

The parent directory is a local container, not a Git repository. The sibling
`txsignx-brand/` directory is local only and must not be committed or pushed.
Do not commit environment files, credentials, seed phrases, mnemonics, private
keys, extended private keys, wallet databases, or API tokens.

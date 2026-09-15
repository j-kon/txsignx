# Milestone 4 — descriptor-aware wallet context

Base: `82edd04c35ac2a85323495f62e53c0eafd366ead`; baseline: 167 tests.

## Design

`txsignx-core` remains factual and independent of BDK, wallet and policy.
`txsignx-wallet` depends on core and BDK's public descriptor functionality.
Policy consumes immutable wallet facts; CLI orchestrates these layers.
No BDK Wallet, signer, persistence or chain backend is instantiated.

Parse only `Descriptor<DescriptorPublicKey>` via `FromStr`, never the
secret-accepting `parse_descriptor` API. Sanitize every dependency error.
Validate descriptor size (64 KiB), rangedness, single-path scope, sanity and
BDK network-kind compatibility before deriving. Require distinct external and
internal descriptors. Reject every duplicate script, including within a keychain.
Reject hardened public suffixes/wildcards; allow hardened origin metadata.
Use a fallible typed public-key translator rather than miniscript definite-key
derivation, to safely return BIP32 depth/derivation errors.
Use BDK Wallet 3.1.0 with only std; its bitcoin ^0.32.8 accepts workspace 0.32.102.
Review the resolved tree and RustSec before committing the dependency.

Cap aggregate key/path work at 200,000 units and derived script bytes at 16 MiB.
Build a private BTreeMap of script bytes to keychain/index for exactly
`0..window`; default 1000, allowed 1..=10000. No PSBT-driven expansion.
Reports contain configured network, window, sorted expected-change indexes and
input/output classifications; never descriptor strings, keys or origins.
Reports are constructed only by the wallet module, with read-only accessors.
Bind reports to the inspection facts used to construct them to reject accidental
cross-transaction reuse by policy without deriving scripts in policy.

Valid core prevout facts are matched; missing/invalid context stays unavailable.
Outputs match scripts without change heuristics. A miss means only
`NoMatchWithinWindow`, never universal non-ownership. Expected change is caller
intent: zero-based indexes, duplicates and out-of-range values rejected.

TG004: valid unmatched input -> HIGH/REVIEW; no duplicate finding for missing or
invalid context. TG005: declared internal change -> no finding; external ->
HIGH/REVIEW; unmatched -> CRITICAL/BLOCK. TG005 without explicit intent is not
evaluated. TG004 is partial when only some inputs have usable context.
Retain evaluated_rules (fully or partially run rules), add rule_evaluations
with evaluated/partially_evaluated/not_evaluated and typed reasons. No-context
preflight decisions/exit codes remain unchanged. Registry becomes 10 active,
4 deferred (TG001/TG006/TG007/TG008). PASS has an explicit incomplete scope note.

## Implementation and verification sequence

1. Public-only configuration boundary and rejection tests.
2. Bounded script derivation, collision and window tests.
3. Immutable input/output classification and public fixture generator.
4. Contextual policy evaluation statuses and TG004/TG005 coverage.
5. Bounded descriptor CLI inputs, sanitized diagnostics and human/JSON reporting.
6. Wallet, policy and CLI regressions including change hijack and collaboration.
7. Full fmt/check/test/Clippy/audit/tree, privacy scan and verification record.
8. Normal incremental commits and normal branch push; no PR.

Use public fixture keys constructed from a public curve point and dummy chain
code, without a seed or private key. Rejection tests alone may construct public
dummy private encodings in test code. Never print them in diagnostics.
File-based descriptor input is recommended: direct args can remain in shell
history/process listings. Files are read with a 64 KiB + 1 sentinel bound.

## Scope

Configured network is explicit, not detected. Testnet-family prefixes cannot
distinguish testnet/testnet4/signet/regtest. No network-truth claim or TG001.
No network I/O, sync, persistence/database, balances/history, signing,
finalization, broadcast, node/RPC, or Milestone 5 work. This is not a
professional security audit.

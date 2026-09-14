# Deterministic policy demo fixtures

These standard-base64 PSBT v0 files use public dummy transaction data derived
from `../psbt-unsigned.b64`. No private keys, seed phrases, wallet xpubs,
personal wallet data, signatures or signing operations are used.

Regenerate the name/base64 TSV with:

```sh
cargo run --quiet -p txsignx-core --example policy_fixtures
```

The generator is `crates/txsignx-core/examples/policy_fixtures.rs`. It changes
only specified dummy values/metadata. Its stdout can be split on the tab and
saved as `<name>.b64` with a terminal newline. It performs no file writes.

| Fixture | Construction / expected default policy |
|---|---|
| pass | Original 150,000 output sats, 151,000 supplied input sats: PASS, no findings |
| absolute-fee | 2,000,000 output sats, 200,000 fee: BLOCK, TG002 only |
| percentage-fee | 10,000 output sats, 2,000 fee: BLOCK, TG003 only |
| 800k-fee | 100,000 output sats, 800,000 fee, 900,000 total inputs: BLOCK / CRITICAL, TG002 + TG003 |
| missing-utxo | Removes witness_utxo: REVIEW, TG010 |
| invalid-utxo | Adds a dummy non-witness transaction with the wrong TXID: BLOCK, TG009 |
| unusual-sighash | Explicit numeric sighash 2 (NONE): REVIEW, TG011 |
| op-return | Changes the 100,000-sat first output to OP_RETURN: BLOCK, TG014 |
| unknown-script | Changes the first output to OP_TRUE, an unrecognized template: REVIEW, TG013 |
| negative-fee | Supplies only 1 input sat against 150,000 output sats: inspection succeeds with negative_fee; preflight returns evaluation error, exit 1 |

Fee fixtures retain two outputs; the second has zero value and a recognized
script. No dust rule is approximated. The input outpoint remains synthetic and
witness-only amounts are supplied context, not authenticated blockchain facts.

PASS is scoped to active development policies, not a global safety claim. The
fee ratio is fee divided by total input value. Defaults are 100,000 maximum
absolute fee sats and 1,000 basis points (10%). Equality does not trigger.

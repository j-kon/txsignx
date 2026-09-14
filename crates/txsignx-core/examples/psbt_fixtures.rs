//! Print deterministic public dummy fixtures; no private keys or signing.
#[path = "../tests/psbt_common/mod.rs"]
mod common;
fn main() {
    println!("{}", common::encode(&common::unsigned()));
    println!("{}", common::encode(&common::partial()));
}

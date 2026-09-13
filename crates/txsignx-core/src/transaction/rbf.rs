/// BIP125 explicit signaling only; inherited signaling and mempool policy are unknown.
pub fn signals_explicit_rbf(sequence: u32) -> bool {
    sequence < 0xffff_fffe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_below_threshold_signal() {
        for sequence in [0, 1, 0x8000_0000, 0xffff_fffd] {
            assert!(signals_explicit_rbf(sequence), "{sequence:#x}");
        }
    }

    #[test]
    fn final_and_locktime_enabled_sequences_do_not_signal() {
        for sequence in [0xffff_fffe, 0xffff_ffff] {
            assert!(!signals_explicit_rbf(sequence), "{sequence:#x}");
        }
    }
}

use bitcoin::psbt::Input;
use serde::Serialize;

/// Field presence only; no signature verification or broadcast readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PsbtSigningState {
    Unsigned,
    PartiallySigned,
    Finalized,
    Mixed,
}

pub(super) fn input_state(input: &Input) -> PsbtSigningState {
    if input.final_script_sig.is_some() || input.final_script_witness.is_some() {
        PsbtSigningState::Finalized
    } else if !input.partial_sigs.is_empty()
        || input.tap_key_sig.is_some()
        || !input.tap_script_sigs.is_empty()
    {
        PsbtSigningState::PartiallySigned
    } else {
        PsbtSigningState::Unsigned
    }
}

pub(super) fn overall(states: impl Iterator<Item = PsbtSigningState>) -> PsbtSigningState {
    let mut finalized = false;
    let mut nonfinal = false;
    let mut partial = false;
    for state in states {
        finalized |= state == PsbtSigningState::Finalized;
        nonfinal |= state != PsbtSigningState::Finalized;
        partial |= state == PsbtSigningState::PartiallySigned;
    }
    match (finalized, nonfinal, partial) {
        (true, true, _) => PsbtSigningState::Mixed,
        (true, false, _) => PsbtSigningState::Finalized,
        (false, _, true) => PsbtSigningState::PartiallySigned,
        _ => PsbtSigningState::Unsigned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{ScriptBuf, Witness};
    #[test]
    fn empty_final_markers_count_and_take_precedence() {
        let mut input = Input::default();
        assert_eq!(input_state(&input), PsbtSigningState::Unsigned);
        input.tap_key_sig = Some(bitcoin::taproot::Signature::from_slice(&[1; 64]).unwrap());
        assert_eq!(input_state(&input), PsbtSigningState::PartiallySigned);
        input.final_script_sig = Some(ScriptBuf::new());
        assert_eq!(input_state(&input), PsbtSigningState::Finalized);
        input.final_script_sig = None;
        input.final_script_witness = Some(Witness::new());
        assert_eq!(input_state(&input), PsbtSigningState::Finalized);
    }
    #[test]
    fn aggregate_states_include_empty_and_mixed() {
        use PsbtSigningState::*;
        for (states, expected) in [
            (vec![], Unsigned),
            (vec![Unsigned, Unsigned], Unsigned),
            (vec![Unsigned, PartiallySigned], PartiallySigned),
            (vec![Finalized, Finalized], Finalized),
            (vec![Finalized, Unsigned], Mixed),
            (vec![PartiallySigned, Finalized], Mixed),
        ] {
            assert_eq!(overall(states.into_iter()), expected);
        }
    }
}

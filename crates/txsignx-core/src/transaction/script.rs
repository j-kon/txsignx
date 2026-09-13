use bitcoin::Script;
use serde::Serialize;

/// Structural script classification, without choosing an address network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ScriptType {
    #[serde(rename = "p2pkh")]
    P2pkh,
    #[serde(rename = "p2sh")]
    P2sh,
    #[serde(rename = "p2wpkh")]
    P2wpkh,
    #[serde(rename = "p2wsh")]
    P2wsh,
    #[serde(rename = "p2tr")]
    P2tr,
    #[serde(rename = "op_return")]
    OpReturn,
    #[serde(rename = "unknown")]
    Unknown,
}

/// Classify a script without executing it or validating a spending condition.
pub fn classify_script(script: &Script) -> ScriptType {
    if script.is_p2pkh() {
        ScriptType::P2pkh
    } else if script.is_p2sh() {
        ScriptType::P2sh
    } else if script.is_p2wpkh() {
        ScriptType::P2wpkh
    } else if script.is_p2wsh() {
        ScriptType::P2wsh
    } else if script.is_p2tr() {
        ScriptType::P2tr
    } else if script.is_op_return() {
        ScriptType::OpReturn
    } else {
        ScriptType::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{ScriptBuf, hex::FromHex};

    fn classify(hex: &str) -> ScriptType {
        classify_script(&ScriptBuf::from_bytes(Vec::from_hex(hex).unwrap()))
    }

    #[test]
    fn p2pkh() {
        assert_eq!(
            classify(&format!("76a914{}88ac", "11".repeat(20))),
            ScriptType::P2pkh
        );
    }

    #[test]
    fn p2sh() {
        assert_eq!(
            classify(&format!("a914{}87", "11".repeat(20))),
            ScriptType::P2sh
        );
    }

    #[test]
    fn p2wpkh() {
        assert_eq!(
            classify(&format!("0014{}", "11".repeat(20))),
            ScriptType::P2wpkh
        );
    }

    #[test]
    fn p2wsh() {
        assert_eq!(
            classify(&format!("0020{}", "11".repeat(32))),
            ScriptType::P2wsh
        );
    }

    #[test]
    fn p2tr() {
        assert_eq!(
            classify(&format!("5120{}", "11".repeat(32))),
            ScriptType::P2tr
        );
    }

    #[test]
    fn op_return() {
        assert_eq!(classify("6a"), ScriptType::OpReturn);
        assert_eq!(classify("6a026869"), ScriptType::OpReturn);
    }

    #[test]
    fn unknown_and_near_miss_scripts() {
        for script in ["", "51", "6b", "006a", "0014", "5120", "76a914", "4cff"] {
            assert_eq!(classify(script), ScriptType::Unknown, "{script}");
        }
        for script in [
            format!("0014{}", "11".repeat(19)),
            format!("5120{}00", "11".repeat(32)),
            format!("5220{}", "11".repeat(32)),
        ] {
            assert_eq!(classify(&script), ScriptType::Unknown);
        }
    }
}

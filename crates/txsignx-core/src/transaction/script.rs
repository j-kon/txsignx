use bitcoin::{Address, Script, hex::DisplayHex, script::Instruction};
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

const MAX_DISASSEMBLY_CHARS: usize = 4096;
const MAX_PUSH_DISPLAY_BYTES: usize = 64;

/// Disassemble a script into safe, bounded opcode and push data representations.
/// Does NOT execute or interpret script conditions. Never panics on malformed scripts.
pub fn disassemble_script(script: &Script) -> String {
    if script.is_empty() {
        return String::new();
    }
    let mut parts = Vec::new();
    let mut total_len = 0;

    for instruction_result in script.instructions() {
        if total_len >= MAX_DISASSEMBLY_CHARS {
            parts.push("[TRUNCATED]".to_string());
            break;
        }
        match instruction_result {
            Ok(Instruction::Op(op)) => {
                let op_str = op.to_string();
                total_len += op_str.len() + 1;
                parts.push(op_str);
            }
            Ok(Instruction::PushBytes(push)) => {
                let bytes = push.as_bytes();
                let len = bytes.len();
                let push_str = if len == 0 {
                    "PUSHBYTES_0".to_string()
                } else if len <= MAX_PUSH_DISPLAY_BYTES {
                    format!("PUSHBYTES_{len} {}", bytes.to_lower_hex_string())
                } else {
                    format!(
                        "PUSHBYTES_{len} {}...",
                        bytes[..MAX_PUSH_DISPLAY_BYTES].to_lower_hex_string()
                    )
                };
                total_len += push_str.len() + 1;
                parts.push(push_str);
            }
            Err(_) => {
                parts.push("[MALFORMED_OPCODE]".to_string());
                break;
            }
        }
    }
    parts.join(" ")
}

/// Derive a standard Bitcoin address from an output script for the given explicit network.
/// Returns None for OP_RETURN, nonstandard scripts, or scripts not mapping to standard addresses.
pub fn derive_address(script: &Script, network: bitcoin::Network) -> Option<String> {
    Address::from_script(script, network)
        .ok()
        .map(|a| a.to_string())
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

    #[test]
    fn disassemble_standard_p2pkh() {
        let script = ScriptBuf::from_bytes(
            Vec::from_hex(&format!("76a914{}88ac", "11".repeat(20))).unwrap(),
        );
        assert_eq!(
            disassemble_script(&script),
            format!(
                "OP_DUP OP_HASH160 PUSHBYTES_20 {} OP_EQUALVERIFY OP_CHECKSIG",
                "11".repeat(20)
            )
        );
    }

    #[test]
    fn disassemble_segwit_and_op_return() {
        let wpkh =
            ScriptBuf::from_bytes(Vec::from_hex(&format!("0014{}", "22".repeat(20))).unwrap());
        assert_eq!(
            disassemble_script(&wpkh),
            format!("PUSHBYTES_0 PUSHBYTES_20 {}", "22".repeat(20))
        );

        let op_ret = ScriptBuf::from_bytes(Vec::from_hex("6a04deadbeef").unwrap());
        assert_eq!(
            disassemble_script(&op_ret),
            "OP_RETURN PUSHBYTES_4 deadbeef"
        );
    }

    #[test]
    fn disassemble_malformed_script_does_not_panic() {
        // Truncated push data: 0x4c specifies OP_PUSHDATA1 with length, but trailing bytes missing
        let malformed = ScriptBuf::from_bytes(vec![0x4c]);
        assert_eq!(disassemble_script(&malformed), "[MALFORMED_OPCODE]");
    }

    #[test]
    fn derive_address_for_standard_types_and_networks() {
        let p2pkh = ScriptBuf::from_bytes(
            Vec::from_hex(&format!("76a914{}88ac", "11".repeat(20))).unwrap(),
        );
        let addr_main = derive_address(&p2pkh, bitcoin::Network::Bitcoin).unwrap();
        assert!(addr_main.starts_with('1'));
        let addr_regtest = derive_address(&p2pkh, bitcoin::Network::Regtest).unwrap();
        assert!(addr_regtest.starts_with('m') || addr_regtest.starts_with('n'));

        let p2wpkh =
            ScriptBuf::from_bytes(Vec::from_hex(&format!("0014{}", "22".repeat(20))).unwrap());
        let addr_wpkh = derive_address(&p2wpkh, bitcoin::Network::Bitcoin).unwrap();
        assert!(addr_wpkh.starts_with("bc1q"));
        let addr_wpkh_regtest = derive_address(&p2wpkh, bitcoin::Network::Regtest).unwrap();
        assert!(addr_wpkh_regtest.starts_with("bcrt1q"));

        let op_ret = ScriptBuf::from_bytes(Vec::from_hex("6a04deadbeef").unwrap());
        assert!(derive_address(&op_ret, bitcoin::Network::Bitcoin).is_none());

        let unknown = ScriptBuf::from_bytes(vec![0x51, 0x52]);
        assert!(derive_address(&unknown, bitcoin::Network::Bitcoin).is_none());
    }
}

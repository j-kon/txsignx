use std::io::{self, IsTerminal, Write};

pub const LOGO_LINES: &[&str] = &[
    "████████╗██╗  ██╗███████╗██╗ ██████╗ ███╗   ██╗██╗  ██╗",
    "╚══██╔══╝╚██╗██╔╝██╔════╝██║██╔════╝ ████╗  ██║╚██╗██╔╝",
    "   ██║    ╚███╔╝ ███████╗██║██║  ███╗██╔██╗ ██║ ╚███╔╝",
    "   ██║    ██╔██╗ ╚════██║██║██║   ██║██║╚██╗██║ ██╔██╗",
    "   ██║   ██╔╝ ██╗███████║██║╚██████╔╝██║ ╚████║██╔╝ ██╗",
    "   ╚═╝   ╚═╝  ╚═╝╚══════╝╚═╝ ╚═════╝ ╚═╝  ╚═══╝╚═╝  ╚═╝",
];

const LOGO_PARTS: &[(&str, &str)] = &[
    (
        "████████╗██╗  ██╗███████╗██╗ ██████╗ ███╗   ██╗",
        "██╗  ██╗",
    ),
    (
        "╚══██╔══╝╚██╗██╔╝██╔════╝██║██╔════╝ ████╗  ██║",
        "╚██╗██╔╝",
    ),
    ("   ██║    ╚███╔╝ ███████╗██║██║  ███╗██╔██╗ ██║ ", "╚███╔╝"),
    ("   ██║    ██╔██╗ ╚════██║██║██║   ██║██║╚██╗██║ ", "██╔██╗"),
    (
        "   ██║   ██╔╝ ██╗███████║██║╚██████╔╝██║ ╚████║",
        "██╔╝ ██╗",
    ),
    (
        "   ╚═╝   ╚═╝  ╚═╝╚══════╝╚═╝ ╚═════╝ ╚═╝  ╚═══╝",
        "╚═╝  ╚═╝",
    ),
];

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BOLD_WHITE: &str = "\x1b[1;97m";
const ANSI_BITCOIN_ORANGE: &str = "\x1b[1;38;2;247;147;26m";
const ANSI_MUTED: &str = "\x1b[90m";
const ANSI_BOLD: &str = "\x1b[1m";

pub fn should_use_color() -> bool {
    io::stdout().is_terminal()
        && match std::env::var_os("NO_COLOR") {
            None => true,
            Some(val) => val.is_empty(),
        }
}

pub fn write_banner(out: &mut impl Write, use_color: bool, version: &str) -> io::Result<()> {
    if use_color {
        for (prefix, suffix) in LOGO_PARTS {
            writeln!(
                out,
                "{ANSI_BOLD_WHITE}{prefix}{ANSI_BITCOIN_ORANGE}{suffix}{ANSI_RESET}"
            )?;
        }
        writeln!(out)?;
        writeln!(out, "Bitcoin transaction security before signing.")?;
        writeln!(
            out,
            "{ANSI_MUTED}Inspect. Verify. Sign with Confidence.{ANSI_RESET}"
        )?;
        writeln!(out)?;
        writeln!(out, "{ANSI_MUTED}v{version}{ANSI_RESET}")?;
        writeln!(out)?;
        writeln!(out, "{ANSI_BOLD}Quick start:{ANSI_RESET}")?;
        writeln!(out, "  txsignx policy list")?;
        writeln!(out, "  txsignx tx inspect <RAW_TX_HEX>")?;
        writeln!(out, "  txsignx psbt inspect --file payment.b64")?;
        writeln!(out, "  txsignx psbt preflight --file payment.b64")?;
        writeln!(out)?;
        writeln!(out, "Run `txsignx --help` for all commands.")?;
    } else {
        for line in LOGO_LINES {
            writeln!(out, "{line}")?;
        }
        writeln!(out)?;
        writeln!(out, "Bitcoin transaction security before signing.")?;
        writeln!(out, "Inspect. Verify. Sign with Confidence.")?;
        writeln!(out)?;
        writeln!(out, "v{version}")?;
        writeln!(out)?;
        writeln!(out, "Quick start:")?;
        writeln!(out, "  txsignx policy list")?;
        writeln!(out, "  txsignx tx inspect <RAW_TX_HEX>")?;
        writeln!(out, "  txsignx psbt inspect --file payment.b64")?;
        writeln!(out, "  txsignx psbt preflight --file payment.b64")?;
        writeln!(out)?;
        writeln!(out, "Run `txsignx --help` for all commands.")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_parts_match_approved_geometry() {
        assert_eq!(LOGO_PARTS.len(), LOGO_LINES.len());
        for (i, &(prefix, suffix)) in LOGO_PARTS.iter().enumerate() {
            let combined = format!("{prefix}{suffix}");
            assert_eq!(combined, LOGO_LINES[i], "row {i} geometry mismatch");
        }
    }

    #[test]
    fn plain_banner_contains_no_ansi_escapes() {
        let mut buf = Vec::new();
        write_banner(&mut buf, false, "0.1.0").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            !s.contains('\x1b'),
            "plain banner must not have ANSI escapes"
        );
        assert!(s.contains("████████╗"));
        assert!(s.contains("Bitcoin transaction security before signing."));
        assert!(s.contains("Inspect. Verify. Sign with Confidence."));
        assert!(s.contains("v0.1.0"));
        assert!(s.contains("Quick start:"));
        assert!(s.contains("txsignx policy list"));
        assert!(s.contains("Run `txsignx --help` for all commands."));
    }

    #[test]
    fn color_banner_contains_branding_and_shape() {
        let mut buf = Vec::new();
        write_banner(&mut buf, true, "0.1.0").unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains('\x1b'), "color banner must have ANSI escapes");
        assert!(s.contains("247;147;26m"), "must contain Bitcoin Orange");
        assert!(s.contains("v0.1.0"));
    }
}

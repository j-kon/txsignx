use std::io::{self, Write};
use txsignx_policy::{POLICY_SCOPE, RuleCatalog, Severity};

use crate::style::*;

fn severity_color(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => ANSI_CRITICAL,
        Severity::High => ANSI_HIGH,
        Severity::Medium => ANSI_MEDIUM,
        Severity::Low => ANSI_INFO,
        Severity::Info => ANSI_INFO,
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "CRITICAL",
        Severity::High => "HIGH",
        Severity::Medium => "MEDIUM",
        Severity::Low => "LOW",
        Severity::Info => "INFO",
    }
}

pub fn write_policy_list(
    out: &mut impl Write,
    catalog: &RuleCatalog,
    use_color: bool,
) -> io::Result<()> {
    if use_color {
        writeln!(
            out,
            "{ANSI_BOLD_WHITE}TxSignX Development Policy Rules{ANSI_RESET}"
        )?;
        writeln!(
            out,
            "{ANSI_MUTED}------------------------------------{ANSI_RESET}"
        )?;
        writeln!(out)?;

        for (i, rule) in catalog.active_rules.iter().enumerate() {
            if i > 0 {
                writeln!(out)?;
            }
            let color = severity_color(rule.default_severity);
            let label = severity_label(rule.default_severity);
            writeln!(
                out,
                "{ANSI_BOLD_WHITE}{}{ANSI_RESET}  {color}{label}{ANSI_RESET}  {ANSI_BOLD}{}{ANSI_RESET}\n  {}\n  {ANSI_MUTED}Required context:{ANSI_RESET} {}",
                rule.code,
                rule.title,
                rule.description,
                rule.required_context.join(", ")
            )?;
        }

        writeln!(out)?;
        writeln!(
            out,
            "{ANSI_BOLD_WHITE}RESERVED / DEFERRED — not evaluated{ANSI_RESET}"
        )?;
        writeln!(
            out,
            "{ANSI_MUTED}------------------------------------{ANSI_RESET}"
        )?;
        writeln!(out)?;

        for (i, rule) in catalog.deferred_rules.iter().enumerate() {
            if i > 0 {
                writeln!(out)?;
            }
            writeln!(
                out,
                "{ANSI_BOLD_WHITE}{}{ANSI_RESET}  {ANSI_DEFERRED}DEFERRED{ANSI_RESET}  {ANSI_BOLD}{}{ANSI_RESET}\n  {ANSI_MUTED}Requires:{ANSI_RESET} {}",
                rule.code,
                rule.title,
                rule.required_context.join(", ")
            )?;
        }

        writeln!(out)?;
        writeln!(out, "{ANSI_MUTED}{POLICY_SCOPE}{ANSI_RESET}")?;
    } else {
        writeln!(out, "TxSignX Development Policy Rules")?;
        writeln!(out, "------------------------------------")?;
        writeln!(out)?;

        for (i, rule) in catalog.active_rules.iter().enumerate() {
            if i > 0 {
                writeln!(out)?;
            }
            writeln!(
                out,
                "{}  {}  {}\n  {}\n  Required context: {}",
                rule.code,
                severity_label(rule.default_severity),
                rule.title,
                rule.description,
                rule.required_context.join(", ")
            )?;
        }

        writeln!(out)?;
        writeln!(out, "RESERVED / DEFERRED — not evaluated")?;
        writeln!(out, "------------------------------------")?;
        writeln!(out)?;

        for (i, rule) in catalog.deferred_rules.iter().enumerate() {
            if i > 0 {
                writeln!(out)?;
            }
            writeln!(
                out,
                "{}  DEFERRED  {}\n  Requires: {}",
                rule.code,
                rule.title,
                rule.required_context.join(", ")
            )?;
        }

        writeln!(out)?;
        writeln!(out, "{POLICY_SCOPE}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use txsignx_policy::rule_catalog;

    #[test]
    fn plain_output_contains_no_ansi_escapes() {
        let catalog = rule_catalog().unwrap();
        let mut buf = Vec::new();
        write_policy_list(&mut buf, &catalog, false).unwrap();
        let text = String::from_utf8(buf).unwrap();

        assert!(
            !text.contains('\x1b'),
            "plain policy list must have no ANSI escapes"
        );
        assert!(text.contains("TxSignX Development Policy Rules"));
        assert!(text.contains("------------------------------------"));
        assert!(text.contains("RESERVED / DEFERRED — not evaluated"));
        assert!(text.contains("TG001"));
        assert!(text.contains("CRITICAL"));
        assert!(text.contains("TG004"));
        assert!(text.contains("HIGH"));
        assert!(text.contains("TG013"));
        assert!(text.contains("MEDIUM"));
        assert!(text.contains("TG012"));
        assert!(text.contains("INFO"));
        assert!(text.contains("TG007"));
        assert!(text.contains("DEFERRED"));
        assert!(text.contains(POLICY_SCOPE));
    }

    #[test]
    fn color_output_contains_ansi_escapes_and_all_severity_colors() {
        let catalog = rule_catalog().unwrap();
        let mut buf = Vec::new();
        write_policy_list(&mut buf, &catalog, true).unwrap();
        let text = String::from_utf8(buf).unwrap();

        assert!(
            text.contains('\x1b'),
            "color policy list must have ANSI escapes"
        );
        // Headings
        assert!(text.contains(ANSI_BOLD_WHITE));
        // Severity colors
        assert!(
            text.contains(ANSI_CRITICAL),
            "must contain red for CRITICAL"
        );
        assert!(text.contains(ANSI_HIGH), "must contain orange for HIGH");
        assert!(text.contains(ANSI_MEDIUM), "must contain blue for MEDIUM");
        assert!(text.contains(ANSI_INFO), "must contain cyan for INFO");
        assert!(
            text.contains(ANSI_DEFERRED),
            "must contain muted for DEFERRED"
        );
        // Rule codes
        assert!(text.contains("TG001"));
        assert!(text.contains("TG007"));
        assert!(text.contains("TG017"));
    }

    #[test]
    fn spacing_separates_rule_blocks() {
        let catalog = rule_catalog().unwrap();
        let mut buf = Vec::new();
        write_policy_list(&mut buf, &catalog, false).unwrap();
        let text = String::from_utf8(buf).unwrap();

        // Check that consecutive rules are separated by blank lines
        assert!(
            text.contains("\n\nTG002"),
            "rules should be separated by an empty line"
        );
        assert!(
            text.contains("\n\nTG003"),
            "rules should be separated by an empty line"
        );
        assert!(
            text.contains("\n\nTG008"),
            "deferred rules should be separated by an empty line"
        );
    }
}

use std::io::{self, IsTerminal};

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_BOLD: &str = "\x1b[1m";
pub const ANSI_BOLD_WHITE: &str = "\x1b[1;97m";
pub const ANSI_BITCOIN_ORANGE: &str = "\x1b[1;38;2;247;147;26m";
pub const ANSI_MUTED: &str = "\x1b[90m";

// Severity label colors for interactive terminals
pub const ANSI_CRITICAL: &str = "\x1b[1;31m"; // Red
pub const ANSI_HIGH: &str = "\x1b[1;38;2;247;147;26m"; // Bitcoin Orange / Yellow
pub const ANSI_MEDIUM: &str = "\x1b[1;94m"; // Bright Blue
pub const ANSI_INFO: &str = "\x1b[1;36m"; // Cyan
pub const ANSI_DEFERRED: &str = "\x1b[90m"; // Muted Gray

/// Returns `true` if stdout is connected to a terminal and `NO_COLOR` is not set.
pub fn should_use_color() -> bool {
    io::stdout().is_terminal()
        && match std::env::var_os("NO_COLOR") {
            None => true,
            Some(val) => val.is_empty(),
        }
}

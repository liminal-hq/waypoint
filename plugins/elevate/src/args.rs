// Builds the command line the helper is started with on Windows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::pipe_names::PipeNames;

// These flag names are a contract with the helper, which parses the same words.
pub const FLAG_PIPE_TO: &str = "--pipe-to";
pub const FLAG_PIPE_FROM: &str = "--pipe-from";
pub const FLAG_TOKEN: &str = "--token";
pub const FLAG_PARENT: &str = "--parent";

/// Quotes one argument so that `CommandLineToArgvW` and the Rust and C runtimes read it back as it is.
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\x0b', '"']) {
        return arg.to_string();
    }
    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                // Backslashes before a quote are doubled, and the quote itself is escaped.
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            c => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push(c);
                backslashes = 0;
            }
        }
    }
    // Backslashes before the closing quote are doubled so they do not escape it.
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

/// The parameters for `ShellExecuteExW`: both pipe names, the launch token and the app's process id. The token is on a command line that other processes of the same user can read; that residual risk is accepted, because such a process can already act as the person, and the pipe's access list, the client process checks and the one-launch token still stop everything else.
pub fn helper_arguments(names: &PipeNames, token: &str, parent_pid: u32) -> String {
    [
        FLAG_PIPE_TO,
        &quote_arg(&names.to),
        FLAG_PIPE_FROM,
        &quote_arg(&names.from),
        FLAG_TOKEN,
        &quote_arg(token),
        FLAG_PARENT,
        &parent_pid.to_string(),
    ]
    .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_arguments_stay_bare() {
        assert_eq!(quote_arg(r"\\.\pipe\a-b-to"), r"\\.\pipe\a-b-to");
        assert_eq!(quote_arg("abc123"), "abc123");
    }

    #[test]
    fn spaces_quotes_and_trailing_backslashes_are_quoted() {
        assert_eq!(quote_arg(""), "\"\"");
        assert_eq!(quote_arg("a b"), "\"a b\"");
        assert_eq!(quote_arg(r"C:\Program Files\x"), r#""C:\Program Files\x""#);
        assert_eq!(quote_arg(r"a b\"), r#""a b\\""#);
        assert_eq!(quote_arg(r#"a"b"#), r#""a\"b""#);
        assert_eq!(quote_arg(r#"a\"b"#), r#""a\\\"b""#);
    }

    #[test]
    fn the_command_line_names_both_pipes_the_token_and_the_parent() {
        let names = PipeNames {
            to: r"\\.\pipe\p-1-to".into(),
            from: r"\\.\pipe\p-1-from".into(),
        };
        assert_eq!(
            helper_arguments(&names, "abcd", 4321),
            r"--pipe-to \\.\pipe\p-1-to --pipe-from \\.\pipe\p-1-from --token abcd --parent 4321"
        );
    }
}

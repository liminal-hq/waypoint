// Reads the helper's command line with the standard library only
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// These flag names are a contract with the plugin that starts the helper on Windows, which builds the same words.
const FLAG_PIPE_TO: &str = "--pipe-to";
const FLAG_PIPE_FROM: &str = "--pipe-from";
const FLAG_TOKEN: &str = "--token";
const FLAG_PARENT: &str = "--parent";

/// How the helper was asked to talk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// On standard input and output, as `pkexec` starts it.
    Stdio,
    /// Over two named pipes, as the Windows launcher starts it.
    Pipes(PipeArgs),
}

#[derive(Clone, PartialEq, Eq)]
pub struct PipeArgs {
    /// The pipe the app writes and the helper reads.
    pub to: String,
    /// The pipe the helper writes and the app reads.
    pub from: String,
    /// The per-launch token the helper presents first.
    pub token: String,
    /// The process id of the app that made the pipes.
    pub parent: u32,
}

// The token and the pipe names stay out of any debug output.
impl std::fmt::Debug for PipeArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipeArgs")
            .field("parent", &self.parent)
            .finish_non_exhaustive()
    }
}

/// The command line was not one the helper understands. Never carries the text of an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageError;

/// Reads the arguments after the program name. None means standard input and output; all four pipe flags together mean pipes; anything else (an unknown flag, a missing or repeated value, a partial set) is a [`UsageError`].
pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Mode, UsageError> {
    let mut to = None;
    let mut from = None;
    let mut token = None;
    let mut parent = None;
    let mut any = false;
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        any = true;
        let slot = match flag.as_str() {
            FLAG_PIPE_TO => &mut to,
            FLAG_PIPE_FROM => &mut from,
            FLAG_TOKEN => &mut token,
            FLAG_PARENT => &mut parent,
            _ => return Err(UsageError),
        };
        let value = args.next().ok_or(UsageError)?;
        if slot.replace(value).is_some() {
            return Err(UsageError);
        }
    }
    if !any {
        return Ok(Mode::Stdio);
    }
    match (to, from, token, parent) {
        (Some(to), Some(from), Some(token), Some(parent)) if !token.is_empty() => {
            Ok(Mode::Pipes(PipeArgs {
                to,
                from,
                token,
                parent: parent.parse().map_err(|_| UsageError)?,
            }))
        }
        _ => Err(UsageError),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Mode, UsageError> {
        parse(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn no_arguments_mean_standard_input_and_output() {
        assert_eq!(parse_strs(&[]), Ok(Mode::Stdio));
    }

    #[test]
    fn all_four_flags_in_any_order_mean_pipes() {
        let expected = Mode::Pipes(PipeArgs {
            to: r"\\.\pipe\p-to".into(),
            from: r"\\.\pipe\p-from".into(),
            token: "abcd".into(),
            parent: 4321,
        });
        assert_eq!(
            parse_strs(&[
                "--pipe-to",
                r"\\.\pipe\p-to",
                "--pipe-from",
                r"\\.\pipe\p-from",
                "--token",
                "abcd",
                "--parent",
                "4321"
            ]),
            Ok(expected.clone())
        );
        assert_eq!(
            parse_strs(&[
                "--parent",
                "4321",
                "--token",
                "abcd",
                "--pipe-from",
                r"\\.\pipe\p-from",
                "--pipe-to",
                r"\\.\pipe\p-to"
            ]),
            Ok(expected)
        );
    }

    #[test]
    fn anything_else_is_a_usage_error() {
        let bad: [&[&str]; 9] = [
            &["--help"],
            &["stray"],
            &["--pipe-to"],
            &["--pipe-to", "a", "--pipe-from", "b", "--token", "t"],
            &[
                "--pipe-to",
                "a",
                "--pipe-from",
                "b",
                "--token",
                "t",
                "--parent",
                "x",
            ],
            &[
                "--pipe-to",
                "a",
                "--pipe-from",
                "b",
                "--token",
                "t",
                "--parent",
                "-1",
            ],
            &[
                "--pipe-to",
                "a",
                "--pipe-to",
                "c",
                "--pipe-from",
                "b",
                "--token",
                "t",
                "--parent",
                "1",
            ],
            &[
                "--pipe-to",
                "a",
                "--pipe-from",
                "b",
                "--token",
                "",
                "--parent",
                "1",
            ],
            &[
                "--pipe-to",
                "a",
                "--pipe-from",
                "b",
                "--token",
                "t",
                "--parent",
                "1",
                "extra",
            ],
        ];
        for args in bad {
            assert_eq!(parse_strs(args), Err(UsageError), "{args:?}");
        }
    }

    #[test]
    fn the_debug_text_hides_the_token_and_the_names() {
        let Ok(Mode::Pipes(args)) = parse_strs(&[
            "--pipe-to",
            "secret-to",
            "--pipe-from",
            "secret-from",
            "--token",
            "secret-token",
            "--parent",
            "7",
        ]) else {
            panic!("not pipes");
        };
        let text = format!("{args:?}");
        assert!(!text.contains("secret"), "{text}");
    }
}

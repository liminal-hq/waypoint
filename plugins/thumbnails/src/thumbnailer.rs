// Parses `*.thumbnailer` files, finds the installed ones and expands their `Exec` command line
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::path::{Path, PathBuf};

/// One external thumbnailer, as `[Thumbnailer Entry]` of a `*.thumbnailer` file describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnailer {
    /// The file's name without `.thumbnailer`.
    pub id: String,
    /// A program that must exist for the thumbnailer to be usable.
    pub try_exec: Option<String>,
    /// The command line, with `%i`, `%o`, `%s`, `%u` and `%%` still in it.
    pub exec: String,
    pub mime_types: Vec<String>,
}

/// The values the codes of an `Exec` line stand for.
#[derive(Debug, Clone, Copy)]
pub struct ExecValues<'a> {
    pub input: &'a Path,
    pub uri: &'a str,
    pub output: &'a Path,
    pub size: u32,
}

impl Thumbnailer {
    /// Parses the text of a `.thumbnailer` file; `None` if it has no `[Thumbnailer Entry]` with an `Exec` and at least one `MimeType`.
    pub fn parse(id: &str, text: &str) -> Option<Thumbnailer> {
        let mut in_entry = false;
        let (mut try_exec, mut exec, mut mime_types) = (None, None, Vec::new());
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                in_entry = line == "[Thumbnailer Entry]";
                continue;
            }
            if !in_entry {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "TryExec" if !value.is_empty() => try_exec = Some(value.to_string()),
                "Exec" if !value.is_empty() => exec = Some(value.to_string()),
                "MimeType" => {
                    mime_types = value
                        .split(';')
                        .map(str::trim)
                        .filter(|mime| !mime.is_empty())
                        .map(str::to_string)
                        .collect();
                }
                _ => {}
            }
        }
        if mime_types.is_empty() {
            return None;
        }
        Some(Thumbnailer {
            id: id.to_string(),
            try_exec,
            exec: exec?,
            mime_types,
        })
    }

    pub fn handles(&self, mime: &str) -> bool {
        self.mime_types.iter().any(|candidate| candidate == mime)
    }

    /// The program and arguments to run: the `Exec` line split into words by the Desktop Entry quoting rules, then each code replaced (`%i` input path, `%u` input URI, `%o` output path, `%s` size, `%%` a percent sign). Values are never re-split, so a name with spaces stays one argument. `None` for an empty command.
    pub fn command(&self, values: &ExecValues<'_>) -> Option<Vec<String>> {
        let words = split_exec(&self.exec);
        let argv: Vec<String> = words.iter().map(|word| expand_word(word, values)).collect();
        (!argv.is_empty() && !argv[0].is_empty()).then_some(argv)
    }
}

/// Splits an `Exec` value into words: spaces separate, double quotes group, and inside quotes a backslash escapes `"`, `` ` ``, `$` and `\`.
pub fn split_exec(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => match chars.peek() {
                Some(&next) if matches!(next, '"' | '`' | '$' | '\\') => {
                    current.push(next);
                    chars.next();
                }
                _ => current.push('\\'),
            },
            ' ' | '\t' if !quoted => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            other => {
                current.push(other);
                started = true;
            }
        }
    }
    if started {
        words.push(current);
    }
    words
}

fn expand_word(word: &str, values: &ExecValues<'_>) -> String {
    let mut out = String::with_capacity(word.len() + 16);
    let mut chars = word.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('i') => out.push_str(&values.input.to_string_lossy()),
            Some('o') => out.push_str(&values.output.to_string_lossy()),
            Some('u') => out.push_str(values.uri),
            Some('s') => out.push_str(&values.size.to_string()),
            Some('%') => out.push('%'),
            // An unknown code is dropped, as the Desktop Entry specification says for deprecated ones.
            Some(_) | None => {}
        }
    }
    out
}

/// Finds a program the way `TryExec` means it: an absolute path must be an executable file; a bare name is looked for in `path_dirs`.
pub fn program_exists(program: &str, path_dirs: &[PathBuf]) -> bool {
    let candidate = Path::new(program);
    if candidate.is_absolute() || program.contains('/') {
        return is_executable(candidate);
    }
    path_dirs
        .iter()
        .any(|dir| is_executable(&dir.join(program)))
}

/// The program to run for `program`: itself when it has a path, else the first match in `path_dirs`.
pub fn resolve_program(program: &str, path_dirs: &[PathBuf]) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(PathBuf::from(program));
    }
    path_dirs
        .iter()
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Reads every `*.thumbnailer` file in `{dir}/thumbnailers` for each data directory, in order. A file name found in an earlier directory hides the same name in a later one; a thumbnailer whose `TryExec` program is missing is left out. Within a directory the order is by file name, so the result is the same on every run.
pub fn discover(data_dirs: &[PathBuf], path_dirs: &[PathBuf]) -> Vec<Thumbnailer> {
    let mut seen = std::collections::HashSet::new();
    let mut found = Vec::new();
    for dir in data_dirs {
        let Ok(read) = fs::read_dir(dir.join("thumbnailers")) else {
            continue;
        };
        let mut files: Vec<PathBuf> = read
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "thumbnailer"))
            .collect();
        files.sort();
        for path in files {
            let Some(id) = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
            else {
                continue;
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            let Some(thumbnailer) = fs::read_to_string(&path)
                .ok()
                .and_then(|text| Thumbnailer::parse(&id, &text))
            else {
                continue;
            };
            let usable = thumbnailer
                .try_exec
                .as_deref()
                .is_none_or(|program| program_exists(program, path_dirs));
            if usable {
                found.push(thumbnailer);
            }
        }
    }
    found
}

#[cfg(test)]
mod tests;

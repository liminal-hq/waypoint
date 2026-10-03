// Finds a file's MIME type by matching its name against the shared MIME database's globs, never by reading the file
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::path::PathBuf;

/// One glob of the database: `weight:mime:glob[:flags]` in a `globs2` file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    weight: u32,
    mime: String,
    glob: String,
    case_sensitive: bool,
}

/// The name globs of the MIME database, in the order `shared-mime-info` resolves them: the highest weight, then the longest glob.
#[derive(Debug, Clone, Default)]
pub struct MimeDb {
    entries: Vec<Entry>,
}

impl MimeDb {
    /// Parses the text of a `globs2` file. Comments, blank lines, `__NOGLOBS__` markers and malformed lines are skipped.
    pub fn parse(text: &str) -> MimeDb {
        let entries = text
            .lines()
            .filter_map(|line| {
                let line = line.trim_end();
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                let mut parts = line.splitn(4, ':');
                let weight = parts.next()?.trim().parse().ok()?;
                let mime = parts.next()?.to_string();
                let glob = parts.next()?.to_string();
                if glob.is_empty() || mime.is_empty() || mime == "__NOGLOBS__" {
                    return None;
                }
                let case_sensitive = parts
                    .next()
                    .is_some_and(|flags| flags.split(',').any(|flag| flag == "cs"));
                Some(Entry {
                    weight,
                    mime,
                    glob,
                    case_sensitive,
                })
            })
            .collect();
        MimeDb { entries }
    }

    /// Reads and merges `mime/globs2` from every data directory that has one, as `shared-mime-info` does: `~/.local/share/mime` only adds the user's own types to the system's, it does not replace them. On the same weight and glob length an earlier directory wins. With no database anywhere, the small built-in table of the kinds of file the plugin has generators for.
    pub fn load(data_dirs: &[PathBuf]) -> MimeDb {
        let mut entries = Vec::new();
        for dir in data_dirs {
            if let Ok(text) = fs::read_to_string(dir.join("mime/globs2")) {
                entries.extend(MimeDb::parse(&text).entries);
            }
        }
        if entries.is_empty() {
            return MimeDb::fallback();
        }
        MimeDb { entries }
    }

    /// The kinds of file the built-in generator decodes, plus PDF, so a system with no MIME database still works.
    pub fn fallback() -> MimeDb {
        const TABLE: &[(&str, &str)] = &[
            ("*.png", "image/png"),
            ("*.jpg", "image/jpeg"),
            ("*.jpeg", "image/jpeg"),
            ("*.jpe", "image/jpeg"),
            ("*.gif", "image/gif"),
            ("*.webp", "image/webp"),
            ("*.bmp", "image/bmp"),
            ("*.tif", "image/tiff"),
            ("*.tiff", "image/tiff"),
            ("*.ico", "image/vnd.microsoft.icon"),
            ("*.pdf", "application/pdf"),
        ];
        MimeDb {
            entries: TABLE
                .iter()
                .map(|(glob, mime)| Entry {
                    weight: 50,
                    mime: (*mime).to_string(),
                    glob: (*glob).to_string(),
                    case_sensitive: false,
                })
                .collect(),
        }
    }

    /// The MIME type of a file called `name` (the name only, with no folder), or `None`.
    pub fn mime_of(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.entries
            .iter()
            .filter(|entry| {
                let subject = if entry.case_sensitive { name } else { &lower };
                if entry.case_sensitive {
                    glob_matches(&entry.glob, subject)
                } else {
                    glob_matches(&entry.glob.to_lowercase(), subject)
                }
            })
            .enumerate()
            // The earlier entry wins a tie, so the first data directory takes precedence.
            .max_by_key(|(at, entry)| (entry.weight, entry.glob.len(), std::cmp::Reverse(*at)))
            .map(|(_, entry)| entry)
            .map(|entry| entry.mime.as_str())
    }
}

/// Matches `name` against a shell-style glob: `*` for any run of characters, `?` for one, `[a-z]` and `[!a-z]` for a class, and `\` to escape.
pub fn glob_matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    matches_from(&pattern, &name)
}

fn matches_from(pattern: &[char], name: &[char]) -> bool {
    let (mut p, mut n) = (0, 0);
    // Where to resume after the last `*`: the pattern just past it, and the name position it has consumed up to.
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p + 1, n));
                p += 1;
                continue;
            }
            Some(_) => {
                if let Some(next) = match_one(pattern, p, name[n]) {
                    p = next;
                    n += 1;
                    continue;
                }
            }
            None => {}
        }
        match star {
            Some((resume, consumed)) => {
                star = Some((resume, consumed + 1));
                p = resume;
                n = consumed + 1;
            }
            None => return false,
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

/// Matches one character against the pattern item at `p` (not `*`), returning where the next item starts.
fn match_one(pattern: &[char], p: usize, c: char) -> Option<usize> {
    match pattern[p] {
        '?' => Some(p + 1),
        '\\' if p + 1 < pattern.len() => (pattern[p + 1] == c).then_some(p + 2),
        '[' => match_class(pattern, p, c),
        literal => (literal == c).then_some(p + 1),
    }
}

fn match_class(pattern: &[char], start: usize, c: char) -> Option<usize> {
    let mut i = start + 1;
    let negate = matches!(pattern.get(i), Some('!' | '^'));
    if negate {
        i += 1;
    }
    let mut matched = false;
    let mut first = true;
    loop {
        let item = *pattern.get(i)?;
        if item == ']' && !first {
            return (matched != negate).then_some(i + 1);
        }
        first = false;
        if pattern.get(i + 1) == Some(&'-') && pattern.get(i + 2).is_some_and(|end| *end != ']') {
            let end = pattern[i + 2];
            matched |= item <= c && c <= end;
            i += 3;
        } else {
            matched |= item == c;
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests;

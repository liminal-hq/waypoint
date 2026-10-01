// The rules of a batch rename: what each rule does to a name, and applying a stack of them to a list
// of entries. It is pure (A47): the entries, their times and "now" are passed in, and nothing reads
// a file system or a clock. Clashes between the results are `rename_clash`'s concern.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Rules run in order, each on the result of the one before. A name is held as a stem and an
// extension (the extension keeps its leading dot), and a rule works on the stem unless its scope
// says otherwise, so the extension a file had is the extension it keeps. A folder has no extension:
// its whole name is its stem, and a rule aimed at the extension leaves it alone. A rule that would
// leave an entry with no name at all is skipped for that entry, so no output is ever empty.
//
// A rule that cannot work (a pattern that does not compile, a date format with a token that is not
// known) is reported as a `RuleError` and skipped, so the others still preview.

use regex::{NoExpand, Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::CaseRule;

use crate::names::split_name;

/// The most a compiled pattern may grow to, so a hostile pattern costs a bounded amount.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

/// Which part of a name a rule works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum RenameScope {
    /// The whole name, extension included.
    Name,
    /// The name without its extension.
    Stem,
    /// The extension, without its dot. A folder, and a file with no extension, have none.
    Extension,
}

/// Where a counter or a date goes relative to the stem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum RulePosition {
    Prefix,
    Suffix,
    /// Instead of the stem.
    ReplaceStem,
}

/// How the letters of a name change case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum CaseMode {
    Upper,
    Lower,
    /// The first letter of each word upper, the rest lower.
    Title,
    /// The first letter upper, the rest lower.
    Sentence,
}

/// Which time a date rule writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DateSource {
    /// The entry's modification time.
    Modified,
    /// The entry's creation time; an entry whose provider has none uses its modification time.
    Created,
    /// The time the rename was planned, the same for every entry.
    Today,
}

/// Where `Insert` puts its text, in the stem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum InsertAt {
    Start,
    End,
    /// Before the character at this position (counted from 0 in characters); past the end is the
    /// end.
    Index {
        index: u32,
    },
}

/// One step of a batch rename.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum RenameRule {
    /// Replaces text. `find` is a regular expression when `regex` is set (`$1` and `${name}` in
    /// `replace` insert groups) and plain text otherwise. An empty `find` matches nothing. With
    /// `all` unset only the first match goes. A pattern is matched against the scope's text alone,
    /// so `^` and `$` are the ends of the stem for the stem scope.
    FindReplace {
        find: String,
        replace: String,
        regex: bool,
        case_sensitive: bool,
        scope: RenameScope,
        all: bool,
    },
    /// Numbers the entries in order: `start`, then `step` more for each one, padded with zeros to
    /// `width`. `separator` sits between the number and the stem (and is unused when the number
    /// replaces the stem).
    Counter {
        start: u32,
        step: u32,
        width: u8,
        position: RulePosition,
        separator: String,
    },
    Case {
        mode: CaseMode,
        scope: RenameScope,
    },
    /// Writes a time. `format` knows `%Y` (year), `%y` (two-digit year), `%m`, `%d`, `%H`, `%M`,
    /// `%S` and `%%`.
    DateToken {
        source: DateSource,
        format: String,
        position: RulePosition,
        separator: String,
    },
    Insert {
        text: String,
        at: InsertAt,
    },
    /// Removes the characters of the stem from `from` up to but not including `to` (counted from 0
    /// in characters).
    Remove {
        from: u32,
        to: u32,
    },
    /// Trims whitespace from both ends of the stem.
    TrimWhitespace,
    /// Gives files this extension (without its dot; empty removes it). Folders keep their names.
    ChangeExtension {
        to: String,
    },
}

/// What a batch rename job carries besides its sources: the rules, and the context that makes a
/// run reproducible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct RenameSpec {
    pub rules: Vec<RenameRule>,
    /// How far local time is ahead of UTC, in minutes, for the dates the rules write.
    #[serde(default)]
    pub utc_offset_minutes: i32,
    /// The time "today" means, in milliseconds since the Unix epoch. Absent, the planner reads
    /// the clock; a redo carries the time the first run used, so it writes the same names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number", optional)]
    pub now_ms: Option<i64>,
}

/// A rule that cannot work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct RuleError {
    /// The rule's position in the stack.
    pub rule: usize,
    pub reason: String,
}

/// An entry the rules are applied to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameInput {
    pub name: String,
    pub is_dir: bool,
    pub modified_ms: Option<i64>,
    pub created_ms: Option<i64>,
    /// The entry's place in the whole selection, which counters count and problems refer to.
    pub index: usize,
}

/// What the rules need to know besides the entries, for the folder they are in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameCtx {
    pub now_ms: i64,
    pub utc_offset_minutes: i32,
    /// How the folder's file system compares names.
    pub case_rule: CaseRule,
    /// Every name in the folder, which a result may not take.
    pub siblings: Vec<String>,
}

impl RenameCtx {
    pub fn new(now_ms: i64, case_rule: CaseRule) -> Self {
        Self {
            now_ms,
            utc_offset_minutes: 0,
            case_rule,
            siblings: Vec::new(),
        }
    }
}

/// A name in two parts. `ext` is empty or starts with a dot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parts {
    pub stem: String,
    pub ext: String,
}

impl Parts {
    pub(crate) fn of(name: &str, is_dir: bool) -> Self {
        if is_dir {
            return Self {
                stem: name.to_owned(),
                ext: String::new(),
            };
        }
        let (stem, ext) = split_name(name);
        Self {
            stem: stem.to_owned(),
            ext: ext.to_owned(),
        }
    }

    pub(crate) fn name(&self) -> String {
        format!("{}{}", self.stem, self.ext)
    }
}

/// A rule ready to run: its pattern compiled, or the reason it cannot be.
pub(crate) struct Compiled<'a> {
    rule: &'a RenameRule,
    regex: Option<Regex>,
    broken: bool,
}

/// Checks every rule and compiles what needs compiling.
pub(crate) fn compile(rules: &[RenameRule]) -> (Vec<Compiled<'_>>, Vec<RuleError>) {
    let mut errors = Vec::new();
    let mut out = Vec::with_capacity(rules.len());
    for (at, rule) in rules.iter().enumerate() {
        let mut regex = None;
        let mut problem: Option<String> = None;
        match rule {
            RenameRule::FindReplace {
                find,
                regex: is_regex,
                case_sensitive,
                ..
            } if !find.is_empty() => {
                let pattern = if *is_regex {
                    find.clone()
                } else {
                    regex::escape(find)
                };
                match RegexBuilder::new(&pattern)
                    .case_insensitive(!case_sensitive)
                    .size_limit(REGEX_SIZE_LIMIT)
                    .build()
                {
                    Ok(compiled) => regex = Some(compiled),
                    Err(error) => problem = Some(format!("the pattern is not valid: {error}")),
                }
            }
            RenameRule::DateToken { format, .. } => {
                problem = check_date_format(format).err();
            }
            _ => {}
        }
        if let Some(reason) = problem {
            errors.push(RuleError { rule: at, reason });
        }
        out.push(Compiled {
            rule,
            regex,
            broken: errors.last().is_some_and(|e| e.rule == at),
        });
    }
    (out, errors)
}

/// Checks the rules without applying them.
pub fn validate_rules(rules: &[RenameRule]) -> Vec<RuleError> {
    compile(rules).1
}

fn check_date_format(format: &str) -> Result<(), String> {
    if format.is_empty() {
        return Err("the date format is empty".to_owned());
    }
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('Y' | 'y' | 'm' | 'd' | 'H' | 'M' | 'S' | '%') => {}
                Some(other) => return Err(format!("the date format has no `%{other}`")),
                None => return Err("the date format ends in a lone `%`".to_owned()),
            }
        }
    }
    Ok(())
}

/// The civil date of a day count since 1970-01-01 (proleptic Gregorian).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (year + i64::from(month <= 2), month, day)
}

/// Writes a time with a format `check_date_format` has accepted, in the local time `offset_minutes`
/// ahead of UTC.
pub(crate) fn format_time(format: &str, ms: i64, offset_minutes: i32) -> String {
    let seconds = ms
        .saturating_add(i64::from(offset_minutes) * 60_000)
        .div_euclid(1000);
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let in_day = seconds.rem_euclid(86_400);
    let (hour, minute, second) = (in_day / 3600, in_day % 3600 / 60, in_day % 60);
    let mut out = String::new();
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&format!("{year:04}")),
            Some('y') => out.push_str(&format!("{:02}", year.rem_euclid(100))),
            Some('m') => out.push_str(&format!("{month:02}")),
            Some('d') => out.push_str(&format!("{day:02}")),
            Some('H') => out.push_str(&format!("{hour:02}")),
            Some('M') => out.push_str(&format!("{minute:02}")),
            Some('S') => out.push_str(&format!("{second:02}")),
            Some('%') => out.push('%'),
            _ => {}
        }
    }
    out
}

/// Changes the case of `text`. The first letter of a word takes its uppercase only when that is
/// one character, so applying `Title` or `Sentence` twice is the same as once.
pub(crate) fn change_case(text: &str, mode: CaseMode) -> String {
    fn upper_one(c: char) -> char {
        let mut up = c.to_uppercase();
        match (up.next(), up.next()) {
            (Some(only), None) => only,
            _ => c,
        }
    }
    match mode {
        CaseMode::Upper => text.to_uppercase(),
        CaseMode::Lower => text.to_lowercase(),
        CaseMode::Title => {
            let mut out = String::with_capacity(text.len());
            let mut in_word = false;
            for c in text.chars() {
                if c.is_alphanumeric() {
                    if in_word {
                        out.extend(c.to_lowercase());
                    } else {
                        out.push(upper_one(c));
                    }
                    in_word = true;
                } else {
                    // An apostrophe inside a word stays inside it ("don't").
                    in_word = in_word && c == '\'';
                    out.push(c);
                }
            }
            out
        }
        CaseMode::Sentence => {
            let mut out = String::with_capacity(text.len());
            let mut first = true;
            for c in text.chars() {
                if first && c.is_alphabetic() {
                    out.push(upper_one(c));
                    first = false;
                } else {
                    out.extend(c.to_lowercase());
                    first = first && !c.is_alphanumeric();
                }
            }
            out
        }
    }
}

fn counter_text(start: u32, step: u32, width: u8, index: usize) -> String {
    let n = u64::from(start) + u64::from(step) * index as u64;
    format!("{n:0>width$}", width = usize::from(width))
}

fn char_split(text: &str, at: u32) -> (&str, &str) {
    let byte = text
        .char_indices()
        .nth(at as usize)
        .map_or(text.len(), |(b, _)| b);
    text.split_at(byte)
}

fn place(position: RulePosition, stem: &str, text: &str, separator: &str) -> String {
    match position {
        RulePosition::Prefix => format!("{text}{separator}{stem}"),
        RulePosition::Suffix => format!("{stem}{separator}{text}"),
        RulePosition::ReplaceStem => text.to_owned(),
    }
}

fn replace_in(re: &Regex, text: &str, replacement: &str, is_regex: bool, all: bool) -> String {
    let limit = usize::from(!all);
    if is_regex {
        re.replacen(text, limit, replacement).into_owned()
    } else {
        re.replacen(text, limit, NoExpand(replacement)).into_owned()
    }
}

/// What one rule makes of a name's parts. `Err` is why the rule cannot apply to this entry.
fn apply_rule(
    compiled: &Compiled<'_>,
    parts: &Parts,
    input: &RenameInput,
    ctx: &RenameCtx,
) -> Result<Parts, String> {
    let is_dir = input.is_dir;
    let mut next = parts.clone();
    // Works on the extension body (without its dot) when the entry has one.
    let with_extension = |next: &mut Parts, f: &dyn Fn(&str) -> String| {
        if is_dir {
            return;
        }
        if let Some(body) = parts.ext.strip_prefix('.') {
            let changed = f(body);
            next.ext = if changed.is_empty() {
                String::new()
            } else {
                format!(".{changed}")
            };
        }
    };
    match compiled.rule {
        RenameRule::FindReplace {
            replace,
            regex,
            scope,
            all,
            ..
        } => {
            let Some(re) = &compiled.regex else {
                return Ok(next);
            };
            match scope {
                RenameScope::Stem => next.stem = replace_in(re, &parts.stem, replace, *regex, *all),
                RenameScope::Extension => {
                    with_extension(&mut next, &|b| replace_in(re, b, replace, *regex, *all));
                }
                RenameScope::Name => {
                    let whole = replace_in(re, &parts.name(), replace, *regex, *all);
                    next = Parts::of(&whole, is_dir);
                }
            }
        }
        RenameRule::Counter {
            start,
            step,
            width,
            position,
            separator,
        } => {
            let text = counter_text(*start, *step, *width, input.index);
            next.stem = place(*position, &parts.stem, &text, separator);
        }
        RenameRule::Case { mode, scope } => match scope {
            RenameScope::Stem => next.stem = change_case(&parts.stem, *mode),
            RenameScope::Extension => with_extension(&mut next, &|b| change_case(b, *mode)),
            RenameScope::Name => next = Parts::of(&change_case(&parts.name(), *mode), is_dir),
        },
        RenameRule::DateToken {
            source,
            format,
            position,
            separator,
        } => {
            let ms = match source {
                DateSource::Today => Some(ctx.now_ms),
                DateSource::Modified => input.modified_ms,
                DateSource::Created => input.created_ms.or(input.modified_ms),
            };
            let ms = ms.ok_or_else(|| "its time is not known".to_owned())?;
            let text = format_time(format, ms, ctx.utc_offset_minutes);
            next.stem = place(*position, &parts.stem, &text, separator);
        }
        RenameRule::Insert { text, at } => {
            let (head, tail) = match at {
                InsertAt::Start => ("", parts.stem.as_str()),
                InsertAt::End => (parts.stem.as_str(), ""),
                InsertAt::Index { index } => char_split(&parts.stem, *index),
            };
            next.stem = format!("{head}{text}{tail}");
        }
        RenameRule::Remove { from, to } => {
            if from < to {
                let (head, rest) = char_split(&parts.stem, *from);
                let (_, tail) = char_split(rest, to - from);
                next.stem = format!("{head}{tail}");
            }
        }
        RenameRule::TrimWhitespace => next.stem = parts.stem.trim().to_owned(),
        RenameRule::ChangeExtension { to } => {
            if !is_dir {
                let body = to.trim_start_matches('.');
                next.ext = if body.is_empty() {
                    String::new()
                } else {
                    format!(".{body}")
                };
            }
        }
    }
    Ok(next)
}

/// The result of the rules on one name.
pub(crate) struct Ruled {
    pub to: String,
    /// The extension changed, which the rules' scopes (and not only their stems) did.
    pub extension_changed: bool,
    /// Why a rule could not apply to this entry, which makes it a problem for the entry.
    pub failures: Vec<String>,
}

/// Runs the compiled rules over one entry.
pub(crate) fn run_rules(compiled: &[Compiled<'_>], input: &RenameInput, ctx: &RenameCtx) -> Ruled {
    let first = Parts::of(&input.name, input.is_dir);
    let mut parts = first.clone();
    let mut failures = Vec::new();
    for rule in compiled.iter().filter(|c| !c.broken) {
        match apply_rule(rule, &parts, input, ctx) {
            // A step that would leave no name at all is skipped.
            Ok(next) if next.name().is_empty() => {}
            Ok(next) => parts = next,
            Err(reason) => failures.push(reason),
        }
    }
    Ruled {
        extension_changed: !input.is_dir && parts.ext != first.ext,
        to: parts.name(),
        failures,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str, index: usize) -> RenameInput {
        RenameInput {
            name: name.to_owned(),
            is_dir: false,
            modified_ms: Some(1_700_000_000_000),
            created_ms: None,
            index,
        }
    }

    fn run(rules: &[RenameRule], name: &str, index: usize) -> String {
        let (compiled, errors) = compile(rules);
        assert!(errors.is_empty(), "{errors:?}");
        run_rules(
            &compiled,
            &input(name, index),
            &RenameCtx::new(1_700_000_000_000, CaseRule::Sensitive),
        )
        .to
    }

    fn find(find: &str, replace: &str) -> RenameRule {
        RenameRule::FindReplace {
            find: find.to_owned(),
            replace: replace.to_owned(),
            regex: false,
            case_sensitive: true,
            scope: RenameScope::Stem,
            all: true,
        }
    }

    #[test]
    fn dates_are_written_from_a_count_of_milliseconds() {
        // 2023-11-14 22:13:20 UTC.
        assert_eq!(
            format_time("%Y-%m-%d %H:%M:%S", 1_700_000_000_000, 0),
            "2023-11-14 22:13:20"
        );
        assert_eq!(
            format_time("%y/%%", 1_700_000_000_000, 0),
            "23/%",
            "two-digit years and a literal percent"
        );
        assert_eq!(format_time("%Y-%m-%d", 0, 0), "1970-01-01");
        assert_eq!(format_time("%Y-%m-%d", -1, 0), "1969-12-31");
        // Local time ahead of UTC by 2 h 30 m crosses midnight.
        assert_eq!(format_time("%d %H:%M", 1_700_000_000_000, 150), "15 00:43");
        assert_eq!(format_time("%Y-%m-%d", 951_782_400_000, 0), "2000-02-29");
        assert_eq!(format_time("%Y-%m-%d", 4_107_542_400_000, 0), "2100-03-01");
    }

    #[test]
    fn a_date_format_is_checked() {
        assert!(check_date_format("%Y%m%d").is_ok());
        assert!(check_date_format("").is_err());
        assert!(check_date_format("%Q").unwrap_err().contains("%Q"));
        assert!(check_date_format("a%").is_err());
    }

    #[test]
    fn find_and_replace_works_on_the_stem_by_default() {
        assert_eq!(run(&[find("a", "b")], "banana.txt", 0), "bbnbnb.txt");
        assert_eq!(
            run(&[find("txt", "md")], "txt.txt", 0),
            "md.txt",
            "the extension is not touched"
        );
        let first_only = RenameRule::FindReplace {
            find: "a".to_owned(),
            replace: "b".to_owned(),
            regex: false,
            case_sensitive: true,
            scope: RenameScope::Stem,
            all: false,
        };
        assert_eq!(run(&[first_only], "banana", 0), "bbnana");
        let insensitive = RenameRule::FindReplace {
            find: "A".to_owned(),
            replace: "-".to_owned(),
            regex: false,
            case_sensitive: false,
            scope: RenameScope::Stem,
            all: true,
        };
        assert_eq!(run(&[insensitive], "banana", 0), "b-n-n-");
        assert_eq!(
            run(&[find("", "x")], "abc", 0),
            "abc",
            "an empty find matches nothing"
        );
        assert_eq!(
            run(&[find("$", "x")], "a$b", 0),
            "axb",
            "plain text is not a pattern"
        );
    }

    #[test]
    fn counters_pad_and_position() {
        let counter = |position, separator: &str| RenameRule::Counter {
            start: 5,
            step: 10,
            width: 3,
            position,
            separator: separator.to_owned(),
        };
        assert_eq!(
            run(&[counter(RulePosition::Prefix, "_")], "a.txt", 0),
            "005_a.txt"
        );
        assert_eq!(
            run(&[counter(RulePosition::Suffix, "-")], "a.txt", 2),
            "a-025.txt"
        );
        assert_eq!(
            run(&[counter(RulePosition::ReplaceStem, "-")], "a.txt", 1),
            "015.txt"
        );
        let unpadded = RenameRule::Counter {
            start: 1,
            step: 1,
            width: 0,
            position: RulePosition::ReplaceStem,
            separator: String::new(),
        };
        assert_eq!(run(&[unpadded], "a", 99), "100");
    }

    #[test]
    fn case_modes_are_idempotent_for_ordinary_text() {
        for text in [
            "hello wORLD",
            "don't STOP-me now",
            "ÉCOLE élève",
            "x1y2 z",
            "",
        ] {
            for mode in [
                CaseMode::Upper,
                CaseMode::Lower,
                CaseMode::Title,
                CaseMode::Sentence,
            ] {
                let once = change_case(text, mode);
                assert_eq!(change_case(&once, mode), once, "{text:?} {mode:?}");
            }
        }
        assert_eq!(change_case("hello wORLD", CaseMode::Title), "Hello World");
        assert_eq!(change_case("don't STOP", CaseMode::Title), "Don't Stop");
        assert_eq!(
            change_case("  hello WORLD", CaseMode::Sentence),
            "  Hello world"
        );
        assert_eq!(change_case("1st PLACE", CaseMode::Sentence), "1st place");
    }

    #[test]
    fn insert_and_remove_count_characters() {
        let insert = |text: &str, at| RenameRule::Insert {
            text: text.to_owned(),
            at,
        };
        assert_eq!(run(&[insert("X", InsertAt::Start)], "ab.c", 0), "Xab.c");
        assert_eq!(run(&[insert("X", InsertAt::End)], "ab.c", 0), "abX.c");
        assert_eq!(
            run(&[insert("X", InsertAt::Index { index: 1 })], "ébc", 0),
            "éXbc"
        );
        assert_eq!(
            run(&[insert("X", InsertAt::Index { index: 99 })], "ab", 0),
            "abX"
        );
        let remove = |from, to| RenameRule::Remove { from, to };
        assert_eq!(run(&[remove(1, 3)], "abcde.txt", 0), "ade.txt");
        assert_eq!(run(&[remove(3, 99)], "abcde", 0), "abc");
        assert_eq!(run(&[remove(3, 1)], "abcde", 0), "abcde");
        assert_eq!(
            run(&[remove(0, 99)], "abc", 0),
            "abc",
            "never an empty name"
        );
    }

    #[test]
    fn the_extension_changes_only_when_a_rule_says_so() {
        let to = |to: &str| RenameRule::ChangeExtension { to: to.to_owned() };
        assert_eq!(run(&[to("md")], "a.txt", 0), "a.md");
        assert_eq!(run(&[to(".md")], "a.txt", 0), "a.md");
        assert_eq!(run(&[to("")], "a.txt", 0), "a");
        assert_eq!(run(&[to("gz")], "a", 0), "a.gz");
        assert_eq!(run(&[to("md")], "a.tar.gz", 0), "a.md");
        let rules = [to("md")];
        let (compiled, _) = compile(&rules);
        let dir = RenameInput {
            is_dir: true,
            ..input("a.b", 0)
        };
        let ctx = RenameCtx::new(0, CaseRule::Sensitive);
        let out = run_rules(&compiled, &dir, &ctx);
        assert_eq!(out.to, "a.b", "a folder has no extension");
        assert!(!out.extension_changed);
        let file = run_rules(&compiled, &input("a.txt", 0), &ctx);
        assert!(file.extension_changed);
        let upper_ext = RenameRule::Case {
            mode: CaseMode::Upper,
            scope: RenameScope::Extension,
        };
        assert_eq!(run(&[upper_ext], "a.tar.gz", 0), "a.TAR.GZ");
    }

    #[test]
    fn a_folder_name_is_all_stem() {
        let rules = [find("b", "X")];
        let (compiled, _) = compile(&rules);
        let dir = RenameInput {
            is_dir: true,
            ..input("a.b", 0)
        };
        let out = run_rules(&compiled, &dir, &RenameCtx::new(0, CaseRule::Sensitive));
        assert_eq!(out.to, "a.X");
    }

    #[test]
    fn a_broken_rule_is_reported_and_skipped() {
        let rules = [
            RenameRule::FindReplace {
                find: "(".to_owned(),
                replace: String::new(),
                regex: true,
                case_sensitive: true,
                scope: RenameScope::Stem,
                all: true,
            },
            RenameRule::DateToken {
                source: DateSource::Today,
                format: "%Q".to_owned(),
                position: RulePosition::Prefix,
                separator: String::new(),
            },
            find("a", "b"),
        ];
        let errors = validate_rules(&rules);
        assert_eq!(errors.iter().map(|e| e.rule).collect::<Vec<_>>(), [0, 1]);
        let (compiled, _) = compile(&rules);
        let out = run_rules(
            &compiled,
            &input("a.txt", 0),
            &RenameCtx::new(0, CaseRule::Sensitive),
        );
        assert_eq!(out.to, "b.txt", "the good rule still runs");
    }

    #[test]
    fn a_missing_time_is_a_failure_for_that_entry() {
        let rule = RenameRule::DateToken {
            source: DateSource::Modified,
            format: "%Y".to_owned(),
            position: RulePosition::Prefix,
            separator: "_".to_owned(),
        };
        let rules = [rule];
        let (compiled, _) = compile(&rules);
        let mut entry = input("a.txt", 0);
        entry.modified_ms = None;
        let out = run_rules(&compiled, &entry, &RenameCtx::new(0, CaseRule::Sensitive));
        assert_eq!(out.to, "a.txt");
        assert_eq!(out.failures.len(), 1);
    }
}

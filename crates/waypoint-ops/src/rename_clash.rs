// What can go wrong between a batch rename's results and the folder they land in, and the order of
// renames that gets every entry to its result without ever overwriting one. It is pure (A47): names
// go in, a verdict and a sequence of steps come out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Two results may not be the same name under the folder's case rule, and a result may not be the
// name of an entry that is staying. A result that is the name of an entry that is itself being
// renamed is fine, because that entry will have moved by then: `a`→`b` with `b`→`c` is a chain, and
// `a`→`b` with `b`→`a` is a swap. Because results are distinct and so are the names they replace,
// every entry waits for at most one other and at most one waits for it, so the dependencies form
// disjoint chains and cycles. A chain goes from its far end back. A cycle (a swap, a rotation, or a
// rename that changes only the case on a provider that treats the two spellings as one name) sends
// one entry to a temporary name first and finishes with it. No step ever has a taken name as its
// target.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::{windows, CaseRule};
use waypoint_protocol::VfsError;
use waypoint_vfs::validate_name;

use crate::rename_rules::{
    compile, run_rules, Compiled, RenameCtx, RenameInput, RenameRule, RuleError,
};

/// Why a result cannot be used, or that there is nothing to do for the entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum Problem {
    /// The folder's file system would not accept the name, or a rule could not work on the entry.
    Invalid { reason: String },
    /// Another result is the same name; `with` is that entry's index.
    DuplicateTarget { with: usize },
    /// An entry that is not being renamed already has the name.
    ExistsInFolder,
    /// The rules leave the name as it is. Not a reason to refuse the rename.
    UnchangedSkip,
}

impl Problem {
    /// Whether the rename cannot go ahead while this stands.
    pub fn blocks(&self) -> bool {
        !matches!(self, Problem::UnchangedSkip)
    }
}

/// One entry's row of the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct PreviewRow {
    /// The entry's place in the selection.
    pub index: usize,
    pub from: String,
    pub to: String,
    /// The name differs from the old one.
    pub changed: bool,
    /// The entry's extension is not the one it had, which only a rule aimed at it does.
    pub extension_changed: bool,
    pub problems: Vec<Problem>,
}

/// What the rules make of each entry, and the rules that could not work.
pub type RenameOutput = PreviewRow;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleOutputs {
    pub rows: Vec<RenameOutput>,
    pub rule_errors: Vec<RuleError>,
}

/// A name on its way to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NamePair<'a> {
    index: usize,
    from: &'a str,
    to: &'a str,
}

/// The key two names share exactly when they are the same name under `rule`.
fn key(name: &str, rule: CaseRule) -> String {
    match rule {
        CaseRule::Sensitive => name.to_owned(),
        CaseRule::Insensitive => windows::fold(name),
    }
}

fn invalid_reason(error: VfsError) -> String {
    match error {
        VfsError::InvalidName { reason, .. } => reason,
        other => format!("{other:?}"),
    }
}

/// The problems of each pair, in the order given. `siblings` are every name in the folder (the
/// pairs' own `from`s are counted too, so a caller that leaves out an entry that is staying does
/// not lose its name).
fn find_problems(pairs: &[NamePair<'_>], siblings: &[String], rule: CaseRule) -> Vec<Vec<Problem>> {
    let changing: Vec<bool> = pairs.iter().map(|p| p.from != p.to).collect();
    let vacated: HashSet<String> = pairs
        .iter()
        .zip(&changing)
        .filter(|(_, c)| **c)
        .map(|(p, _)| key(p.from, rule))
        .collect();
    let mut taken: HashSet<String> = siblings.iter().map(|n| key(n, rule)).collect();
    taken.extend(pairs.iter().map(|p| key(p.from, rule)));
    taken.retain(|name| !vacated.contains(name));

    let mut wanting: HashMap<String, Vec<usize>> = HashMap::new();
    for (at, (pair, changing)) in pairs.iter().zip(&changing).enumerate() {
        if *changing {
            wanting.entry(key(pair.to, rule)).or_default().push(at);
        }
    }

    pairs
        .iter()
        .enumerate()
        .map(|(at, pair)| {
            if !changing[at] {
                return vec![Problem::UnchangedSkip];
            }
            let mut problems = Vec::new();
            if let Err(error) = validate_name(pair.to.as_ref(), rule) {
                problems.push(Problem::Invalid {
                    reason: invalid_reason(error),
                });
            }
            let same = key(pair.to, rule);
            if let Some(other) = wanting[&same].iter().find(|other| **other != at) {
                problems.push(Problem::DuplicateTarget {
                    with: pairs[*other].index,
                });
            }
            if taken.contains(&same) {
                problems.push(Problem::ExistsInFolder);
            }
            problems
        })
        .collect()
}

/// Applies the rules to the entries of one folder and finds what is wrong with the results.
pub fn apply_rules(inputs: &[RenameInput], rules: &[RenameRule], ctx: &RenameCtx) -> RuleOutputs {
    let (compiled, rule_errors) = compile(rules);
    RuleOutputs {
        rows: rows_for(inputs, &compiled, ctx),
        rule_errors,
    }
}

fn rows_for(inputs: &[RenameInput], compiled: &[Compiled<'_>], ctx: &RenameCtx) -> Vec<PreviewRow> {
    let ruled: Vec<_> = inputs
        .iter()
        .map(|input| run_rules(compiled, input, ctx))
        .collect();
    let pairs: Vec<NamePair<'_>> = inputs
        .iter()
        .zip(&ruled)
        .map(|(input, ruled)| NamePair {
            index: input.index,
            from: &input.name,
            to: &ruled.to,
        })
        .collect();
    let found = find_problems(&pairs, &ctx.siblings, ctx.case_rule);
    inputs
        .iter()
        .zip(ruled)
        .zip(found)
        .map(|((input, ruled), found)| {
            let mut problems: Vec<Problem> = ruled
                .failures
                .into_iter()
                .map(|reason| Problem::Invalid { reason })
                .collect();
            // A rule that could not work is the entry's problem, and "nothing to do" is then not
            // the whole story.
            let failed = !problems.is_empty();
            problems.extend(found.into_iter().filter(|p| !failed || p.blocks()));
            PreviewRow {
                index: input.index,
                changed: ruled.to != input.name,
                extension_changed: ruled.extension_changed,
                from: input.name.clone(),
                to: ruled.to,
                problems,
            }
        })
        .collect()
}

fn problems_block(problems: &[Problem]) -> bool {
    problems.iter().any(Problem::blocks)
}

/// Where an entry is, from the point of view of one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    /// Under the name it has now.
    Source,
    /// Under a temporary name of the executor's choosing, one per entry.
    Temp,
    /// Under the name the rules gave it.
    Target,
}

/// One rename of one entry, which is always to a free name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenameStep {
    /// The entry, by its position in the list given to `plan_batch`.
    pub entry: usize,
    pub from: Place,
    pub to: Place,
}

/// An entry and the name the rules gave it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchEntry {
    pub from: String,
    pub to: String,
}

/// Why a batch cannot be planned: each entry that has a blocking problem, with the problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    pub problems: Vec<(usize, Problem)>,
}

impl std::fmt::Display for Clash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} names cannot be used", self.problems.len())
    }
}

impl std::error::Error for Clash {}

/// The sequence of renames that takes every entry to its name, with `siblings` the folder's other
/// names. Entries whose name does not change have no step. It refuses with the problems when any
/// result is invalid, repeated or taken by an entry that stays, so nothing is half done; otherwise
/// every step's target is free when the step runs, and the sequence leaves exactly the results.
pub fn plan_batch(
    entries: &[BatchEntry],
    siblings: &[String],
    rule: CaseRule,
) -> Result<Vec<RenameStep>, Clash> {
    let pairs: Vec<NamePair<'_>> = entries
        .iter()
        .enumerate()
        .map(|(index, e)| NamePair {
            index,
            from: &e.from,
            to: &e.to,
        })
        .collect();
    let found = find_problems(&pairs, siblings, rule);
    let problems: Vec<(usize, Problem)> = found
        .into_iter()
        .enumerate()
        .flat_map(|(at, list)| list.into_iter().map(move |p| (at, p)))
        .filter(|(_, p)| p.blocks())
        .collect();
    if !problems.is_empty() {
        return Err(Clash { problems });
    }

    let moving: Vec<usize> = (0..entries.len())
        .filter(|at| entries[*at].from != entries[*at].to)
        .collect();
    let by_source: HashMap<String, usize> = moving
        .iter()
        .map(|at| (key(&entries[*at].from, rule), *at))
        .collect();
    // The entry that has to move out of the way of `at`'s result, if there is one.
    let blocker = |at: usize| by_source.get(&key(&entries[at].to, rule)).copied();

    let mut steps = Vec::new();
    let mut seen = vec![false; entries.len()];
    for start in moving {
        if seen[start] {
            continue;
        }
        let mut chain = Vec::new();
        let mut here = start;
        loop {
            if seen[here] {
                break;
            }
            seen[here] = true;
            chain.push(here);
            match blocker(here) {
                Some(next) => here = next,
                None => break,
            }
        }
        let last = *chain.last().expect("a walk takes at least its first entry");
        let cycle = blocker(last) == Some(chain[0]);
        let step = |entry, from, to| RenameStep { entry, from, to };
        if cycle {
            // The first entry steps aside so the last can take its place, and so on back round.
            steps.push(step(chain[0], Place::Source, Place::Temp));
            for entry in chain[1..].iter().rev() {
                steps.push(step(*entry, Place::Source, Place::Target));
            }
            steps.push(step(chain[0], Place::Temp, Place::Target));
        } else {
            for entry in chain.iter().rev() {
                steps.push(step(*entry, Place::Source, Place::Target));
            }
        }
    }
    Ok(steps)
}

/// The entries of one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderInputs {
    pub case_rule: CaseRule,
    /// Every name in the folder.
    pub siblings: Vec<String>,
    pub inputs: Vec<RenameInput>,
}

/// What the preview of a batch rename is asked over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewRequest {
    pub folders: Vec<FolderInputs>,
    pub rules: Vec<RenameRule>,
    pub now_ms: i64,
    pub utc_offset_minutes: i32,
}

/// The preview of a batch rename: a row for each entry, in selection order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct BatchPreview {
    pub rows: Vec<PreviewRow>,
    /// Rules that cannot work; they are left out of the results.
    pub rule_errors: Vec<RuleError>,
    /// The time "today" meant, which a job submitted from this preview carries so it writes the
    /// same names.
    #[ts(type = "number")]
    pub now_ms: i64,
    /// Rows with a problem that stops the rename, plus rules that cannot work.
    pub problems: usize,
    /// Rows whose name changes.
    pub changes: usize,
}

impl BatchPreview {
    /// Whether the rename can go ahead as previewed.
    pub fn ready(&self) -> bool {
        self.problems == 0 && self.changes > 0
    }
}

/// The pure core of the preview command: applies the rules to the entries of each folder and finds
/// the clashes within it.
pub fn preview_batch_rename(request: &PreviewRequest) -> BatchPreview {
    let (compiled, rule_errors) = compile(&request.rules);
    let mut rows = Vec::new();
    for folder in &request.folders {
        let ctx = RenameCtx {
            now_ms: request.now_ms,
            utc_offset_minutes: request.utc_offset_minutes,
            case_rule: folder.case_rule,
            siblings: folder.siblings.clone(),
        };
        rows.extend(rows_for(&folder.inputs, &compiled, &ctx));
    }
    rows.sort_by_key(|row| row.index);
    let problems = rule_errors.len()
        + rows
            .iter()
            .filter(|row| problems_block(&row.problems))
            .count();
    let changes = rows.iter().filter(|row| row.changed).count();
    BatchPreview {
        rows,
        rule_errors,
        now_ms: request.now_ms,
        problems,
        changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rename_rules::{RenameScope, RulePosition};

    fn entries(pairs: &[(&str, &str)]) -> Vec<BatchEntry> {
        pairs
            .iter()
            .map(|(from, to)| BatchEntry {
                from: (*from).to_owned(),
                to: (*to).to_owned(),
            })
            .collect()
    }

    /// The folder as a set of names under `rule` after `steps`, panicking on an overwrite.
    fn replay(
        list: &[BatchEntry],
        siblings: &[&str],
        steps: &[RenameStep],
        rule: CaseRule,
    ) -> Vec<String> {
        let mut names: HashMap<String, String> = siblings
            .iter()
            .map(|n| (key(n, rule), (*n).to_owned()))
            .collect();
        for step in steps {
            let entry = &list[step.entry];
            let name = |place: Place| match place {
                Place::Source => entry.from.clone(),
                Place::Target => entry.to.clone(),
                Place::Temp => format!("\u{1}temp{}", step.entry),
            };
            let (from, to) = (name(step.from), name(step.to));
            assert!(names.remove(&key(&from, rule)).is_some(), "{from} is there");
            assert!(
                names.insert(key(&to, rule), to.clone()).is_none(),
                "{to} was free"
            );
        }
        let mut out: Vec<String> = names.into_values().collect();
        out.sort();
        out
    }

    #[test]
    fn a_swap_goes_through_a_temporary_name() {
        let list = entries(&[("a", "b"), ("b", "a")]);
        let steps = plan_batch(&list, &["a".into(), "b".into()], CaseRule::Sensitive).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].to, Place::Temp);
        assert_eq!(steps[2].from, Place::Temp);
        assert_eq!(
            replay(&list, &["a", "b"], &steps, CaseRule::Sensitive),
            ["a", "b"]
        );
    }

    #[test]
    fn a_chain_needs_no_temporary_name() {
        let list = entries(&[("a", "b"), ("b", "c"), ("c", "d")]);
        let siblings = ["a", "b", "c", "keep"];
        let siblings_owned: Vec<String> = siblings.iter().map(|s| (*s).to_owned()).collect();
        let steps = plan_batch(&list, &siblings_owned, CaseRule::Sensitive).unwrap();
        assert_eq!(steps.len(), 3);
        assert!(steps
            .iter()
            .all(|s| s.from == Place::Source && s.to == Place::Target));
        assert_eq!(
            replay(&list, &siblings, &steps, CaseRule::Sensitive),
            ["b", "c", "d", "keep"]
        );
    }

    #[test]
    fn a_case_only_change_is_direct_where_cases_differ_and_aside_where_they_do_not() {
        let list = entries(&[("readme", "README")]);
        let sensitive = plan_batch(&list, &["readme".into()], CaseRule::Sensitive).unwrap();
        assert_eq!(sensitive.len(), 1);
        let insensitive = plan_batch(&list, &["readme".into()], CaseRule::Insensitive).unwrap();
        assert_eq!(insensitive.len(), 2, "through a temporary name");
        assert_eq!(
            replay(&list, &["readme"], &insensitive, CaseRule::Insensitive),
            ["README"]
        );
    }

    #[test]
    fn clashes_refuse_the_whole_batch() {
        let list = entries(&[("a", "x"), ("b", "X"), ("c", "keep"), ("d", "")]);
        let siblings: Vec<String> = ["a", "b", "c", "d", "keep"].map(String::from).into();
        let clash = plan_batch(&list, &siblings, CaseRule::Insensitive).unwrap_err();
        assert!(clash
            .problems
            .contains(&(0, Problem::DuplicateTarget { with: 1 })));
        assert!(clash
            .problems
            .contains(&(1, Problem::DuplicateTarget { with: 0 })));
        assert!(clash.problems.contains(&(2, Problem::ExistsInFolder)));
        assert!(clash
            .problems
            .iter()
            .any(|(at, p)| *at == 3 && matches!(p, Problem::Invalid { .. })));
        // Under a case-sensitive rule `x` and `X` are different names.
        let clash = plan_batch(&list, &siblings, CaseRule::Sensitive).unwrap_err();
        assert!(!clash
            .problems
            .iter()
            .any(|(_, p)| matches!(p, Problem::DuplicateTarget { .. })));
    }

    #[test]
    fn windows_names_are_checked_by_the_insensitive_rule_only() {
        let list = entries(&[("a", "CON"), ("b", "dots."), ("c", "ok")]);
        let siblings: Vec<String> = ["a", "b", "c"].map(String::from).into();
        assert!(plan_batch(&list, &siblings, CaseRule::Sensitive).is_ok());
        let clash = plan_batch(&list, &siblings, CaseRule::Insensitive).unwrap_err();
        let bad: Vec<usize> = clash.problems.iter().map(|(at, _)| *at).collect();
        assert_eq!(bad, [0, 1]);
    }

    #[test]
    fn an_unchanged_entry_is_skipped_and_keeps_its_name() {
        let list = entries(&[("a", "a"), ("b", "a2")]);
        let siblings: Vec<String> = ["a", "b"].map(String::from).into();
        let steps = plan_batch(&list, &siblings, CaseRule::Sensitive).unwrap();
        assert_eq!(steps.len(), 1);
        // Taking the name of an entry that is staying is a clash.
        let list = entries(&[("a", "a"), ("b", "a")]);
        assert!(plan_batch(&list, &siblings, CaseRule::Sensitive).is_err());
    }

    fn input(name: &str, index: usize) -> RenameInput {
        RenameInput {
            name: name.to_owned(),
            is_dir: false,
            modified_ms: None,
            created_ms: None,
            index,
        }
    }

    fn replace(find: &str, replace: &str) -> RenameRule {
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
    fn the_preview_marks_each_row() {
        let mut ctx = RenameCtx::new(0, CaseRule::Sensitive);
        ctx.siblings = ["a1.txt", "b.txt", "z.txt", "z2.txt"]
            .map(String::from)
            .into();
        let inputs = [input("a1.txt", 0), input("b.txt", 1), input("z.txt", 2)];
        let out = apply_rules(&inputs, &[replace("a1", "z2")], &ctx);
        assert!(out.rule_errors.is_empty());
        assert_eq!(out.rows[0].to, "z2.txt");
        assert_eq!(out.rows[0].problems, [Problem::ExistsInFolder]);
        assert_eq!(out.rows[1].problems, [Problem::UnchangedSkip]);
        assert!(!out.rows[1].changed);
    }

    #[test]
    fn the_preview_counts_what_stops_it() {
        let mut folder = FolderInputs {
            case_rule: CaseRule::Sensitive,
            siblings: ["a.txt", "b.txt"].map(String::from).into(),
            inputs: vec![input("a.txt", 0), input("b.txt", 1)],
        };
        let rules = |find: &str, to: &str| vec![replace(find, to)];
        let request = |folder: &FolderInputs, rules| PreviewRequest {
            folders: vec![folder.clone()],
            rules,
            now_ms: 7,
            utc_offset_minutes: 0,
        };
        let ok = preview_batch_rename(&request(&folder, rules("a", "c")));
        assert!(ok.ready());
        assert_eq!((ok.problems, ok.changes, ok.now_ms), (0, 1, 7));
        // Both entries become `x.txt`.
        let clash = preview_batch_rename(&request(
            &folder,
            vec![RenameRule::Counter {
                start: 1,
                step: 0,
                width: 1,
                position: RulePosition::ReplaceStem,
                separator: String::new(),
            }],
        ));
        assert_eq!(clash.problems, 2);
        assert!(!clash.ready());
        // A rule that cannot work counts as a problem and leaves the names alone.
        let bad = preview_batch_rename(&request(
            &folder,
            vec![RenameRule::FindReplace {
                find: "[".to_owned(),
                replace: String::new(),
                regex: true,
                case_sensitive: true,
                scope: RenameScope::Stem,
                all: true,
            }],
        ));
        assert_eq!((bad.problems, bad.changes), (1, 0));
        // Nothing to do is not a problem, and not ready either.
        folder.inputs.truncate(1);
        let none = preview_batch_rename(&request(&folder, rules("zzz", "y")));
        assert!(!none.ready());
        assert_eq!(none.problems, 0);
    }

    #[test]
    fn an_entry_the_rules_leave_alone_is_only_skipped() {
        let mut ctx = RenameCtx::new(0, CaseRule::Insensitive);
        ctx.siblings = vec!["a".to_owned()];
        let out = apply_rules(&[input("a", 0)], &[], &ctx);
        assert_eq!(out.rows[0].problems, [Problem::UnchangedSkip]);
    }
}

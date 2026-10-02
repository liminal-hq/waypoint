// Seeded random checks of batch rename: the rules keep their invariants over random names and rule
// stacks, `plan_batch` never overwrites and always reaches the target set (checked against a model
// on both case rules, and by running its steps on the in-memory and local providers), and whole
// jobs agree with the pure preview and undo to the exact original names. A failing seed reproduces.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod journal_support;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;

use common::*;
use journal_support::*;
use waypoint_path::windows;
use waypoint_vfs::{child_path, validate_name};

const NOW: i64 = 1_700_000_000_000;

const NAMES: [&str; 14] = [
    "report.txt",
    "Résumé final.PDF",
    " spaced name .md",
    "a.tar.gz",
    ".hidden",
    "noext",
    "UPPER.TXT",
    "x.y.z",
    "日本語.txt",
    "multi  space.txt",
    "e.TAR.xz",
    "a",
    ".tar.gz",
    "don't stop.mp3",
];

fn pick<'a>(rng: &mut Rng, items: &[&'a str]) -> &'a str {
    items[rng.below(items.len())]
}

fn scope(rng: &mut Rng, stem_only: bool) -> RenameScope {
    if stem_only {
        return RenameScope::Stem;
    }
    *rng.pick(&[RenameScope::Name, RenameScope::Stem, RenameScope::Extension])
}

fn random_rule(rng: &mut Rng, stem_only: bool) -> RenameRule {
    let position = *rng.pick(&[
        RulePosition::Prefix,
        RulePosition::Suffix,
        RulePosition::ReplaceStem,
    ]);
    match rng.below(9) {
        0 => RenameRule::FindReplace {
            find: pick(rng, &["a", "e", "x", ".", " ", "1", "REP", ""]).to_owned(),
            replace: pick(rng, &["", "z", "Q", " ", "-", "$1", "é"]).to_owned(),
            regex: false,
            case_sensitive: rng.chance(2),
            scope: scope(rng, stem_only),
            all: rng.chance(2),
        },
        1 => RenameRule::FindReplace {
            find: pick(
                rng,
                &[
                    "^", "$", "a*", "(.)(.)", "[0-9]+", "\\s+", "^.", "x?", "(?i)e", "\\w+",
                ],
            )
            .to_owned(),
            replace: pick(rng, &["$1", "${1}-", "", "_", "$2$1", "[$0]"]).to_owned(),
            regex: true,
            case_sensitive: rng.chance(2),
            scope: scope(rng, stem_only),
            all: rng.chance(2),
        },
        2 => RenameRule::Counter {
            start: rng.below(50) as u32,
            step: rng.below(4) as u32,
            width: rng.below(5) as u8,
            position,
            separator: pick(rng, &["", "_", " - "]).to_owned(),
        },
        3 => RenameRule::Case {
            mode: *rng.pick(&[
                CaseMode::Upper,
                CaseMode::Lower,
                CaseMode::Title,
                CaseMode::Sentence,
            ]),
            scope: scope(rng, stem_only),
        },
        4 => RenameRule::DateToken {
            source: *rng.pick(&[DateSource::Modified, DateSource::Created, DateSource::Today]),
            format: pick(rng, &["%Y-%m-%d", "%y%m%d_%H%M%S", "%%", "%Y"]).to_owned(),
            position,
            separator: pick(rng, &["", "_"]).to_owned(),
        },
        5 => RenameRule::Insert {
            text: pick(rng, &["X", "", " new ", "é"]).to_owned(),
            at: match rng.below(3) {
                0 => InsertAt::Start,
                1 => InsertAt::End,
                _ => InsertAt::Index {
                    index: rng.below(12) as u32,
                },
            },
        },
        6 => RenameRule::Remove {
            from: rng.below(8) as u32,
            to: rng.below(20) as u32,
        },
        7 => RenameRule::TrimWhitespace,
        _ if stem_only => RenameRule::TrimWhitespace,
        _ => RenameRule::ChangeExtension {
            to: pick(rng, &["md", ".txt", "", "tar.gz"]).to_owned(),
        },
    }
}

fn random_inputs(rng: &mut Rng) -> Vec<RenameInput> {
    let count = 1 + rng.below(8);
    (0..count)
        .map(|index| RenameInput {
            name: pick(rng, &NAMES).to_owned(),
            is_dir: rng.chance(4),
            modified_ms: (!rng.chance(8)).then_some(NOW - 86_400_000 * rng.below(900) as i64),
            created_ms: rng
                .chance(3)
                .then_some(NOW - 86_400_000 * rng.below(900) as i64),
            index,
        })
        .collect()
}

fn ctx(rule: CaseRule) -> RenameCtx {
    RenameCtx {
        now_ms: NOW,
        utc_offset_minutes: -300,
        case_rule: rule,
        siblings: Vec::new(),
    }
}

fn ext_of(name: &str) -> &str {
    split_name(name).1
}

#[test]
fn rules_keep_their_invariants_over_random_names_and_stacks() {
    for seed in 0..400u64 {
        let mut rng = Rng::seeded(seed);
        let stem_only = rng.chance(2);
        let rules: Vec<RenameRule> = (0..1 + rng.below(4))
            .map(|_| random_rule(&mut rng, stem_only))
            .collect();
        let mut inputs = random_inputs(&mut rng);
        // An entry with no time fails a date rule, which is not a clash `plan_batch` sees.
        let timed = rng.chance(2);
        if timed {
            inputs.iter_mut().for_each(|i| i.modified_ms = Some(NOW));
        }
        let rule = if rng.chance(2) {
            CaseRule::Sensitive
        } else {
            CaseRule::Insensitive
        };
        let first = apply_rules(&inputs, &rules, &ctx(rule));
        let second = apply_rules(&inputs, &rules, &ctx(rule));
        assert_eq!(first.rows, second.rows, "seed {seed}: deterministic");
        assert_eq!(first.rule_errors, second.rule_errors);
        for (input, row) in inputs.iter().zip(&first.rows) {
            let what = format!("seed {seed} {:?} -> {:?} by {rules:?}", input.name, row.to);
            assert!(!row.to.is_empty(), "{what}: an empty name");
            assert_eq!(row.from, input.name);
            assert_eq!(row.changed, row.to != input.name, "{what}");
            if stem_only && !input.is_dir {
                assert!(
                    row.to.ends_with(ext_of(&input.name)),
                    "{what}: the extension is untouched"
                );
                assert!(!row.extension_changed, "{what}");
            }
            if input.is_dir {
                assert!(!row.extension_changed, "{what}: a folder has no extension");
            }
        }
        // With nothing in the way there is a plan exactly when no row has a problem.
        let blocked = first
            .rows
            .iter()
            .any(|r| r.problems.iter().any(Problem::blocks));
        let pairs: Vec<BatchEntry> = first
            .rows
            .iter()
            .map(|r| BatchEntry {
                from: r.from.clone(),
                to: r.to.clone(),
            })
            .collect();
        // The inputs may repeat a name (the pool is small); give each entry its own folder name.
        let unique: HashSet<String> = inputs.iter().map(|i| fold(rule, &i.name)).collect();
        if timed && unique.len() == inputs.len() {
            let siblings: Vec<String> = inputs.iter().map(|i| i.name.clone()).collect();
            let planned = plan_batch(&pairs, &siblings, rule);
            assert_eq!(planned.is_err(), blocked, "seed {seed}: {rules:?}");
        }
    }
}

fn fold(rule: CaseRule, name: &str) -> String {
    match rule {
        CaseRule::Sensitive => name.to_owned(),
        CaseRule::Insensitive => windows::fold(name),
    }
}

#[test]
fn case_rules_are_idempotent() {
    for seed in 0..300u64 {
        let mut rng = Rng::seeded(seed);
        let rules: Vec<RenameRule> = (0..1 + rng.below(3))
            .map(|_| RenameRule::Case {
                mode: *rng.pick(&[
                    CaseMode::Upper,
                    CaseMode::Lower,
                    CaseMode::Title,
                    CaseMode::Sentence,
                ]),
                scope: {
                    let stem = rng.chance(2);
                    scope(&mut rng, stem)
                },
            })
            .collect();
        // A single mode applied twice is the same as once; a stack settles on its last mode.
        let last = rules.last().cloned().into_iter().collect::<Vec<_>>();
        let inputs = random_inputs(&mut rng);
        let once = apply_rules(&inputs, &last, &ctx(CaseRule::Sensitive));
        let again_inputs: Vec<RenameInput> = inputs
            .iter()
            .zip(&once.rows)
            .map(|(i, r)| RenameInput {
                name: r.to.clone(),
                ..i.clone()
            })
            .collect();
        let twice = apply_rules(&again_inputs, &last, &ctx(CaseRule::Sensitive));
        for (a, b) in once.rows.iter().zip(&twice.rows) {
            assert_eq!(a.to, b.to, "seed {seed}: {:?} by {last:?}", a.from);
        }
    }
}

// ---- regex edge cases ----

fn regex_rule(find: &str, replace: &str, all: bool) -> RenameRule {
    RenameRule::FindReplace {
        find: find.to_owned(),
        replace: replace.to_owned(),
        regex: true,
        case_sensitive: true,
        scope: RenameScope::Stem,
        all,
    }
}

fn one(name: &str, rules: &[RenameRule]) -> String {
    let input = RenameInput {
        name: name.to_owned(),
        is_dir: false,
        modified_ms: None,
        created_ms: None,
        index: 0,
    };
    let out = apply_rules(&[input], rules, &ctx(CaseRule::Sensitive));
    assert!(out.rule_errors.is_empty(), "{:?}", out.rule_errors);
    out.rows[0].to.clone()
}

#[test]
fn regex_edge_cases() {
    // Anchors are the ends of the stem.
    assert_eq!(one("abc.txt", &[regex_rule("^", "x_", true)]), "x_abc.txt");
    assert_eq!(one("abc.txt", &[regex_rule("$", "_x", true)]), "abc_x.txt");
    // An empty match at every position, replaced once or everywhere.
    assert_eq!(one("ab.txt", &[regex_rule("x*", "-", true)]), "-a-b-.txt");
    assert_eq!(one("ab.txt", &[regex_rule("x*", "-", false)]), "-ab.txt");
    // Groups, numbered and named.
    assert_eq!(
        one(
            "2024-05 notes.md",
            &[regex_rule(r"^(\d+)-(\d+)", "$2.$1", true)]
        ),
        "05.2024 notes.md"
    );
    assert_eq!(
        one(
            "john smith.txt",
            &[regex_rule(r"(?P<f>\w+) (?P<l>\w+)", "${l}, ${f}", true)]
        ),
        "smith, john.txt"
    );
    // A group that did not take part is empty, and `$` can be escaped.
    assert_eq!(
        one("ab.txt", &[regex_rule("(x)?a", "[$1]", true)]),
        "[]b.txt"
    );
    assert_eq!(one("ab.txt", &[regex_rule("a", "$$", true)]), "$b.txt");
    // Case-insensitive plain text, and the extension left alone.
    let loud = RenameRule::FindReplace {
        find: "TXT".to_owned(),
        replace: "x".to_owned(),
        regex: false,
        case_sensitive: false,
        scope: RenameScope::Stem,
        all: true,
    };
    assert_eq!(one("txt.txt", &[loud]), "x.txt");
    // Unicode.
    assert_eq!(
        one("日本語.txt", &[regex_rule("本", "X", true)]),
        "日X語.txt"
    );
    // An invalid pattern is a problem, not a panic, and the other rules still run.
    let input = RenameInput {
        name: "ab.txt".to_owned(),
        is_dir: false,
        modified_ms: None,
        created_ms: None,
        index: 0,
    };
    let out = apply_rules(
        &[input],
        &[regex_rule("(", "", true), regex_rule("a", "z", true)],
        &ctx(CaseRule::Sensitive),
    );
    assert_eq!(out.rule_errors.len(), 1);
    assert_eq!(out.rule_errors[0].rule, 0);
    assert_eq!(out.rows[0].to, "zb.txt");
    // A pattern too big to compile is refused the same way.
    let huge = regex_rule("(((a{100}){100}){100}){100}", "", true);
    assert_eq!(validate_rules(&[huge]).len(), 1);
}

#[test]
fn counter_widths_and_steps() {
    let counter = |start, step, width, position| RenameRule::Counter {
        start,
        step,
        width,
        position,
        separator: "-".to_owned(),
    };
    let inputs: Vec<RenameInput> = (0..3)
        .map(|index| RenameInput {
            name: format!("f{index}.txt"),
            is_dir: false,
            modified_ms: None,
            created_ms: None,
            index,
        })
        .collect();
    let names = |rule: RenameRule| -> Vec<String> {
        apply_rules(&inputs, &[rule], &ctx(CaseRule::Sensitive))
            .rows
            .into_iter()
            .map(|r| r.to)
            .collect()
    };
    assert_eq!(
        names(counter(1, 1, 3, RulePosition::Prefix)),
        ["001-f0.txt", "002-f1.txt", "003-f2.txt"]
    );
    assert_eq!(
        names(counter(10, 5, 0, RulePosition::Suffix)),
        ["f0-10.txt", "f1-15.txt", "f2-20.txt"]
    );
    assert_eq!(
        names(counter(98, 1, 2, RulePosition::ReplaceStem)),
        ["98.txt", "99.txt", "100.txt"],
        "a number wider than its width is not cut"
    );
    assert_eq!(
        names(counter(7, 0, 1, RulePosition::ReplaceStem)),
        ["7.txt", "7.txt", "7.txt"]
    );
    assert_eq!(
        names(counter(u32::MAX, u32::MAX, 0, RulePosition::ReplaceStem))[2],
        "12884901885.txt",
        "no overflow"
    );
}

#[test]
fn windows_names_are_problems_only_under_the_insensitive_rule() {
    let rules = [RenameRule::FindReplace {
        find: "ok".to_owned(),
        replace: "con".to_owned(),
        regex: false,
        case_sensitive: true,
        scope: RenameScope::Stem,
        all: true,
    }];
    let input = |name: &str, index| RenameInput {
        name: name.to_owned(),
        is_dir: false,
        modified_ms: None,
        created_ms: None,
        index,
    };
    let inputs = [input("ok", 0), input("ok.txt", 1)];
    for (rule, blocked) in [(CaseRule::Sensitive, false), (CaseRule::Insensitive, true)] {
        let out = apply_rules(&inputs, &rules, &ctx(rule));
        assert_eq!(out.rows[0].to, "con");
        assert_eq!(
            out.rows
                .iter()
                .any(|r| r.problems.iter().any(Problem::blocks)),
            blocked
        );
    }
    // A trailing dot or space is invalid there too.
    let trim = [RenameRule::Insert {
        text: ".".to_owned(),
        at: InsertAt::End,
    }];
    let out = apply_rules(&[input("name", 0)], &trim, &ctx(CaseRule::Insensitive));
    assert!(matches!(out.rows[0].problems[0], Problem::Invalid { .. }));
    let out = apply_rules(&[input("name", 0)], &trim, &ctx(CaseRule::Sensitive));
    assert!(out.rows[0].problems.is_empty());
}

// ---- plan_batch against a model ----

/// A folder of names to move, and what the model says must happen.
struct Case {
    entries: Vec<BatchEntry>,
    siblings: Vec<String>,
}

fn random_case(rng: &mut Rng, rule: CaseRule) -> Case {
    let count = 1 + rng.below(7);
    let base = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let mut froms: Vec<String> = Vec::new();
    for name in base.iter().take(count) {
        // Spellings differ only in case, which are separate names to a case-sensitive folder.
        froms.push(if rng.chance(3) {
            name.to_uppercase()
        } else {
            (*name).to_owned()
        });
    }
    let staying: Vec<String> = (0..rng.below(3)).map(|i| format!("stay{i}")).collect();
    let mut pool: Vec<String> = froms.clone();
    pool.extend(staying.iter().cloned());
    pool.extend(["new1", "new2", "NEW1"].map(String::from));
    let permute = rng.chance(2);
    let mut targets: Vec<String> = froms.clone();
    if permute {
        // A random permutation, with some case changes: swaps, rotations and chains appear.
        for i in (1..targets.len()).rev() {
            targets.swap(i, rng.below(i + 1));
        }
        for t in &mut targets {
            if rng.chance(4) {
                *t = if rng.chance(2) {
                    t.to_uppercase()
                } else {
                    t.to_lowercase()
                };
            }
        }
    } else {
        for t in &mut targets {
            *t = rng.pick(&pool).clone();
        }
    }
    let mut siblings = froms.clone();
    siblings.extend(staying);
    // The folder cannot hold two names that are one name to its rule.
    let mut seen = HashSet::new();
    siblings.retain(|n| seen.insert(fold(rule, n)));
    let kept: HashSet<String> = siblings.iter().cloned().collect();
    let entries = froms
        .into_iter()
        .zip(targets)
        .filter(|(from, _)| kept.contains(from))
        .map(|(from, to)| BatchEntry { from, to })
        .collect();
    Case { entries, siblings }
}

/// What the model says is wrong, without looking at `plan_batch`.
fn oracle_clashes(case: &Case, rule: CaseRule) -> bool {
    let moving: Vec<&BatchEntry> = case.entries.iter().filter(|e| e.from != e.to).collect();
    let vacated: HashSet<String> = moving.iter().map(|e| fold(rule, &e.from)).collect();
    let mut wanted = HashSet::new();
    for entry in &moving {
        let key = fold(rule, &entry.to);
        if !wanted.insert(key.clone()) {
            return true;
        }
        let staying = case
            .siblings
            .iter()
            .any(|s| fold(rule, s) == key && !vacated.contains(&key));
        if staying {
            return true;
        }
    }
    false
}

/// Runs the steps on a set of names (never overwriting) and returns the names left.
fn replay(case: &Case, steps: &[RenameStep], rule: CaseRule) -> Vec<String> {
    let mut names: HashMap<String, String> = case
        .siblings
        .iter()
        .map(|n| (fold(rule, n), n.clone()))
        .collect();
    let mut under: HashMap<usize, String> = HashMap::new();
    for step in steps {
        let entry = &case.entries[step.entry];
        let name_at = |place: Place| match place {
            Place::Source => entry.from.clone(),
            Place::Target => entry.to.clone(),
            Place::Temp => format!("\u{1}tmp{}", step.entry),
        };
        let (from, to) = (name_at(step.from), name_at(step.to));
        assert!(
            names.remove(&fold(rule, &from)).is_some(),
            "{from:?} is where the step expects it"
        );
        assert!(
            names.insert(fold(rule, &to), to.clone()).is_none(),
            "{to:?} was taken: nothing is overwritten"
        );
        under.insert(step.entry, to);
    }
    let mut left: Vec<String> = names.into_values().collect();
    left.sort();
    left
}

fn expected_names(case: &Case, rule: CaseRule) -> Vec<String> {
    let moving: HashSet<String> = case
        .entries
        .iter()
        .filter(|e| e.from != e.to)
        .map(|e| fold(rule, &e.from))
        .collect();
    let mut out: Vec<String> = case
        .siblings
        .iter()
        .filter(|s| !moving.contains(&fold(rule, s)))
        .cloned()
        .collect();
    out.extend(
        case.entries
            .iter()
            .filter(|e| e.from != e.to)
            .map(|e| e.to.clone()),
    );
    out.sort();
    out
}

#[test]
fn plan_batch_never_overwrites_and_reaches_the_targets() {
    let mut with_temp = 0;
    let mut chains = 0;
    for seed in 0..3000u64 {
        for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
            let mut rng = Rng::seeded(seed);
            let case = random_case(&mut rng, rule);
            let planned = plan_batch(&case.entries, &case.siblings, rule);
            let clashes = oracle_clashes(&case, rule);
            // The oracle ignores validity, which these names always have.
            assert_eq!(
                planned.is_err(),
                clashes,
                "seed {seed} {rule:?}: {:?} in {:?}",
                case.entries,
                case.siblings
            );
            let Ok(steps) = planned else { continue };
            let got = replay(&case, &steps, rule);
            assert_eq!(got, expected_names(&case, rule), "seed {seed} {rule:?}");
            // Every moving entry arrives exactly once.
            let arrivals = steps.iter().filter(|s| s.to == Place::Target).count();
            let moving = case.entries.iter().filter(|e| e.from != e.to).count();
            assert_eq!(arrivals, moving, "seed {seed}");
            if steps.iter().any(|s| s.to == Place::Temp) {
                with_temp += 1;
            } else if moving > 1 {
                chains += 1;
            }
        }
    }
    assert!(with_temp > 100, "cycles were exercised: {with_temp}");
    assert!(chains > 100, "chains were exercised: {chains}");
}

/// Runs the steps against a real provider, with each file holding its own original name.
fn run_on_provider(provider: &dyn Provider, root: &VfsPath, rule: CaseRule, case: &Case) {
    let folder: Tree = case
        .siblings
        .iter()
        .map(|n| (n.clone(), Node::File(n.as_bytes().to_vec())))
        .collect();
    populate(provider, root, &folder);
    let steps = plan_batch(&case.entries, &case.siblings, rule).expect("a plan");
    let at = |name: &str| child_path(root, OsStr::new(name), CaseRule::Sensitive).unwrap();
    for step in &steps {
        let entry = &case.entries[step.entry];
        let name_at = |place: Place| match place {
            Place::Source => entry.from.clone(),
            Place::Target => entry.to.clone(),
            Place::Temp => format!(".tmp-{}", step.entry),
        };
        provider
            .rename(&at(&name_at(step.from)), &at(&name_at(step.to)), false)
            .unwrap_or_else(|e| panic!("{step:?}: {e:?} for {case:?}", case = case.entries));
    }
    let moved: HashMap<String, &BatchEntry> = case
        .entries
        .iter()
        .filter(|e| e.from != e.to)
        .map(|e| (fold(rule, &e.from), e))
        .collect();
    let mut expected: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for name in &case.siblings {
        match moved.get(&fold(rule, name)) {
            Some(entry) => expected.insert(entry.to.clone(), name.as_bytes().to_vec()),
            None => expected.insert(name.clone(), name.as_bytes().to_vec()),
        };
    }
    let found: BTreeMap<String, Vec<u8>> = tree_of(provider, root)
        .into_iter()
        .map(|(k, v)| match v {
            Node::File(bytes) => (k, bytes),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(found, expected, "{:?} in {:?}", case.entries, case.siblings);
}

#[test]
fn plan_batch_steps_do_what_the_model_says_on_real_providers() {
    let mut ran = 0;
    for seed in 0..400u64 {
        for (name, rule) in [
            ("memory/sensitive", CaseRule::Sensitive),
            ("memory/insensitive", CaseRule::Insensitive),
            ("local", CaseRule::NATIVE),
        ] {
            let mut rng = Rng::seeded(seed);
            let case = random_case(&mut rng, rule);
            if plan_batch(&case.entries, &case.siblings, rule).is_err() {
                continue;
            }
            let dir = tempfile::tempdir().unwrap();
            let root = FilePath::from_path(dir.path()).unwrap();
            let base = VfsPath::File(root.clone());
            eprintln!("seed {seed} {name}");
            if name == "local" {
                run_on_provider(&LocalProvider::new(), &base, rule, &case);
            } else {
                run_on_provider(&MemoryProvider::new(root, rule), &base, rule, &case);
            }
            ran += 1;
        }
    }
    assert!(ran > 300);
}

// ---- whole jobs against the pure preview ----

fn random_folder(rng: &mut Rng) -> Tree {
    let pool = [
        "a",
        "B",
        "c.txt",
        "D.TXT",
        "e.tar.gz",
        ".hid",
        "f (2).md",
        "Ünï.txt",
        "g h.md",
        "i",
    ];
    let mut tree = Tree::new();
    for name in pool {
        if rng.chance(3) {
            continue;
        }
        if rng.chance(4) {
            tree.insert(name.to_owned(), Node::Dir);
            tree.insert(format!("{name}/inner"), file(name));
        } else {
            tree.insert(name.to_owned(), file(&format!("content of {name}")));
        }
    }
    tree
}

/// The tree after moving each top-level entry (and what is below it) to its new name at once.
fn moved(tree: &Tree, to: &BTreeMap<String, String>) -> Tree {
    let mut out = Tree::new();
    for (key, node) in tree {
        let (top, rest) = match key.split_once('/') {
            Some((top, rest)) => (top, format!("/{rest}")),
            None => (key.as_str(), String::new()),
        };
        let top = to.get(top).map_or(top, String::as_str);
        out.insert(format!("{top}{rest}"), node.clone());
    }
    out
}

fn clear<P: Provider + 'static>(h: &mut JournalHarness<P>) {
    h.provider.reset();
    let keys: Vec<String> = jwork(h).keys().cloned().collect();
    for key in keys.iter().rev() {
        let path = h.path(key);
        if let Ok(entry) = h.provider.stat(&path) {
            if entry.kind == waypoint_vfs::EntryKind::Directory {
                h.provider.remove_dir(&path).unwrap();
            } else {
                h.provider.remove_file(&path).unwrap();
            }
        }
    }
}

#[test]
fn jobs_agree_with_the_preview_and_undo_restores_the_names() {
    let mut done = 0;
    let mut refused = 0;
    each_journal_provider!(|h, rule, links| {
        let _ = links;
        for seed in 0..120u64 {
            let mut rng = Rng::seeded(seed);
            clear(&mut h);
            let start = random_folder(&mut rng);
            jbuild(&h, &start);
            let tops: Vec<String> = start.keys().filter(|k| !k.contains('/')).cloned().collect();
            if tops.is_empty() {
                continue;
            }
            // A random selection in a random order.
            let mut selected = tops.clone();
            for i in (1..selected.len()).rev() {
                selected.swap(i, rng.below(i + 1));
            }
            selected.truncate(1 + rng.below(selected.len()));
            let rules: Vec<RenameRule> = (0..1 + rng.below(3))
                .map(|_| match random_rule(&mut rng, false) {
                    // The provider's own times are not known to the model.
                    RenameRule::DateToken {
                        format,
                        position,
                        separator,
                        ..
                    } => RenameRule::DateToken {
                        source: DateSource::Today,
                        format,
                        position,
                        separator,
                    },
                    other => other,
                })
                .collect();

            // The model: the pure preview over what is in the folder.
            let inputs: Vec<RenameInput> = selected
                .iter()
                .enumerate()
                .map(|(index, name)| RenameInput {
                    name: name.clone(),
                    is_dir: start.get(name) == Some(&Node::Dir),
                    modified_ms: None,
                    created_ms: None,
                    index,
                })
                .collect();
            let preview = preview_batch_rename(&PreviewRequest {
                folders: vec![FolderInputs {
                    case_rule: rule,
                    siblings: tops.clone(),
                    inputs,
                }],
                rules: rules.clone(),
                now_ms: NOW,
                utc_offset_minutes: 0,
            });
            let refs: Vec<&str> = selected.iter().map(String::as_str).collect();
            let mut request = h.request(JobKind::BatchRename, &refs, None, None);
            request.rename = Some(RenameSpec {
                rules,
                utc_offset_minutes: 0,
                now_ms: Some(NOW),
            });
            let run = h.run_journalled(request);
            if preview.ready() {
                assert_eq!(run.state, JobState::Done, "seed {seed}: {:?}", run.failure);
                let map: BTreeMap<String, String> = preview
                    .rows
                    .iter()
                    .filter(|r| r.changed)
                    .map(|r| (r.from.clone(), r.to.clone()))
                    .collect();
                assert_eq!(jwork(&h), moved(&start, &map), "seed {seed}");
                assert!(partials(&jwork(&h)).is_empty());
                let entry = run.entry.expect("one entry");
                let undo = h.undo(entry);
                assert_eq!(undo.state, JobState::Done, "seed {seed}: {:?}", undo.undo);
                assert_eq!(jwork(&h), start, "seed {seed}: undo restores the names");
                let redo = h.redo(entry);
                assert_eq!(redo.state, JobState::Done, "seed {seed}");
                assert_eq!(jwork(&h), moved(&start, &map), "seed {seed}: redo");
                done += 1;
            } else {
                assert!(
                    matches!(run.state, JobState::Failed { .. }),
                    "seed {seed}: {:?}",
                    run.state
                );
                assert_eq!(jwork(&h), start, "seed {seed}: a refusal writes nothing");
                refused += 1;
            }
            assert!(h.journal.pending().is_empty());
        }
    });
    assert!(done > 100, "{done} jobs ran");
    assert!(refused > 30, "{refused} jobs were refused");
}

#[test]
fn every_name_the_rules_make_passes_the_providers_check() {
    // `validate_name` is what the preview judges by; a plan is only made for names it accepts.
    for seed in 0..200u64 {
        let mut rng = Rng::seeded(seed);
        let rules: Vec<RenameRule> = (0..1 + rng.below(3))
            .map(|_| random_rule(&mut rng, false))
            .collect();
        let mut inputs = random_inputs(&mut rng);
        // Every entry has a time, so a rule never fails for want of one.
        inputs.iter_mut().for_each(|i| i.modified_ms = Some(NOW));
        for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
            let out = apply_rules(&inputs, &rules, &ctx(rule));
            for row in &out.rows {
                let invalid = row
                    .problems
                    .iter()
                    .any(|p| matches!(p, Problem::Invalid { .. }));
                if row.changed {
                    assert_eq!(
                        invalid,
                        validate_name(OsStr::new(&row.to), rule).is_err(),
                        "seed {seed} {:?}",
                        row.to
                    );
                }
            }
        }
    }
}

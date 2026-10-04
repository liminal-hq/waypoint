// Random sequences of operations over random trees, checked against a simple model of what each
// operation should do, on the local provider and on the in-memory provider under both case rules.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::ffi::OsStr;

use common::*;
use waypoint_path::windows;
use waypoint_vfs::validate_name;

fn fold(rule: CaseRule, text: &str) -> String {
    match rule {
        CaseRule::Sensitive => text.to_owned(),
        CaseRule::Insensitive => windows::fold(text),
    }
}

fn parent_of(key: &str) -> &str {
    key.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn name_of(key: &str) -> &str {
    key.rsplit_once('/').map_or(key, |(_, name)| name)
}

fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

/// What the engine is expected to leave behind: the tree, and what is in the Trash.
struct Model {
    rule: CaseRule,
    tree: Tree,
    /// Trashed entries: where they were, what they were (the entry itself at `""`, the rest by
    /// path below it) and the receipt.
    trash: Vec<(String, Tree, TrashReceipt)>,
}

impl Model {
    fn find(&self, key: &str) -> Option<String> {
        let wanted = fold(self.rule, key);
        self.tree
            .keys()
            .find(|k| fold(self.rule, k) == wanted)
            .cloned()
    }

    fn is_dir(&self, key: &str) -> bool {
        key.is_empty() || self.tree.get(key) == Some(&Node::Dir)
    }

    fn children_names(&self, parent: &str) -> Vec<String> {
        self.tree
            .keys()
            .filter(|k| parent_of(k) == parent)
            .map(|k| name_of(k).to_owned())
            .collect()
    }

    /// The entry at `key` and everything below it, keyed by the path below `key` (`""` for the
    /// entry itself).
    fn subtree(&self, key: &str) -> Tree {
        let prefix = format!("{key}/");
        self.tree
            .iter()
            .filter_map(|(k, v)| {
                if k == key {
                    Some((String::new(), v.clone()))
                } else {
                    k.strip_prefix(&prefix)
                        .map(|rest| (rest.to_owned(), v.clone()))
                }
            })
            .collect()
    }

    fn remove_subtree(&mut self, key: &str) {
        let prefix = format!("{key}/");
        self.tree.retain(|k, _| k != key && !k.starts_with(&prefix));
    }

    fn put_subtree(&mut self, key: &str, sub: &Tree) {
        for (rest, node) in sub {
            let at = if rest.is_empty() {
                key.to_owned()
            } else {
                format!("{key}/{rest}")
            };
            self.tree.insert(at, node.clone());
        }
    }

    fn unique(&self, parent: &str, wanted: &str, extra_taken: &[&str]) -> String {
        let taken: Vec<String> = self
            .children_names(parent)
            .iter()
            .map(|n| fold(self.rule, n))
            .chain(extra_taken.iter().map(|n| fold(self.rule, n)))
            .collect();
        unique_full_name(&mut |n| taken.contains(&fold(self.rule, n)), wanted)
    }
}

/// What a successful operation does to the model.
type Apply<P> = Box<dyn FnOnce(&mut Model, &RunResult, &Harness<P>)>;

/// The kind of an error, for comparing with what the model predicts.
fn kind(error: &OpsError) -> &'static str {
    match error {
        // A folder that is gone is a not-found with a name of its own.
        OpsError::NotFound { .. } | OpsError::OriginMissingParent { .. } => "notFound",
        OpsError::NameInUse { .. } => "nameInUse",
        OpsError::InvalidName { .. } => "invalidName",
        OpsError::SameFolder => "sameFolder",
        OpsError::Io { .. } => "io",
        _ => "other",
    }
}

/// Whether a name is refused: by the provider's rules, and on Windows by the path itself, which
/// cannot join some names whatever the case rule.
fn invalid_name(name: &str, rule: CaseRule) -> bool {
    #[cfg(windows)]
    if waypoint_path::FilePath::parse(r"C:\")
        .and_then(|root| root.join(name))
        .is_err()
    {
        return true;
    }
    validate_name(OsStr::new(name), rule).is_err()
}

const NAMES: [&str; 14] = [
    "a",
    "B",
    "b",
    "c.txt",
    "C.TXT",
    "d (2).md",
    "e.tar.gz",
    ".hid",
    "New folder",
    "CON",
    "x:y",
    "trail.",
    "",
    "a/b",
];

/// Returns how many operations succeeded and how many were refused.
fn run_sequence<P: Provider + 'static>(
    mut h: Harness<P>,
    rule: CaseRule,
    links: bool,
    seed: u64,
    steps: usize,
) -> (usize, usize) {
    let (mut succeeded, mut refused) = (0, 0);
    let mut rng = Rng::seeded(seed);
    let mut initial = random_tree(&mut rng, 14);
    if !links {
        initial.retain(|_, n| !matches!(n, Node::Link(_)));
    }
    build(&h, &initial);
    let mut model = Model {
        rule,
        tree: initial,
        trash: Vec::new(),
    };
    assert_eq!(work_tree(&h), model.tree, "seed {seed}: the fixture");

    for step in 0..steps {
        h.provider.reset();
        let keys: Vec<String> = model.tree.keys().cloned().collect();
        let dirs: Vec<String> = std::iter::once(String::new())
            .chain(keys.iter().filter(|k| model.is_dir(k)).cloned())
            .collect();
        let context = |what: &str| format!("seed {seed} step {step}: {what}");
        // `(request, expected)`: the error kind the engine should refuse with, or none.
        let (request, expected, apply): (JobRequest, Option<&str>, Apply<P>);
        let choice = rng.below(if model.trash.is_empty() { 9 } else { 10 });
        let src = (!keys.is_empty()).then(|| rng.pick(&keys).clone());
        match (choice, src) {
            (0..=2, _) => {
                let parent = rng.pick(&dirs).clone();
                let name = *rng.pick(&NAMES);
                let folder = rng.chance(2);
                let keep_both = rng.chance(3);
                let mut r = h.request(
                    if folder {
                        JobKind::CreateFolder
                    } else {
                        JobKind::CreateFile
                    },
                    &[],
                    Some(&parent),
                    Some(name),
                );
                if keep_both {
                    r.options.conflict = Some(ConflictPolicy::KeepBoth);
                }
                let invalid = invalid_name(name, rule);
                let final_name = if keep_both && !invalid {
                    model.unique(&parent, name, &[])
                } else {
                    name.to_owned()
                };
                let taken = model.find(&join(&parent, &final_name)).is_some();
                expected = if invalid {
                    Some("invalidName")
                } else if taken {
                    Some("nameInUse")
                } else {
                    None
                };
                request = r;
                apply = Box::new(move |m, _, _| {
                    let key = join(&parent, &final_name);
                    m.tree.insert(
                        key,
                        if folder {
                            Node::Dir
                        } else {
                            Node::File(Vec::new())
                        },
                    );
                });
            }
            (3 | 4, Some(src)) => {
                let name = *rng.pick(&NAMES);
                request = h.request(JobKind::Rename, &[&src], None, Some(name));
                let parent = parent_of(&src).to_owned();
                let old = name_of(&src).to_owned();
                let target = join(&parent, name);
                let invalid = invalid_name(name, rule);
                expected = if invalid {
                    Some("invalidName")
                } else if name == old {
                    Some("sameFolder")
                } else if fold(rule, name) != fold(rule, &old) && model.find(&target).is_some() {
                    Some("nameInUse")
                } else {
                    None
                };
                apply = Box::new(move |m, _, _| {
                    let sub = m.subtree(&src);
                    m.remove_subtree(&src);
                    m.put_subtree(&target, &sub);
                });
            }
            (5 | 6, Some(src)) => {
                request = h.request(JobKind::Duplicate, &[&src], None, None);
                expected = None;
                let parent = parent_of(&src).to_owned();
                let old = name_of(&src).to_owned();
                let name = model.unique(&parent, &old, &[&old]);
                apply = Box::new(move |m, _, _| {
                    let sub = m.subtree(&src);
                    m.put_subtree(&join(&parent, &name), &sub);
                });
            }
            (7, Some(src)) => {
                request = h.request(JobKind::Delete, &[&src], None, None);
                expected = None;
                apply = Box::new(move |m, _, _| m.remove_subtree(&src));
            }
            (8, Some(src)) => {
                request = h.request(JobKind::Trash, &[&src], None, None);
                expected = None;
                apply = Box::new(move |m, result, _| {
                    let receipt = result.report.as_ref().unwrap().trashed[0].clone();
                    let sub = m.subtree(&src);
                    m.remove_subtree(&src);
                    m.trash.push((src, sub, receipt));
                });
            }
            (9, _) => {
                let index = rng.below(model.trash.len());
                let (original, _, receipt) = &model.trash[index];
                let original = original.clone();
                let parent = parent_of(&original).to_owned();
                request = JobRequest {
                    kind: JobKind::Restore,
                    sources: Sources::Locations {
                        locations: vec![h.trash.trashed_location(receipt)],
                    },
                    destination: None,
                    name: None,
                    options: JobOptions::default(),
                    origin_window: "main-1".to_owned(),
                    rename: None,
                    archive: None,
                };
                expected = if !model.is_dir(&parent) {
                    // A missing parent, or one that became a file.
                    Some(if model.find(&parent).is_some() {
                        "io"
                    } else {
                        "notFound"
                    })
                } else if model.find(&original).is_some() {
                    Some("nameInUse")
                } else {
                    None
                };
                apply = Box::new(move |m, _, _| {
                    let (_, sub, _) = m.trash.remove(index);
                    m.put_subtree(&original, &sub);
                });
            }
            _ => continue,
        }

        let planner_refusal = expected.is_some() && request.kind != JobKind::Restore;
        let result = h.run(request.clone());
        match (expected, &result.state) {
            (None, JobState::Done) => {
                succeeded += 1;
                apply(&mut model, &result, &h)
            }
            (Some(want), JobState::Failed { error, .. }) => {
                refused += 1;
                assert_eq!(
                    kind(error),
                    want,
                    "{}",
                    context(&format!("{request:?} -> {error:?}"))
                );
            }
            (want, state) => panic!(
                "{}",
                context(&format!("{request:?}: expected {want:?}, got {state:?}"))
            ),
        }
        if planner_refusal {
            assert_eq!(
                h.provider.write_calls(),
                0,
                "{}",
                context("a refusal wrote")
            );
        }
        h.provider.reset();
        let actual = work_tree(&h);
        assert!(partials(&actual).is_empty(), "{}", context("partials"));
        assert_eq!(actual, model.tree, "{}", context(&format!("{request:?}")));
        assert_eq!(h.trash.len(), model.trash.len(), "{}", context("the Trash"));
        assert!(h.store.violations().is_empty());
    }
    assert_eq!(
        h.replayed(),
        h.store.snapshot(),
        "seed {seed}: replay differs"
    );
    assert!(
        h.store.slots_in_use() == 0,
        "seed {seed}: a job still holds a slot"
    );
    (succeeded, refused)
}

#[test]
fn random_operations_agree_with_the_model_on_the_memory_provider() {
    let (mut succeeded, mut refused) = (0, 0);
    for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
        for seed in 0..40 {
            let (h, _guard) = memory_harness(rule);
            let (s, r) = run_sequence(h, rule, true, seed, 45);
            succeeded += s;
            refused += r;
        }
    }
    // The sequences are not all refusals, and not all successes.
    assert!(
        succeeded > 1000 && refused > 300,
        "{succeeded} done, {refused} refused"
    );
}

#[test]
fn random_operations_agree_with_the_model_on_the_local_provider() {
    let (mut succeeded, mut refused) = (0, 0);
    for seed in 100..112 {
        let (h, _guard) = local_harness();
        let (s, r) = run_sequence(h, CaseRule::NATIVE, cfg!(unix), seed, 30);
        succeeded += s;
        refused += r;
    }
    assert!(
        succeeded > 100 && refused > 30,
        "{succeeded} done, {refused} refused"
    );
}

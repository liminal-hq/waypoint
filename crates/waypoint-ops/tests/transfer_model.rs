// Random trees and random copy and move requests, checked against a small model of what each policy
// should leave behind: the destination equals the model (bytes, times, modes, link text), a copy
// leaves the source byte for byte, a move leaves what was not moved and nothing else, and a fault at
// any step leaves each item moved or untouched with nothing half written.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod xfer;

use std::collections::BTreeMap;

use waypoint_path::windows;
use xfer::*;

const CHUNK: usize = SMALL_CHUNK;
const NAMES: [&str; 11] = [
    "a", "B", "b", "c.txt", "C.TXT", "d (2).md", "e.tar.gz", ".hid", "é", "日本", "x y",
];
const POLICIES: [ConflictPolicy; 5] = [
    ConflictPolicy::Replace,
    ConflictPolicy::Skip,
    ConflictPolicy::KeepBoth,
    ConflictPolicy::MergeFolders,
    ConflictPolicy::ReplaceIfNewer,
];

#[derive(Debug, Clone, PartialEq)]
enum E {
    File {
        bytes: Vec<u8>,
        mtime: i64,
        mode: u32,
    },
    Dir {
        kids: Kids,
        mtime: i64,
        mode: u32,
        exact: bool,
    },
    Link(String),
}

type Kids = BTreeMap<String, E>;

fn fold(rule: CaseRule, name: &str) -> String {
    match rule {
        CaseRule::Sensitive => name.to_owned(),
        CaseRule::Insensitive => windows::fold(name),
    }
}

fn find(kids: &Kids, name: &str, rule: CaseRule) -> Option<String> {
    let wanted = fold(rule, name);
    kids.keys().find(|k| fold(rule, k) == wanted).cloned()
}

// ---- generation ----

fn gen_bytes(rng: &mut Rng) -> Vec<u8> {
    match rng.below(8) {
        0 => Vec::new(),
        1 => vec![0; 4 * CHUNK + 5], // sparse-ish: zeros across chunks
        2 => pattern(2 * CHUNK + rng.below(CHUNK), rng.below(200) as u8),
        3 => pattern(CHUNK, rng.below(200) as u8),
        4 => pattern(CHUNK + 1, rng.below(200) as u8),
        _ => pattern(rng.below(60), rng.below(200) as u8),
    }
}

fn gen_mtime(rng: &mut Rng) -> i64 {
    // Seconds between 2001 and 2030 with milliseconds, so times differ and compare.
    1_000_000_000_000 + rng.below(900_000) as i64 * 1_000_000 + rng.below(1000) as i64
}

fn gen_kids(rng: &mut Rng, depth: usize, rule: CaseRule, links: bool) -> Kids {
    let mut kids = Kids::new();
    let count = rng.below(if depth == 0 { 5 } else { 4 });
    for _ in 0..count {
        let name = (*rng.pick(&NAMES)).to_owned();
        if find(&kids, &name, rule).is_some() || find(&kids, &name, CaseRule::Insensitive).is_some()
        {
            continue;
        }
        let entry = match rng.below(10) {
            0..=2 if depth < 3 => E::Dir {
                kids: gen_kids(rng, depth + 1, rule, links),
                mtime: gen_mtime(rng),
                mode: *rng.pick(&[0o755, 0o700, 0o750]),
                exact: false,
            },
            3 if links => E::Link(
                (*rng.pick(&[
                    "target",
                    "../up",
                    "missing/dir/at/all",
                    "a",
                    "dir link",
                    "b/c",
                ]))
                .to_owned(),
            ),
            _ => E::File {
                bytes: gen_bytes(rng),
                mtime: gen_mtime(rng),
                mode: *rng.pick(&[0o644, 0o600, 0o755, 0o640]),
            },
        };
        kids.insert(name, entry);
    }
    kids
}

// ---- building and reading through a provider ----

fn build_kids(provider: &dyn Provider, folder: &VfsPath, kids: &Kids) {
    for (name, entry) in kids {
        let path = folder.join(name).unwrap();
        match entry {
            E::Dir { kids, .. } => {
                provider.create_dir(&path).unwrap();
                build_kids(provider, &path, kids);
            }
            E::File { bytes, .. } => {
                let mut w = provider
                    .create_write(&path, WriteOptions::exclusive())
                    .unwrap();
                std::io::Write::write_all(&mut w, bytes).unwrap();
                w.finish(false).unwrap();
            }
            E::Link(text) => provider.symlink(&path, text.as_ref()).unwrap(),
        }
    }
}

/// Sets times and modes once everything exists (a folder's time changes when it gains entries).
fn stamp_kids(provider: &dyn Provider, folder: &VfsPath, kids: &Kids, modes: bool) {
    for (name, entry) in kids {
        let path = folder.join(name).unwrap();
        match entry {
            E::Dir {
                kids, mtime, mode, ..
            } => {
                stamp_kids(provider, &path, kids, modes);
                stamp(provider, &path, *mtime, modes.then_some(*mode));
            }
            E::File { mtime, mode, .. } => stamp(provider, &path, *mtime, modes.then_some(*mode)),
            E::Link(_) => {}
        }
    }
}

fn stamp(provider: &dyn Provider, path: &VfsPath, mtime: i64, mode: Option<u32>) {
    provider
        .set_times(
            path,
            FileTimes {
                accessed: None,
                modified: Some(ms_to_time(mtime)),
            },
        )
        .unwrap();
    if let Some(mode) = mode {
        provider
            .set_permissions(
                path,
                Permissions {
                    mode: Some(mode),
                    readonly: mode & 0o222 == 0,
                },
            )
            .unwrap();
    }
}

fn read_kids(provider: &dyn Provider, folder: &VfsPath) -> Kids {
    let mut out = Kids::new();
    for entry in provider
        .list(folder, &CancelToken::new(), 0, &mut |_| {})
        .unwrap()
    {
        let name = entry.name.to_string_lossy().into_owned();
        let path = folder.join(&entry.name).unwrap();
        // A link reports its target's permissions, which may be nowhere (or a loop).
        let mode = if entry.kind == EntryKind::Symlink {
            0
        } else {
            provider
                .permissions(&path)
                .ok()
                .and_then(|p| p.mode)
                .map_or(0, |m| m & 0o7777)
        };
        let e = match entry.kind {
            EntryKind::Directory => E::Dir {
                kids: read_kids(provider, &path),
                mtime: entry.modified_ms.unwrap_or(0),
                mode,
                exact: false,
            },
            EntryKind::Symlink => E::Link(
                provider
                    .read_link(&path)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            ),
            _ => {
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut provider.open_read(&path).unwrap(), &mut bytes)
                    .unwrap();
                E::File {
                    bytes,
                    mtime: entry.modified_ms.unwrap_or(0),
                    mode,
                }
            }
        };
        out.insert(name, e);
    }
    out
}

/// Whether `actual` is what the model says. Times are compared for files and for folders the
/// operation made; modes where the platform has them.
fn same(model: &Kids, actual: &Kids, modes: bool, at: &str, why: &mut Vec<String>) {
    let names: std::collections::BTreeSet<&String> = model.keys().chain(actual.keys()).collect();
    for name in names {
        let here = format!("{at}/{name}");
        match (model.get(name), actual.get(name)) {
            (None, Some(_)) => why.push(format!("{here}: unexpected")),
            (Some(_), None) => why.push(format!("{here}: missing")),
            (
                Some(E::File {
                    bytes: a,
                    mtime: ta,
                    mode: ma,
                }),
                Some(E::File {
                    bytes: b,
                    mtime: tb,
                    mode: mb,
                }),
            ) => {
                if a != b {
                    why.push(format!("{here}: bytes differ ({} vs {})", a.len(), b.len()));
                }
                if (ta - tb).abs() > 1 {
                    why.push(format!("{here}: mtime {ta} vs {tb}"));
                }
                if modes && ma != mb {
                    why.push(format!("{here}: mode {ma:o} vs {mb:o}"));
                }
            }
            (Some(E::Link(a)), Some(E::Link(b))) => {
                if a != b {
                    why.push(format!("{here}: link {a} vs {b}"));
                }
            }
            (
                Some(E::Dir {
                    kids: ka,
                    mtime: ta,
                    mode: ma,
                    exact,
                }),
                Some(E::Dir {
                    kids: kb,
                    mtime: tb,
                    mode: mb,
                    ..
                }),
            ) => {
                if *exact {
                    if (ta - tb).abs() > 1 {
                        why.push(format!("{here}: folder mtime {ta} vs {tb}"));
                    }
                    if modes && ma != mb {
                        why.push(format!("{here}: folder mode {ma:o} vs {mb:o}"));
                    }
                }
                same(ka, kb, modes, &here, why);
            }
            (Some(a), Some(b)) => {
                why.push(format!("{here}: kinds differ ({} vs {})", kind(a), kind(b)))
            }
            (None, None) => unreachable!(),
        }
    }
}

fn kind(e: &E) -> &'static str {
    match e {
        E::File { .. } => "file",
        E::Dir { .. } => "dir",
        E::Link(_) => "link",
    }
}

// ---- the model ----

#[derive(Clone, Copy, PartialEq, Debug)]
enum Do {
    Skip,
    KeepBoth,
    Replace,
    Merge,
}

/// What a policy does to a clash, written out afresh (the engine's table is in `action_for`).
/// `None` is a choice that cannot settle it, which the test's answers turn into Skip.
fn decide(policy: ConflictPolicy, src: &E, ex: &E, nested: bool) -> Option<Do> {
    let (sd, ed) = (matches!(src, E::Dir { .. }), matches!(ex, E::Dir { .. }));
    use ConflictPolicy::*;
    match policy {
        Skip => Some(Do::Skip),
        KeepBoth => Some(Do::KeepBoth),
        Replace => {
            if sd != ed {
                // Refused: the test answers the error with Skip.
                Some(Do::Skip)
            } else if sd && nested {
                Some(Do::Merge)
            } else {
                Some(Do::Replace)
            }
        }
        MergeFolders => {
            if sd && ed {
                Some(Do::Merge)
            } else {
                None
            }
        }
        ReplaceIfNewer => {
            if sd && ed {
                Some(Do::Merge)
            } else if sd || ed {
                None
            } else {
                let newer = match (src, ex) {
                    (E::File { mtime: a, .. }, E::File { mtime: b, .. }) => a > b,
                    // A link has no time of its own that the engine compares in the model: it
                    // reads the link's own modification time, which the model does not track.
                    _ => return None,
                };
                Some(if newer { Do::Replace } else { Do::Skip })
            }
        }
    }
}

struct Sim {
    rule: CaseRule,
    policy: ConflictPolicy,
    moving: bool,
    /// (source path, destination path) of every file or link placed, for conservation checks.
    moved: Vec<(String, String)>,
}

fn mark_exact(e: &mut E) {
    if let E::Dir { kids, exact, .. } = e {
        *exact = true;
        for k in kids.values_mut() {
            mark_exact(k);
        }
    }
}

fn leaves(e: &E, path: &str, to: &str, out: &mut Vec<(String, String)>) {
    match e {
        E::Dir { kids, .. } => {
            for (n, k) in kids {
                leaves(k, &format!("{path}/{n}"), &format!("{to}/{n}"), out);
            }
        }
        _ => out.push((path.to_owned(), to.to_owned())),
    }
}

impl Sim {
    /// Places `src[name]` in `dst`. Returns whether it all went.
    fn place(
        &mut self,
        src: &mut Kids,
        src_path: &str,
        name: &str,
        dst: &mut Kids,
        dst_path: &str,
        nested: bool,
    ) -> bool {
        let entry = src[name].clone();
        let from = format!("{src_path}/{name}");
        let hit = find(dst, name, self.rule);
        let Some(key) = hit else {
            self.put_new(src, &from, name, dst, dst_path, name, entry);
            return true;
        };
        let existing = dst[&key].clone();
        let action = decide(self.policy, &entry, &existing, nested).unwrap_or(Do::Skip);
        match action {
            Do::Skip => false,
            Do::KeepBoth => {
                let unique = {
                    let taken = |n: &str| find(dst, n, self.rule).is_some();
                    unique_full_name(&mut |n| taken(n), name)
                };
                self.put_new(src, &from, name, dst, dst_path, &unique, entry);
                true
            }
            Do::Replace => {
                dst.remove(&key);
                self.put_new(src, &from, name, dst, dst_path, name, entry);
                true
            }
            Do::Merge => {
                let (E::Dir { kids: sk, .. }, E::Dir { .. }) = (&entry, &existing) else {
                    unreachable!("only folders merge")
                };
                let names: Vec<String> = sk.keys().cloned().collect();
                let mut whole = true;
                let mut src_kids = sk.clone();
                let E::Dir { kids: dk, .. } = dst.get_mut(&key).unwrap() else {
                    unreachable!()
                };
                let here = format!("{dst_path}/{key}");
                for child in &names {
                    whole &= self.place(&mut src_kids, &from, child, dk, &here, true);
                }
                if self.moving {
                    if whole {
                        src.remove(name);
                    } else if let Some(E::Dir { kids, .. }) = src.get_mut(name) {
                        *kids = src_kids;
                    }
                }
                whole
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn put_new(
        &mut self,
        src: &mut Kids,
        from: &str,
        name: &str,
        dst: &mut Kids,
        dst_path: &str,
        as_name: &str,
        mut entry: E,
    ) {
        let to = format!("{dst_path}/{as_name}");
        leaves(&entry, from, &to, &mut self.moved);
        mark_exact(&mut entry);
        dst.insert(as_name.to_owned(), entry);
        if self.moving {
            src.remove(name);
        }
    }
}

// ---- the scenarios ----

struct Scenario {
    src: Kids,
    dst: Kids,
    sources: Vec<String>,
    kind: JobKind,
    policy: ConflictPolicy,
    verify: bool,
    cross: bool,
    fast: bool,
}

fn scenario(seed: u64, rule: CaseRule, links: bool) -> Scenario {
    let mut rng = Rng::seeded(seed);
    let policy = *rng.pick(&POLICIES);
    // Whether a link is newer than a file is a matter of the link's own time, which the model does
    // not carry, so a replace-if-newer scenario has no links.
    let links = links && policy != ConflictPolicy::ReplaceIfNewer;
    let src = gen_kids(&mut rng, 0, rule, links);
    let mut dst = gen_kids(&mut rng, 0, rule, links);
    // Clashes: some sources' names (or their other-case spelling) exist in the destination, and
    // folders overlap so merges have something to merge.
    for (name, entry) in &src {
        if !rng.chance(2) {
            continue;
        }
        let spelled = if rng.chance(4) {
            name.to_uppercase()
        } else {
            name.clone()
        };
        if find(&dst, &spelled, CaseRule::Insensitive).is_some() {
            dst.retain(|k, _| {
                fold(CaseRule::Insensitive, k) != fold(CaseRule::Insensitive, &spelled)
            });
        }
        let clash = match (entry, rng.below(5)) {
            (E::Dir { kids, .. }, 0..=2) => {
                let mut overlap = Kids::new();
                for (k, v) in kids {
                    if !rng.chance(2) {
                        continue;
                    }
                    overlap.insert(
                        k.clone(),
                        match v {
                            // The same name with other content, so a merge meets clashes.
                            E::File { mtime, .. } => E::File {
                                bytes: gen_bytes(&mut rng),
                                mtime: gen_mtime(&mut rng).min(*mtime + 5_000_000),
                                mode: 0o644,
                            },
                            other => other.clone(),
                        },
                    );
                }
                E::Dir {
                    kids: overlap,
                    mtime: gen_mtime(&mut rng),
                    mode: 0o755,
                    exact: false,
                }
            }
            (_, 0 | 1) => E::Dir {
                kids: gen_kids(&mut rng, 2, rule, links),
                mtime: gen_mtime(&mut rng),
                mode: 0o755,
                exact: false,
            },
            _ => E::File {
                bytes: gen_bytes(&mut rng),
                mtime: gen_mtime(&mut rng),
                mode: 0o644,
            },
        };
        dst.insert(spelled, clash);
    }
    let mut names: Vec<String> = src.keys().filter(|_| rng.below(4) != 0).cloned().collect();
    if names.is_empty() {
        names = src.keys().take(1).cloned().collect();
    }
    // A shuffled order.
    for i in (1..names.len()).rev() {
        names.swap(i, rng.below(i + 1));
    }
    Scenario {
        src,
        dst,
        sources: names,
        kind: if rng.chance(2) {
            JobKind::Copy
        } else {
            JobKind::Move
        },
        policy,
        verify: rng.chance(3),
        cross: rng.chance(2),
        fast: rng.chance(2),
    }
}

fn setup<P: Provider + 'static>(h: &Harness<P>, s: &Scenario, modes: bool) {
    h.provider.create_dir(&h.path("src")).unwrap();
    h.provider.create_dir(&h.path("dst")).unwrap();
    build_kids(h.provider.as_ref(), &h.path("src"), &s.src);
    build_kids(h.provider.as_ref(), &h.path("dst"), &s.dst);
    stamp_kids(h.provider.as_ref(), &h.path("src"), &s.src, modes);
    stamp_kids(h.provider.as_ref(), &h.path("dst"), &s.dst, modes);
    h.provider.reset();
}

fn model(s: &Scenario, rule: CaseRule) -> (Kids, Kids, Vec<(String, String)>) {
    let mut sim = Sim {
        rule,
        policy: s.policy,
        moving: s.kind == JobKind::Move,
        moved: Vec::new(),
    };
    let mut src = s.src.clone();
    let mut dst = s.dst.clone();
    for name in &s.sources {
        sim.place(&mut src, "", name, &mut dst, "", false);
    }
    (src, dst, sim.moved)
}

fn answers() -> Answers {
    Answers {
        // A choice that cannot settle a clash is answered with Skip, for that source only.
        conflicts: Box::new(|conflicts| {
            vec![Resolution {
                source: Some(conflicts[0].source.clone()),
                policy: ConflictPolicy::Skip,
            }]
        }),
        // A Replace that is refused is skipped.
        errors: Box::new(|_, error| match error {
            OpsError::CannotReplace { .. } => Some(Decision::Skip),
            _ => None,
        }),
    }
}

fn request<P: Provider + 'static>(h: &Harness<P>, s: &Scenario) -> JobRequest {
    let sources: Vec<String> = s.sources.iter().map(|n| format!("src/{n}")).collect();
    let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
    let mut r = req(h, s.kind, &refs, "dst", Some(s.policy));
    r.options.verify = Some(s.verify);
    r
}

fn check<P: Provider + 'static>(
    h: &Harness<P>,
    s: &Scenario,
    rule: CaseRule,
    modes: bool,
    tag: &str,
) {
    let (want_src, want_dst, _) = model(s, rule);
    let got_src = read_kids(h.provider.as_ref(), &h.path("src"));
    let got_dst = read_kids(h.provider.as_ref(), &h.path("dst"));
    let mut why = Vec::new();
    same(&want_dst, &got_dst, modes, "dst", &mut why);
    // The source: exact for a copy (untouched); for a move, what stays is as it was.
    same(
        &want_src,
        &got_src,
        modes && s.kind == JobKind::Copy,
        "src",
        &mut why,
    );
    assert!(
        why.is_empty(),
        "{tag}: seed scenario failed:\n{}",
        why.join("\n")
    );
}

#[test]
fn random_copies_and_moves_equal_the_model_on_the_in_memory_provider() {
    // How much of each kind of thing the scenarios exercised, so the test cannot pass by never
    // meeting a clash.
    let (mut replaced, mut merged, mut skipped, mut renamed, mut verified, mut cross) =
        (0, 0, 0, 0, 0, 0);
    for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
        for seed in 0..400u64 {
            let s = scenario(seed, rule, true);
            let (mut h, _dir) = memory_harness(rule);
            volumes_then_setup(&h, &s);
            let tag = format!(
                "{rule:?} seed {seed} {:?} {:?} cross={} verify={}",
                s.kind, s.policy, s.cross, s.verify
            );
            let r = request(&h, &s);
            let result = run_transfer(&mut h, r, &mut answers(), &small(), &mut |_, _| {});
            assert!(
                result.state == JobState::Done,
                "{tag}: {:?} {:?}",
                result.state,
                result.failure.as_ref().map(|f| (&f.error, &f.item))
            );
            let report = result.report.as_ref().unwrap();
            replaced += report.transfer.replaced.len();
            merged += report.transfer.merged.len();
            skipped += report.counts.skipped as usize;
            renamed += report.renamed.len();
            verified += report.transfer.verified.iter().count();
            cross += usize::from(s.cross && s.kind == JobKind::Move);
            h.provider.reset();
            check(&h, &s, rule, true, &tag);
            assert!(leftovers(&work_tree(&h)).is_empty(), "{tag}");
        }
    }
    let stats = (replaced, merged, skipped, renamed, verified, cross);
    assert!(
        replaced > 40 && merged > 30 && skipped > 40 && renamed > 40 && verified > 40 && cross > 40,
        "{stats:?}"
    );
}

/// `src` exists and carries its volume already; the rest is built.
fn volumes_then_setup(h: &Harness<MemoryProvider>, s: &Scenario) {
    h.provider.create_dir(&h.path("src")).unwrap();
    if s.cross {
        memory(h).set_volume(&h.path("src"), VolumeId(2));
    }
    memory(h).enable_fast_copy(s.fast);
    h.provider.create_dir(&h.path("dst")).unwrap();
    build_kids(h.provider.as_ref(), &h.path("src"), &s.src);
    build_kids(h.provider.as_ref(), &h.path("dst"), &s.dst);
    stamp_kids(h.provider.as_ref(), &h.path("src"), &s.src, true);
    stamp_kids(h.provider.as_ref(), &h.path("dst"), &s.dst, true);
    h.provider.reset();
}

#[test]
fn random_copies_and_moves_equal_the_model_on_the_local_provider() {
    for seed in 1000..1100u64 {
        let rule = CaseRule::NATIVE;
        let s = scenario(seed, rule, cfg!(unix));
        let (mut h, _dir) = local_harness();
        setup(&h, &s, cfg!(unix));
        if s.cross {
            // A rename out of `src` is refused as crossing volumes, so a move copies and removes
            // on the real file system as it would between two drives.
            let src = h.path("src").display();
            h.provider
                .fail_always_where(Op::Rename, FaultKind::CrossesDevices, move |p| {
                    p.display().starts_with(&src)
                });
        }
        let tag = format!(
            "local seed {seed} {:?} {:?} cross={} verify={}",
            s.kind, s.policy, s.cross, s.verify
        );
        let r = request(&h, &s);
        let result = run_transfer(&mut h, r, &mut answers(), &small(), &mut |_, _| {});
        assert!(
            result.state == JobState::Done,
            "{tag}: {:?} {:?}",
            result.state,
            result.failure.as_ref().map(|f| (&f.error, &f.item))
        );
        h.provider.reset();
        check(&h, &s, rule, cfg!(unix), &tag);
        assert!(leftovers(&work_tree(&h)).is_empty(), "{tag}");
    }
}

#[test]
fn a_move_is_a_copy_followed_by_removing_what_was_placed() {
    // On every seed the destination of a move equals the destination of the same copy, and the
    // source of the move is the source of the copy less what was placed.
    for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
        for seed in 3000..3120u64 {
            let mut s = scenario(seed, rule, true);
            let mut results = Vec::new();
            for kind in [JobKind::Copy, JobKind::Move] {
                s.kind = kind;
                let (mut h, _dir) = memory_harness(rule);
                volumes_then_setup(&h, &s);
                let r = request(&h, &s);
                let result = run_transfer(&mut h, r, &mut answers(), &small(), &mut |_, _| {});
                assert_eq!(result.state, JobState::Done, "{rule:?} {seed} {kind:?}");
                h.provider.reset();
                results.push((
                    read_kids(h.provider.as_ref(), &h.path("src")),
                    read_kids(h.provider.as_ref(), &h.path("dst")),
                ));
            }
            let (copy, moved) = (&results[0], &results[1]);
            let mut why = Vec::new();
            same(&copy.1, &moved.1, true, "dst", &mut why);
            assert!(why.is_empty(), "{rule:?} {seed}: {}", why.join("\n"));
            // Everything a move left in the source was in the source before, unchanged.
            let mut why = Vec::new();
            for (name, e) in &moved.0 {
                match s.src.get(name) {
                    Some(original) => subset(original, e, &format!("src/{name}"), &mut why),
                    None => why.push(format!("src/{name}: appeared")),
                }
            }
            assert!(why.is_empty(), "{rule:?} {seed}: {}", why.join("\n"));
        }
    }
}

/// `after` is `before` less some entries, the rest unchanged.
fn subset(before: &E, after: &E, at: &str, why: &mut Vec<String>) {
    match (before, after) {
        (E::Dir { kids: a, .. }, E::Dir { kids: b, .. }) => {
            for (name, e) in b {
                match a.get(name) {
                    Some(original) => subset(original, e, &format!("{at}/{name}"), why),
                    None => why.push(format!("{at}/{name}: appeared")),
                }
            }
        }
        (E::File { bytes: a, .. }, E::File { bytes: b, .. }) if a == b => {}
        (E::Link(a), E::Link(b)) if a == b => {}
        _ => why.push(format!("{at}: changed")),
    }
}

#[test]
fn a_planner_refusal_never_reaches_the_provider_for_writing() {
    // Random requests that the planner must refuse: into itself, onto its own folder, a missing
    // source, and a destination with too little room.
    for seed in 0..60u64 {
        let rule = if seed % 2 == 0 {
            CaseRule::Sensitive
        } else {
            CaseRule::Insensitive
        };
        let s = scenario(seed + 5000, rule, true);
        let (mut h, _dir) = memory_harness(rule);
        volumes_then_setup(&h, &s);
        let first = s
            .src
            .iter()
            .find(|(_, e)| matches!(e, E::Dir { .. }))
            .map(|(n, _)| n.clone());
        let mut rng = Rng::seeded(seed);
        let mut cases: Vec<(JobRequest, &str)> = Vec::new();
        if let Some(dir) = &first {
            let mut r = h.request(
                s.kind,
                &[&format!("src/{dir}")],
                Some(&format!("src/{dir}")),
                None,
            );
            r.options.conflict = Some(*rng.pick(&POLICIES));
            cases.push((r, "into itself"));
        }
        let any = s.src.keys().next();
        if let Some(name) = any {
            cases.push((
                h.request(s.kind, &[&format!("src/{name}")], Some("src"), None),
                "same folder",
            ));
            cases.push((
                h.request(
                    s.kind,
                    &[&format!("src/{name}"), "src/__missing__"],
                    Some("dst"),
                    None,
                ),
                "missing",
            ));
            cases.push((
                h.request(
                    s.kind,
                    &[&format!("src/{name}")],
                    Some("dst/__missing__"),
                    None,
                ),
                "no destination",
            ));
        }
        for (request, why) in cases {
            h.provider.reset();
            let result = run_plain(&mut h, request);
            assert!(
                matches!(result.state, JobState::Failed { .. }),
                "{seed} {why}: {:?}",
                result.state
            );
            assert_eq!(h.provider.write_calls(), 0, "{seed} {why}");
        }
    }
}

// ---- faults ----

/// A fixed scenario with a clash, a merge, a replace and a new folder.
fn fault_scenario(kind: JobKind, policy: ConflictPolicy, cross: bool, links: bool) -> Scenario {
    let file = |bytes: Vec<u8>, mtime: i64| E::File {
        bytes,
        mtime,
        mode: 0o644,
    };
    let dir = |kids: Vec<(&str, E)>, mtime: i64| E::Dir {
        kids: kids.into_iter().map(|(n, e)| (n.to_owned(), e)).collect(),
        mtime,
        mode: 0o755,
        exact: false,
    };
    let src: Kids = [
        (
            "f".to_owned(),
            file(pattern(2 * CHUNK + 3, 1), 1_100_000_000_000),
        ),
        (
            "clash".to_owned(),
            file(pattern(CHUNK + 1, 2), 1_300_000_000_000),
        ),
        (
            "tree".to_owned(),
            dir(
                vec![
                    ("x", file(pattern(300, 3), 1_110_000_000_000)),
                    ("same", file(pattern(900, 4), 1_310_000_000_000)),
                    (
                        "sub",
                        dir(
                            vec![("y", file(pattern(1500, 5), 1_120_000_000_000))],
                            1_130_000_000_000,
                        ),
                    ),
                    ("ln", E::Link("x".to_owned())),
                ],
                1_140_000_000_000,
            ),
        ),
        (
            "fresh".to_owned(),
            dir(
                vec![("z", file(pattern(10, 6), 1_150_000_000_000))],
                1_160_000_000_000,
            ),
        ),
    ]
    .into_iter()
    .collect();
    let dst: Kids = [
        ("clash".to_owned(), file(pattern(50, 9), 1_200_000_000_000)),
        (
            "tree".to_owned(),
            dir(
                vec![
                    ("same", file(pattern(40, 8), 1_200_000_000_000)),
                    ("keep", file(pattern(20, 7), 1_200_000_000_000)),
                ],
                1_210_000_000_000,
            ),
        ),
    ]
    .into_iter()
    .collect();
    let mut src = src;
    if !links {
        if let Some(E::Dir { kids, .. }) = src.get_mut("tree") {
            kids.remove("ln");
        }
    }
    Scenario {
        src,
        dst,
        sources: vec!["f".into(), "clash".into(), "tree".into(), "fresh".into()],
        kind,
        policy,
        verify: false,
        cross,
        fast: false,
    }
}

/// Invariants after a run that was stopped by a fault or a cancel, whatever it did.
///
/// `double` says a second failure was scripted into the cleanup of the first, which can leave a
/// partial or an aside behind (the cleanup itself failed); those are set apart, and everything
/// under a name is still held to the same rule.
fn assert_consistent<P: Provider + 'static>(
    h: &Harness<P>,
    s: &Scenario,
    rule: CaseRule,
    double: bool,
    at: &str,
) {
    let got_src = unscratched(read_kids(h.provider.as_ref(), &h.path("src")), double);
    let got_dst = unscratched(read_kids(h.provider.as_ref(), &h.path("dst")), double);
    let (want_src, want_dst, moved) = model(s, rule);
    let moving = s.kind == JobKind::Move;
    let tree = work_tree(h);
    if !double {
        assert!(
            leftovers(&tree).is_empty(),
            "{at}: leftovers {:?}",
            leftovers(&tree)
        );
    }
    // Every file in the destination is whole: it is what was there before, or what the finished
    // job puts there. Nothing half-written is under a name.
    let empty = Kids::new();
    let mut stack = vec![(&got_dst, &want_dst, &s.dst, String::new())];
    while let Some((got, want, before, path)) = stack.pop() {
        for (name, g) in got {
            let here = format!("{path}/{name}");
            let w = want
                .get(name)
                .or_else(|| find(want, name, rule).and_then(|k| want.get(&k)));
            let b = before
                .get(name)
                .or_else(|| find(before, name, rule).and_then(|k| before.get(&k)));
            match g {
                E::File { bytes, .. } => {
                    let ok = [w, b]
                        .into_iter()
                        .flatten()
                        .any(|e| matches!(e, E::File { bytes: x, .. } if x == bytes));
                    assert!(
                        ok,
                        "{at}: dst{here} is neither what it was nor what it becomes"
                    );
                }
                E::Link(text) => {
                    let ok = [w, b]
                        .into_iter()
                        .flatten()
                        .any(|e| matches!(e, E::Link(x) if x == text));
                    assert!(ok, "{at}: dst{here} link");
                }
                E::Dir { kids, .. } => {
                    let ww = match w {
                        Some(E::Dir { kids, .. }) => Some(kids),
                        _ => None,
                    };
                    let bb = match b {
                        Some(E::Dir { kids, .. }) => Some(kids),
                        _ => None,
                    };
                    assert!(
                        ww.is_some() || bb.is_some() || find(want, name, rule).is_none(),
                        "{at}: dst{here} folder"
                    );
                    stack.push((kids, ww.unwrap_or(&empty), bb.unwrap_or(&empty), here));
                }
            }
        }
    }
    // Conservation: a file or link that was to be placed is in the source (whole) or at its
    // destination (whole) - for a move; for a copy it is always in the source.
    fn lookup<'a>(kids: &'a Kids, path: &str, rule: CaseRule) -> Option<&'a E> {
        let mut here = kids;
        let mut parts = path.trim_start_matches('/').split('/').peekable();
        while let Some(part) = parts.next() {
            let key = find(here, part, rule)?;
            let e = here.get(&key)?;
            if parts.peek().is_none() {
                return Some(e);
            }
            match e {
                E::Dir { kids, .. } => here = kids,
                _ => return None,
            }
        }
        None
    }
    for (from, to) in &moved {
        let original = lookup(&s.src, from, rule).expect("the model's source exists");
        let in_src = lookup(&got_src, from, rule).is_some_and(|e| content_eq(e, original));
        let in_dst = lookup(&got_dst, to, rule).is_some_and(|e| content_eq(e, original));
        if moving {
            assert!(in_src || in_dst, "{at}: {from} -> {to} is nowhere");
        } else {
            assert!(in_src, "{at}: a copy changed {from}");
        }
    }
    // The source holds nothing that was not there, unchanged.
    for (name, e) in &got_src {
        match s.src.get(name) {
            Some(original) => {
                let mut why = Vec::new();
                subset(original, e, &format!("src/{name}"), &mut why);
                assert!(why.is_empty(), "{at}: {}", why.join("; "));
            }
            None => panic!("{at}: src/{name} appeared"),
        }
    }
    if !moving {
        let mut why = Vec::new();
        same(&s.src, &got_src, false, "src", &mut why);
        assert!(
            why.is_empty(),
            "{at}: a copy changed its source: {}",
            why.join("; ")
        );
    }
    let _ = want_src;
}

/// Without the entries a failed cleanup left (only when `drop` is set).
fn unscratched(mut kids: Kids, drop: bool) -> Kids {
    if drop {
        kids.retain(|name, _| !name.starts_with(".waypoint-"));
        for e in kids.values_mut() {
            if let E::Dir { kids, .. } = e {
                *kids = unscratched(std::mem::take(kids), true);
            }
        }
    }
    kids
}

fn content_eq(a: &E, b: &E) -> bool {
    match (a, b) {
        (E::File { bytes: x, .. }, E::File { bytes: y, .. }) => x == y,
        (E::Link(x), E::Link(y)) => x == y,
        _ => false,
    }
}

/// Builds the world a scenario runs in, once per run.
type World<'a, P> = &'a dyn Fn(&Scenario) -> (Harness<P>, tempfile::TempDir);

fn memory_world(
    rule: CaseRule,
) -> impl Fn(&Scenario) -> (Harness<MemoryProvider>, tempfile::TempDir) {
    move |s| {
        let (h, dir) = memory_harness(rule);
        volumes_then_setup(&h, s);
        (h, dir)
    }
}

/// The local provider, with a rename out of `src` refused as crossing volumes when the scenario
/// is a cross-volume one, so a move copies and removes on the real file system.
fn local_world(s: &Scenario) -> (Harness<LocalProvider>, tempfile::TempDir) {
    let (h, dir) = local_harness();
    setup(&h, s, cfg!(unix));
    if s.cross {
        let src = h.path("src").display();
        h.provider
            .fail_always_where(Op::Rename, FaultKind::CrossesDevices, move |p| {
                p.display().starts_with(&src)
            });
    }
    (h, dir)
}

/// A clean run agrees with the model; then a failure at every call index, in turn, leaves each
/// item moved or untouched with nothing half written.
fn fault_sweep<P: Provider + 'static>(
    world: World<'_, P>,
    s: &Scenario,
    rule: CaseRule,
    label: &str,
) {
    let (mut h, _dir) = world(s);
    let r = request(&h, s);
    let result = run_transfer(&mut h, r, &mut answers(), &small(), &mut |_, _| {});
    assert_eq!(result.state, JobState::Done, "{label}");
    let calls = h.provider.calls();
    h.provider.reset();
    check(
        &h,
        s,
        rule,
        cfg!(unix) || label.starts_with("memory"),
        &format!("clean {label}"),
    );
    // Every step is hit, with the three kinds of failure taking turns (the cross product of kinds
    // is swept on a smaller scenario in `moves.rs`).
    let kinds = [
        FaultKind::PermissionDenied,
        FaultKind::StorageFull,
        FaultKind::Interrupted,
    ];
    for step in 1..=calls {
        let fault = kinds[(step + s.policy as usize) % 3];
        let (mut h, _dir) = world(s);
        h.provider.fail_at(step, fault);
        // Sometimes a second failure lands in the cleanup after the first.
        let double = step % 3 == 0;
        if double {
            h.provider.fail_at(step + 1, fault);
        }
        let r = request(&h, s);
        // Half of the runs answer errors with Skip, so the job goes on.
        let mut a = answers();
        if step % 2 == 0 {
            a.errors = Box::new(|_, _| Some(Decision::Skip));
        }
        let result = run_transfer(&mut h, r, &mut a, &small(), &mut |_, _| {});
        h.provider.reset();
        let at = format!("{label} {fault:?} at {step}/{calls}");
        assert!(
            matches!(
                result.state,
                JobState::Done | JobState::Failed { .. } | JobState::Waiting { .. }
            ),
            "{at}: {:?}",
            result.state
        );
        assert_consistent(&h, s, rule, double, &at);
    }
}

/// A cancel at every call index, in turn, leaves each item moved or untouched.
fn cancel_sweep<P: Provider + 'static>(
    world: World<'_, P>,
    s: &Scenario,
    rule: CaseRule,
    label: &str,
) {
    let (mut h, _dir) = world(s);
    let r = request(&h, s);
    let result = run_transfer(&mut h, r, &mut answers(), &small(), &mut |_, _| {});
    assert_eq!(result.state, JobState::Done, "{label}");
    let calls = h.provider.calls();
    let mut cancelled = 0;
    for step in 1..=calls {
        let (mut h, _dir) = world(s);
        let r = request(&h, s);
        let result = run_transfer(&mut h, r, &mut answers(), &small(), &mut |h, token| {
            h.provider.cancel_at(step, token)
        });
        h.provider.reset();
        let at = format!("{label} cancel at {step}/{calls}");
        match &result.state {
            JobState::Cancelled => cancelled += 1,
            JobState::Done => {}
            other => panic!("{at}: {other:?}"),
        }
        assert_consistent(&h, s, rule, false, &at);
        // A job that was cancelled before it finished left nothing running, and the store agrees.
        assert!(h.store.violations().is_empty(), "{at}");
    }
    assert!(cancelled > calls / 2, "{label}: {cancelled}/{calls}");
}

#[test]
fn a_fault_at_any_step_leaves_each_item_moved_or_untouched_whatever_the_policy() {
    for rule in [CaseRule::Sensitive, CaseRule::Insensitive] {
        for kind in [JobKind::Copy, JobKind::Move] {
            for cross in [false, true] {
                for policy in POLICIES {
                    let s = fault_scenario(kind, policy, cross, true);
                    let label = format!("memory {rule:?} {kind:?} {policy:?} cross={cross}");
                    fault_sweep(&memory_world(rule), &s, rule, &label);
                }
            }
        }
    }
}

#[test]
fn a_fault_at_any_step_leaves_each_item_moved_or_untouched_on_the_local_provider() {
    for kind in [JobKind::Copy, JobKind::Move] {
        for cross in [false, true] {
            for policy in [ConflictPolicy::Replace, ConflictPolicy::MergeFolders] {
                let s = fault_scenario(kind, policy, cross, cfg!(unix));
                let label = format!("local {kind:?} {policy:?} cross={cross}");
                fault_sweep(&local_world, &s, CaseRule::NATIVE, &label);
            }
        }
    }
}

#[test]
fn a_cancel_at_any_step_leaves_each_item_moved_or_untouched_whatever_the_policy() {
    for kind in [JobKind::Copy, JobKind::Move] {
        for cross in [false, true] {
            for policy in POLICIES {
                let s = fault_scenario(kind, policy, cross, true);
                let label = format!("memory {kind:?} {policy:?} cross={cross}");
                cancel_sweep(
                    &memory_world(CaseRule::Sensitive),
                    &s,
                    CaseRule::Sensitive,
                    &label,
                );
            }
        }
    }
}

#[test]
fn a_cancel_at_any_step_leaves_each_item_moved_or_untouched_on_the_local_provider() {
    for kind in [JobKind::Copy, JobKind::Move] {
        for cross in [false, true] {
            for policy in [ConflictPolicy::Replace, ConflictPolicy::MergeFolders] {
                let s = fault_scenario(kind, policy, cross, cfg!(unix));
                let label = format!("local {kind:?} {policy:?} cross={cross}");
                cancel_sweep(&local_world, &s, CaseRule::NATIVE, &label);
            }
        }
    }
}

#[test]
fn enospc_after_n_bytes_leaves_no_partial_and_a_whole_destination() {
    // The nth write fails with a full disk, for every n across several files.
    for kind in [JobKind::Copy, JobKind::Move] {
        let s = fault_scenario(kind, ConflictPolicy::KeepBoth, true, true);
        let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
        volumes_then_setup(&h, &s);
        let r = request(&h, &s);
        let result = run_transfer(&mut h, r, &mut answers(), &small(), &mut |_, _| {});
        assert_eq!(result.state, JobState::Done);
        let writes = h.provider.calls_of(Op::Write);
        assert!(writes > 8, "{writes}");
        for n in 1..=writes {
            let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
            volumes_then_setup(&h, &s);
            h.provider.fail_nth(Op::Write, n, FaultKind::StorageFull);
            let r = request(&h, &s);
            let result = run_transfer(&mut h, r, &mut answers(), &small(), &mut |_, _| {});
            h.provider.reset();
            let at = format!("{kind:?} write {n}/{writes}");
            assert!(
                matches!(
                    &result.state,
                    JobState::Failed {
                        error: OpsError::NotEnoughSpace { .. },
                        ..
                    }
                ),
                "{at}: {:?}",
                result.state
            );
            assert_consistent(&h, &s, CaseRule::Sensitive, false, &at);
        }
    }
}

#[test]
fn a_cancel_after_every_chunk_of_one_big_file_removes_the_partial() {
    for cross in [false, true] {
        let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
        h.provider.create_dir(&h.path("src")).unwrap();
        if cross {
            memory(&h).set_volume(&h.path("src"), VolumeId(2));
        }
        h.provider.create_dir(&h.path("dst")).unwrap();
        let big = pattern(12 * CHUNK + 7, 3);
        put_bytes(&h, "src/big", &big);
        h.provider.reset();
        let r = req(&h, JobKind::Copy, &["src/big"], "dst", None);
        let result = run_transfer(&mut h, r, &mut Answers::default(), &small(), &mut |_, _| {});
        done(&result);
        let writes = h.provider.calls_of(Op::Write);
        assert_eq!(writes, 13);
        for n in 1..=h.provider.calls() {
            let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
            h.provider.create_dir(&h.path("src")).unwrap();
            if cross {
                memory(&h).set_volume(&h.path("src"), VolumeId(2));
            }
            h.provider.create_dir(&h.path("dst")).unwrap();
            put_bytes(&h, "src/big", &big);
            h.provider.reset();
            let r = req(&h, JobKind::Copy, &["src/big"], "dst", None);
            let result = run_transfer(&mut h, r, &mut Answers::default(), &small(), &mut |h, t| {
                h.provider.cancel_at(n, t)
            });
            h.provider.reset();
            let dst = read_kids(h.provider.as_ref(), &h.path("dst"));
            match result.state {
                JobState::Cancelled => {
                    assert!(dst.is_empty(), "{n}: {:?}", dst.keys().collect::<Vec<_>>())
                }
                JobState::Done => {
                    assert!(matches!(&dst["big"], E::File { bytes, .. } if *bytes == big))
                }
                ref other => panic!("{n}: {other:?}"),
            }
            assert_eq!(read_bytes(&h, "src/big"), big);
        }
    }
}

// ---- other shapes ----

#[cfg(unix)]
#[test]
fn names_that_are_not_utf_8_are_copied_and_moved_under_their_own_bytes() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    for local in [true, false] {
        let names: Vec<OsString> = vec![
            OsString::from_vec(b"caf\xe9".to_vec()),
            OsString::from_vec(b"\xff\xfe-two".to_vec()),
            OsString::from_vec(b"dir-\x80".to_vec()),
        ];
        macro_rules! body {
            ($h:ident) => {{
                $h.provider.create_dir(&$h.path("src")).unwrap();
                $h.provider.create_dir(&$h.path("dst")).unwrap();
                let src = $h.path("src");
                for (n, name) in names.iter().enumerate() {
                    let path = src.join(name).unwrap();
                    if n == 2 {
                        $h.provider.create_dir(&path).unwrap();
                        let inner = path.join(&names[0]).unwrap();
                        let mut w = $h.provider.create_write(&inner, WriteOptions::exclusive()).unwrap();
                        std::io::Write::write_all(&mut w, b"inner").unwrap();
                        w.finish(false).unwrap();
                    } else {
                        let mut w = $h.provider.create_write(&path, WriteOptions::exclusive()).unwrap();
                        std::io::Write::write_all(&mut w, format!("body {n}").as_bytes()).unwrap();
                        w.finish(false).unwrap();
                    }
                }
                $h.provider.symlink(&src.join(OsString::from_vec(b"ln-\xc3".to_vec())).unwrap(), std::ffi::OsStr::new("target")).unwrap();
                $h.provider.reset();
                let listed = $h.provider.list(&src, &CancelToken::new(), 0, &mut |_| {}).unwrap();
                let sources: Vec<Location> = listed.iter().map(|e| src.join(&e.name).unwrap().to_location()).collect();
                let mut request = $h.request(JobKind::Copy, &[], Some("dst"), None);
                request.sources = Sources::Locations { locations: sources.clone() };
                let result = run_plain(&mut $h, request);
                done(&result);
                let dst = $h.provider.list(&$h.path("dst"), &CancelToken::new(), 0, &mut |_| {}).unwrap();
                let mut got: Vec<OsString> = dst.iter().map(|e| e.name.clone()).collect();
                got.sort();
                let mut want: Vec<OsString> = listed.iter().map(|e| e.name.clone()).collect();
                want.sort();
                assert_eq!(got, want);
                let inner = $h.path("dst").join(&names[2]).unwrap().join(&names[0]).unwrap();
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut $h.provider.open_read(&inner).unwrap(), &mut bytes).unwrap();
                assert_eq!(bytes, b"inner");
                // And moved back the other way, from `dst` to `src` is refused as a clash; move to a
                // third folder instead.
                $h.provider.create_dir(&$h.path("third")).unwrap();
                let moved: Vec<Location> = dst.iter().map(|e| $h.path("dst").join(&e.name).unwrap().to_location()).collect();
                let mut request = $h.request(JobKind::Move, &[], Some("third"), None);
                request.sources = Sources::Locations { locations: moved };
                let result = run_plain(&mut $h, request);
                done(&result);
                let third = $h.provider.list(&$h.path("third"), &CancelToken::new(), 0, &mut |_| {}).unwrap();
                let mut got: Vec<OsString> = third.iter().map(|e| e.name.clone()).collect();
                got.sort();
                assert_eq!(got, want);
                assert!($h.provider.list(&$h.path("dst"), &CancelToken::new(), 0, &mut |_| {}).unwrap().is_empty());
            }};
        }
        if local {
            let (mut h, _dir) = local_harness();
            body!(h);
        } else {
            let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
            body!(h);
        }
    }
}

#[test]
fn a_file_across_the_real_chunk_size_copies_and_verifies() {
    // 8 MiB + 5 bytes: one full real chunk and a short one.
    let (mut h, _dir) = local_harness();
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    let big = pattern(8 * 1024 * 1024 + 5, 11);
    put_bytes(&h, "src/big", &big);
    h.provider.reset();
    let mut request = req(&h, JobKind::Copy, &["src/big"], "dst", None);
    request.options.verify = Some(true);
    let result = run_transfer(
        &mut h,
        request,
        &mut Answers::default(),
        &TransferOptions::default(),
        &mut |_, _| {},
    );
    done(&result);
    assert_eq!(read_bytes(&h, "dst/big"), big);
    assert_eq!(result.report.unwrap().transfer.verified.unwrap().files, 1);
}

/// Prints the throughput of the copy engine on the local provider. Not a gate: run it with
/// `cargo test -p waypoint-ops --release --test transfer_model -- --ignored --nocapture`.
#[test]
#[ignore = "a throughput measurement, not a test of behaviour"]
fn throughput_of_a_256_mib_copy_on_the_local_provider() {
    use std::io::Write;
    use std::time::Instant;
    let (mut h, _dir) = local_harness();
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    // Zero-filled, written in 4 MiB pieces; the file system may keep it sparse.
    let mut w = h
        .provider
        .create_write(&h.path("src/big"), WriteOptions::exclusive())
        .unwrap();
    let zeros = vec![0u8; 4 * 1024 * 1024];
    for _ in 0..64 {
        w.write_all(&zeros).unwrap();
    }
    w.finish(true).unwrap();
    h.provider.reset();
    let size = 256.0 * 1024.0 * 1024.0;
    for (label, verify, chunk) in [
        ("fast path (reflink / copy_file_range)", false, CHUNK_BYTES),
        ("8 MiB chunk loop, verified with BLAKE3", true, CHUNK_BYTES),
    ] {
        let mut request = req(
            &h,
            JobKind::Copy,
            &["src/big"],
            "dst",
            Some(ConflictPolicy::KeepBoth),
        );
        request.options.verify = Some(verify);
        let options = TransferOptions {
            chunk_bytes: chunk,
            ..TransferOptions::default()
        };
        let started = Instant::now();
        let result = run_transfer(
            &mut h,
            request,
            &mut Answers::default(),
            &options,
            &mut |_, _| {},
        );
        let secs = started.elapsed().as_secs_f64();
        done(&result);
        println!(
            "{label}: {:.0} MB/s ({secs:.2} s for 256 MiB)",
            size / 1e6 / secs
        );
    }
}

// Name handling the operations share: comparing names and paths under a case rule, splitting a name
// into its stem and extension, and choosing a free name.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::{OsStr, OsString};

use waypoint_path::{windows, CaseRule, VfsPath};

/// The longest name a provider accepts, in bytes (the strictest of its two rules that a suffix can
/// push a name past).
const MAX_NAME_BYTES: usize = 255;

/// The key two names share exactly when they are the same name under `rule`.
pub fn fold_name(name: &OsStr, rule: CaseRule) -> OsString {
    match rule {
        CaseRule::Sensitive => name.to_owned(),
        CaseRule::Insensitive => OsString::from(windows::fold(&name.to_string_lossy())),
    }
}

/// Whether two names are the same name under `rule`.
pub fn same_name(a: &OsStr, b: &OsStr, rule: CaseRule) -> bool {
    fold_name(a, rule) == fold_name(b, rule)
}

/// Whether two paths name the same entry under `rule`, comparing them lexically (links and hard
/// links are only the file system's to judge).
pub fn same_path(a: &VfsPath, b: &VfsPath, rule: CaseRule) -> bool {
    match rule {
        CaseRule::Sensitive => a == b,
        CaseRule::Insensitive => windows::fold(&a.display()) == windows::fold(&b.display()),
    }
}

/// Whether `path` is `ancestor` or lies below it.
pub fn is_within(path: &VfsPath, ancestor: &VfsPath, rule: CaseRule) -> bool {
    let mut current = Some(path.clone());
    while let Some(here) = current {
        if same_path(&here, ancestor, rule) {
            return true;
        }
        current = here.parent();
    }
    false
}

/// The bytes of a name, as an archive stores it (a name that is not Unicode is lossy on Windows,
/// which holds none).
pub fn name_bytes(name: &OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        std::os::unix::ffi::OsStrExt::as_bytes(name).to_vec()
    }
    #[cfg(not(unix))]
    {
        name.to_string_lossy().into_owned().into_bytes()
    }
}

/// The last component of a path.
pub fn file_name_of(path: &VfsPath) -> Option<OsString> {
    path.file_name()
}

/// Splits a name into the part to number and the extension to keep after the number. The
/// extension is what follows the last dot (two parts for `.tar.gz` and its kin), and a leading
/// dot or a trailing one starts no extension, so `.profile` and `end.` are all stem.
pub fn split_name(name: &str) -> (&str, &str) {
    let Some(dot) = name.rfind('.') else {
        return (name, "");
    };
    if dot == 0 || dot + 1 == name.len() {
        return (name, "");
    }
    let (stem, ext) = name.split_at(dot);
    if let Some(inner) = stem.rfind('.') {
        let tar = &stem[inner..];
        if inner > 0 && tar.eq_ignore_ascii_case(".tar") {
            return name.split_at(inner);
        }
    }
    (stem, ext)
}

/// Splits a trailing ` (n)` off a stem: `"report (2)"` is `("report", Some(2))`.
fn split_count(stem: &str) -> (&str, Option<u32>) {
    let Some(open) = stem.rfind(" (") else {
        return (stem, None);
    };
    let Some(digits) = stem[open + 2..].strip_suffix(')') else {
        return (stem, None);
    };
    match digits.parse::<u32>() {
        Ok(n) if n >= 1 && !digits.starts_with('0') && !digits.starts_with('+') => {
            (&stem[..open], Some(n))
        }
        _ => (stem, None),
    }
}

/// Shortens `stem` until `stem + suffix` fits in a name.
fn fit(stem: &str, suffix: &str) -> String {
    let mut stem = stem.to_owned();
    while stem.len() + suffix.len() > MAX_NAME_BYTES && stem.pop().is_some() {}
    format!("{stem}{suffix}")
}

/// A name that is not `taken`: `stem` + `ext` itself when free, otherwise `stem (2)` + `ext`, then
/// `(3)` and so on. A stem that already ends in a count continues from it (`report (2)` becomes
/// `report (3)`), so duplicating a duplicate does not nest. `taken` decides what a clash is, so
/// the caller applies the provider's case rule and any names already handed out in the same
/// batch. The result is a pure function of its inputs.
pub fn unique_name(taken: &mut dyn FnMut(&str) -> bool, stem: &str, ext: &str) -> String {
    let first = fit(stem, ext);
    if !taken(&first) {
        return first;
    }
    let (base, count) = split_count(stem);
    let mut n = count.map_or(2, |c| c.saturating_add(1)).max(2);
    loop {
        let candidate = fit(base, &format!(" ({n}){ext}"));
        if !taken(&candidate) {
            return candidate;
        }
        n = n.checked_add(1).expect("names are not exhausted");
    }
}

/// `unique_name` for a whole name, splitting off its extension.
pub fn unique_full_name(taken: &mut dyn FnMut(&str) -> bool, name: &str) -> String {
    let (stem, ext) = split_name(name);
    unique_name(taken, stem, ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    fn pick(taken: &[&str], name: &str) -> String {
        let taken = set(taken);
        unique_full_name(&mut |n| taken.contains(n), name)
    }

    #[test]
    fn a_free_name_is_kept() {
        assert_eq!(pick(&[], "a.txt"), "a.txt");
    }

    #[test]
    fn a_taken_name_gets_the_next_free_count_before_its_extension() {
        assert_eq!(pick(&["a.txt"], "a.txt"), "a (2).txt");
        assert_eq!(pick(&["a.txt", "a (2).txt"], "a.txt"), "a (3).txt");
        assert_eq!(pick(&["a.txt", "a (3).txt"], "a.txt"), "a (2).txt");
    }

    #[test]
    fn a_counted_name_continues_its_count() {
        assert_eq!(pick(&["a (2).txt"], "a (2).txt"), "a (3).txt");
        assert_eq!(pick(&["a (1)"], "a (1)"), "a (2)");
        assert_eq!(pick(&["a (07)"], "a (07)"), "a (07) (2)");
    }

    #[test]
    fn extensions_split_where_a_person_would() {
        assert_eq!(split_name("a.txt"), ("a", ".txt"));
        assert_eq!(split_name("a.b.txt"), ("a.b", ".txt"));
        assert_eq!(split_name("a.tar.gz"), ("a", ".tar.gz"));
        assert_eq!(split_name("A.TAR.xz"), ("A", ".TAR.xz"));
        assert_eq!(split_name(".profile"), (".profile", ""));
        assert_eq!(split_name(".tar.gz"), (".tar", ".gz"));
        assert_eq!(split_name("end."), ("end.", ""));
        assert_eq!(split_name("plain"), ("plain", ""));
        assert_eq!(pick(&["a.tar.gz"], "a.tar.gz"), "a (2).tar.gz");
        assert_eq!(pick(&[".profile"], ".profile"), ".profile (2)");
    }

    #[test]
    fn a_long_name_is_shortened_to_fit() {
        let name = format!("{}.txt", "x".repeat(251));
        assert_eq!(name.len(), 255);
        let picked = pick(&[&name], &name);
        assert!(picked.len() <= 255, "{}", picked.len());
        assert!(picked.ends_with(" (2).txt"));
        assert_ne!(picked, name);
    }

    #[test]
    fn it_never_collides_and_is_stable() {
        // Every subset of the first few candidates being taken still gives a free name, and the
        // same input gives the same answer.
        let stems = ["a", "a (2)", "résumé", ".x", "n (9)", "q.tar"];
        for stem in stems {
            for mask in 0u32..64 {
                let mut taken: HashSet<String> = HashSet::new();
                taken.insert(format!("{stem}.md"));
                let (base, _) = split_count(stem);
                for bit in 0..6 {
                    if mask & (1 << bit) != 0 {
                        taken.insert(format!("{base} ({}).md", bit + 2));
                    }
                }
                let first = unique_name(&mut |n| taken.contains(n), stem, ".md");
                let again = unique_name(&mut |n| taken.contains(n), stem, ".md");
                assert!(!taken.contains(&first), "{stem} {mask}: {first}");
                assert_eq!(first, again);
            }
        }
    }

    #[test]
    fn names_compare_under_the_case_rule() {
        let a = OsStr::new("Readme.MD");
        let b = OsStr::new("readme.md");
        assert!(!same_name(a, b, CaseRule::Sensitive));
        assert!(same_name(a, b, CaseRule::Insensitive));
        assert!(same_name(a, a, CaseRule::Sensitive));
    }

    #[cfg(unix)]
    #[test]
    fn paths_are_within_themselves_and_their_ancestors_by_the_rule() {
        let p = |s: &str| VfsPath::parse_input(s).unwrap();
        assert!(is_within(&p("/a/b/c"), &p("/a/b"), CaseRule::Sensitive));
        assert!(is_within(&p("/a/b"), &p("/a/b"), CaseRule::Sensitive));
        assert!(!is_within(&p("/a/bc"), &p("/a/b"), CaseRule::Sensitive));
        assert!(!is_within(&p("/a/B/c"), &p("/a/b"), CaseRule::Sensitive));
        assert!(is_within(&p("/a/B/c"), &p("/a/b"), CaseRule::Insensitive));
        assert!(!is_within(&p("/a"), &p("/a/b"), CaseRule::Insensitive));
    }
}

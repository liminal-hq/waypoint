// The write primitives, run against `LocalProvider` (over a temporary directory) and `MemoryProvider`
// (both case rules) with the same expectations, so the in-memory provider that later tests build on
// is known to behave as the real one does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Some tests need Unix semantics, so some items are unused on Windows.
#![cfg_attr(windows, allow(unused_imports, dead_code))]

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tempfile::TempDir;
use waypoint_path::{CaseRule, FilePath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{
    child_path, from_io, CancelToken, EntryKind, FileTimes, LocalProvider, MemOp, MemoryProvider,
    Permissions, Provider, VolumeId, VolumeSpace, WriteOptions,
};

struct Fixture {
    name: &'static str,
    provider: Arc<dyn Provider>,
    root: VfsPath,
    _dir: TempDir,
}

fn fixture(name: &'static str, build: impl FnOnce(FilePath) -> Arc<dyn Provider>) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    Fixture {
        name,
        provider: build(root.clone()),
        root: VfsPath::File(root),
        _dir: dir,
    }
}

/// The real provider and the in-memory one under each case rule.
fn fixtures() -> Vec<Fixture> {
    vec![
        fixture("local", |_| Arc::new(LocalProvider::new())),
        fixture("memory/sensitive", |root| {
            Arc::new(MemoryProvider::new(root, CaseRule::Sensitive))
        }),
        fixture("memory/insensitive", |root| {
            Arc::new(MemoryProvider::new(root, CaseRule::Insensitive))
        }),
    ]
}

fn each(test: impl Fn(&Fixture)) {
    for fixture in fixtures() {
        eprintln!("-- {}", fixture.name);
        test(&fixture);
    }
}

fn at(root: &VfsPath, rel: &str) -> VfsPath {
    root.join(rel).unwrap()
}

/// The `kind` tag of an error, which is what two providers must agree on (locations and operating
/// system messages legitimately differ).
fn kind<T>(result: &Result<T, VfsError>) -> String {
    match result {
        Ok(_) => "ok".to_owned(),
        Err(e) => serde_json::to_value(e).unwrap()["kind"]
            .as_str()
            .unwrap()
            .to_owned(),
    }
}

fn write_file(p: &dyn Provider, path: &VfsPath, bytes: &[u8]) {
    let mut out = p.create_write(path, WriteOptions::exclusive()).unwrap();
    out.write_all(bytes).unwrap();
    out.finish(false).unwrap();
}

fn read_file(p: &dyn Provider, path: &VfsPath) -> Result<Vec<u8>, VfsError> {
    let mut bytes = Vec::new();
    p.open_read(path)?
        .read_to_end(&mut bytes)
        .map_err(|e| from_io(&e, &path.to_location()))?;
    Ok(bytes)
}

fn names(p: &dyn Provider, folder: &VfsPath) -> BTreeSet<OsString> {
    p.list(folder, &CancelToken::new(), 100, &mut |_| {})
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect()
}

fn set(items: &[&str]) -> BTreeSet<OsString> {
    items.iter().map(OsString::from).collect()
}

/// A text dump of a whole tree, links not followed, equal for equal trees.
fn snapshot(p: &dyn Provider, folder: &VfsPath, prefix: &str, out: &mut Vec<String>) {
    for entry in p.list(folder, &CancelToken::new(), 0, &mut |_| {}).unwrap() {
        let path = folder.join(&entry.name).unwrap();
        let rel = format!("{prefix}/{}", entry.name.to_string_lossy());
        match entry.kind {
            EntryKind::Directory => {
                out.push(format!("{rel}/"));
                snapshot(p, &path, &rel, out);
            }
            EntryKind::Symlink => out.push(format!(
                "{rel} -> {}",
                p.read_link(&path).unwrap().to_string_lossy()
            )),
            _ => out.push(format!("{rel} {:?}", read_file(p, &path).unwrap())),
        }
    }
    out.sort();
}

fn tree(p: &dyn Provider, root: &VfsPath) -> Vec<String> {
    let mut out = Vec::new();
    snapshot(p, root, "", &mut out);
    out
}

#[test]
fn creating_folders_and_files() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        p.create_dir(&at(r, "d")).unwrap();
        p.create_file(&at(r, "d/f.txt")).unwrap();
        assert_eq!(names(p, &at(r, "d")), set(&["f.txt"]));
        assert_eq!(read_file(p, &at(r, "d/f.txt")).unwrap(), b"");

        // A taken name is `AlreadyExists` whatever holds it, and nothing changes.
        assert_eq!(kind(&p.create_dir(&at(r, "d"))), "alreadyExists");
        assert_eq!(kind(&p.create_dir(&at(r, "d/f.txt"))), "alreadyExists");
        assert_eq!(kind(&p.create_file(&at(r, "d"))), "alreadyExists");
        assert_eq!(kind(&p.create_file(&at(r, "d/f.txt"))), "alreadyExists");
        // A missing parent is `NotFound`; a file as a parent is `NotADirectory`.
        assert_eq!(kind(&p.create_dir(&at(r, "no/such"))), "notFound");
        assert_eq!(kind(&p.create_file(&at(r, "no/such"))), "notFound");
        // Windows reports a path through a file as not found.
        #[cfg(unix)]
        assert_eq!(kind(&p.create_file(&at(r, "d/f.txt/x"))), "notADirectory");
    });
}

#[test]
fn bad_names_are_refused_before_anything_is_created() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        let rule = p.capabilities().case_rule;
        let too_long = at(r, &"x".repeat(256));
        assert_eq!(kind(&p.create_file(&too_long)), "invalidName");
        assert_eq!(kind(&p.create_dir(&too_long)), "invalidName");
        assert_eq!(
            kind(&p.create_write(&too_long, WriteOptions::exclusive())),
            "invalidName"
        );
        p.create_file(&at(r, "ok")).unwrap();
        assert_eq!(
            kind(&p.rename(&at(r, "ok"), &too_long, false)),
            "invalidName"
        );
        assert_eq!(kind(&p.symlink(&too_long, OsStr::new("ok"))), "invalidName");
        // `FilePath::join` refuses such names on Windows itself, so they are tried on Unix only.
        #[cfg(unix)]
        for reserved in ["NUL", "con.txt", "trail.", "has:colon"] {
            let result = p.create_file(&at(r, reserved));
            if rule == CaseRule::Insensitive {
                assert_eq!(kind(&result), "invalidName", "{reserved}");
            } else {
                result.unwrap();
            }
        }
        assert!(child_path(r, OsStr::new("a/b"), rule).is_err());
        assert!(child_path(r, OsStr::new(".."), rule).is_err());
        assert!(names(p, r).contains(OsStr::new("ok")));
    });
}

#[test]
fn streams_write_and_read_back() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        let path = at(r, "data.bin");
        let payload: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
        let mut out = p.create_write(&path, WriteOptions::exclusive()).unwrap();
        for chunk in payload.chunks(8192) {
            out.write_all(chunk).unwrap();
        }
        out.finish(true).unwrap();
        assert_eq!(read_file(p, &path).unwrap(), payload);

        // Exclusive refuses an existing file and leaves it alone.
        assert_eq!(
            kind(&p.create_write(&path, WriteOptions::exclusive())),
            "alreadyExists"
        );
        assert_eq!(read_file(p, &path).unwrap(), payload);

        // Without it the file is truncated.
        let mut again = p.create_write(&path, WriteOptions::truncate()).unwrap();
        again.write_all(b"short").unwrap();
        again.finish(false).unwrap();
        assert_eq!(read_file(p, &path).unwrap(), b"short");

        // A folder cannot be read or written as a file (Windows says "access denied" instead); a
        // missing file is not found.
        p.create_dir(&at(r, "d")).unwrap();
        #[cfg(unix)]
        {
            assert_eq!(kind(&p.open_read(&at(r, "d"))), "isADirectory");
            assert_eq!(
                kind(&p.create_write(&at(r, "d"), WriteOptions::truncate())),
                "isADirectory"
            );
        }
        assert_eq!(kind(&p.open_read(&at(r, "missing"))), "notFound");
        assert_eq!(
            kind(&p.create_write(&at(r, "no/such"), WriteOptions::exclusive())),
            "notFound"
        );
    });
}

#[cfg(unix)]
#[test]
fn a_created_file_takes_the_requested_mode() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        let path = at(r, "private");
        let options = WriteOptions {
            exclusive: true,
            mode: Some(0o600),
        };
        p.create_write(&path, options)
            .unwrap()
            .finish(false)
            .unwrap();
        assert_eq!(p.permissions(&path).unwrap().mode, Some(0o600));
    });
}

#[test]
fn exclusive_creates_race_to_exactly_one_winner() {
    each(|f| {
        let target = at(&f.root, "contested");
        let by_stream = at(&f.root, "contested-stream");
        let outcomes: Vec<(String, String)> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..12)
                .map(|n| {
                    let (p, target, by_stream) = (f.provider.clone(), &target, &by_stream);
                    scope.spawn(move || {
                        let file = p.create_file(target);
                        let stream = p.create_write(by_stream, WriteOptions::exclusive());
                        if let Ok(mut out) = stream {
                            out.write_all(&[n as u8]).unwrap();
                            out.finish(false).unwrap();
                            return (kind(&file), "ok".to_owned());
                        }
                        (kind(&file), kind(&stream))
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for pick in [
            |o: &(String, String)| o.0.clone(),
            |o: &(String, String)| o.1.clone(),
        ] {
            let wins = outcomes.iter().filter(|o| pick(o) == "ok").count();
            let losses = outcomes
                .iter()
                .filter(|o| pick(o) == "alreadyExists")
                .count();
            assert_eq!((wins, losses), (1, 11), "{outcomes:?}");
        }
        // The winner's single byte is the whole content: nobody else wrote.
        assert_eq!(read_file(&*f.provider, &by_stream).unwrap().len(), 1);
    });
}

#[test]
fn renaming_files_and_folders() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "a"), b"alpha");
        p.create_dir(&at(r, "dir")).unwrap();
        write_file(p, &at(r, "dir/inner"), b"inner");

        p.rename(&at(r, "a"), &at(r, "b"), false).unwrap();
        assert_eq!(read_file(p, &at(r, "b")).unwrap(), b"alpha");
        assert_eq!(kind(&p.stat(&at(r, "a"))), "notFound");

        // A folder moves with everything in it.
        p.create_dir(&at(r, "dest")).unwrap();
        p.rename(&at(r, "dir"), &at(r, "dest/moved"), false)
            .unwrap();
        assert_eq!(read_file(p, &at(r, "dest/moved/inner")).unwrap(), b"inner");

        // Without `overwrite`, an existing target (a file or a folder) is never touched.
        write_file(p, &at(r, "c"), b"gamma");
        assert_eq!(
            kind(&p.rename(&at(r, "b"), &at(r, "c"), false)),
            "alreadyExists"
        );
        assert_eq!(
            kind(&p.rename(&at(r, "b"), &at(r, "dest"), false)),
            "alreadyExists"
        );
        assert_eq!(read_file(p, &at(r, "c")).unwrap(), b"gamma");
        assert_eq!(read_file(p, &at(r, "b")).unwrap(), b"alpha");
        // Not even onto itself.
        assert_eq!(
            kind(&p.rename(&at(r, "b"), &at(r, "b"), false)),
            "alreadyExists"
        );

        // A missing source or target folder.
        assert_eq!(
            kind(&p.rename(&at(r, "nope"), &at(r, "x"), false)),
            "notFound"
        );
        assert_eq!(
            kind(&p.rename(&at(r, "b"), &at(r, "no/x"), false)),
            "notFound"
        );
        #[cfg(unix)]
        assert_eq!(
            kind(&p.rename(&at(r, "b"), &at(r, "c/x"), false)),
            "notADirectory"
        );
    });
}

#[cfg(unix)]
#[test]
fn renaming_over_an_existing_target_with_overwrite() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "a"), b"new");
        write_file(p, &at(r, "b"), b"old");
        p.rename(&at(r, "a"), &at(r, "b"), true).unwrap();
        assert_eq!(read_file(p, &at(r, "b")).unwrap(), b"new");
        assert_eq!(kind(&p.stat(&at(r, "a"))), "notFound");

        // Kinds must match, as for `rename(2)`.
        p.create_dir(&at(r, "d1")).unwrap();
        p.create_dir(&at(r, "full")).unwrap();
        write_file(p, &at(r, "full/x"), b"x");
        assert_eq!(
            kind(&p.rename(&at(r, "b"), &at(r, "d1"), true)),
            "isADirectory"
        );
        assert_eq!(
            kind(&p.rename(&at(r, "d1"), &at(r, "b"), true)),
            "notADirectory"
        );
        assert_eq!(
            kind(&p.rename(&at(r, "d1"), &at(r, "full"), true)),
            "notEmpty"
        );
        // An empty folder is replaced.
        p.create_dir(&at(r, "empty")).unwrap();
        write_file(p, &at(r, "d1/y"), b"y");
        p.rename(&at(r, "d1"), &at(r, "empty"), true).unwrap();
        assert_eq!(read_file(p, &at(r, "empty/y")).unwrap(), b"y");
        // A folder cannot move into itself.
        assert_eq!(
            kind(&p.rename(&at(r, "full"), &at(r, "full/in"), false)),
            "io"
        );
        // Onto itself with `overwrite` is a no-op.
        p.rename(&at(r, "b"), &at(r, "b"), true).unwrap();
        assert_eq!(read_file(p, &at(r, "b")).unwrap(), b"new");
    });
}

#[test]
fn removing_files_and_folders() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "f"), b"x");
        p.create_dir(&at(r, "d")).unwrap();
        write_file(p, &at(r, "d/inner"), b"x");

        assert_eq!(kind(&p.remove_file(&at(r, "d"))), "isADirectory");
        assert_eq!(kind(&p.remove_dir(&at(r, "f"))), "notADirectory");
        assert_eq!(kind(&p.remove_dir(&at(r, "d"))), "notEmpty");
        assert_eq!(kind(&p.remove_file(&at(r, "nope"))), "notFound");
        assert_eq!(kind(&p.remove_dir(&at(r, "nope"))), "notFound");
        assert_eq!(read_file(p, &at(r, "d/inner")).unwrap(), b"x");

        p.remove_file(&at(r, "f")).unwrap();
        p.remove_file(&at(r, "d/inner")).unwrap();
        p.remove_dir(&at(r, "d")).unwrap();
        assert!(names(p, r).is_empty());
    });
}

#[cfg(unix)]
#[test]
fn symlinks_are_acted_on_never_followed() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        p.create_dir(&at(r, "real")).unwrap();
        write_file(p, &at(r, "real/file"), b"data");
        p.symlink(&at(r, "to-dir"), OsStr::new("real")).unwrap();
        p.symlink(&at(r, "to-file"), OsStr::new("real/file"))
            .unwrap();
        p.symlink(&at(r, "dangling"), OsStr::new("nowhere"))
            .unwrap();
        assert_eq!(
            kind(&p.symlink(&at(r, "to-dir"), OsStr::new("x"))),
            "alreadyExists"
        );

        assert_eq!(p.read_link(&at(r, "to-dir")).unwrap(), "real");
        assert_eq!(p.read_link(&at(r, "dangling")).unwrap(), "nowhere");
        assert_eq!(kind(&p.read_link(&at(r, "real"))), "io");
        let entry = p.stat(&at(r, "to-dir")).unwrap();
        assert_eq!(
            (entry.kind, entry.link_target),
            (EntryKind::Symlink, Some(EntryKind::Directory))
        );
        assert_eq!(p.stat(&at(r, "dangling")).unwrap().link_target, None);
        assert_eq!(read_file(p, &at(r, "to-file")).unwrap(), b"data");

        // Permissions belong to the target, so a link refuses them.
        let readonly = Permissions {
            mode: Some(0o400),
            readonly: true,
        };
        assert_eq!(
            kind(&p.set_permissions(&at(r, "to-file"), readonly)),
            "unsupported"
        );
        assert_eq!(
            p.permissions(&at(r, "real/file")).unwrap().mode,
            Some(0o644)
        );

        // `remove_dir` on a link to a folder refuses; `remove_file` removes the link only.
        assert_eq!(kind(&p.remove_dir(&at(r, "to-dir"))), "notADirectory");
        p.remove_file(&at(r, "to-dir")).unwrap();
        assert_eq!(read_file(p, &at(r, "real/file")).unwrap(), b"data");

        // A rename moves the link itself, over a link or file, and never the target.
        p.rename(&at(r, "to-file"), &at(r, "moved"), false).unwrap();
        assert_eq!(p.read_link(&at(r, "moved")).unwrap(), "real/file");
        assert_eq!(kind(&p.stat(&at(r, "to-file"))), "notFound");
        p.rename(&at(r, "dangling"), &at(r, "moved"), true).unwrap();
        assert_eq!(p.read_link(&at(r, "moved")).unwrap(), "nowhere");
        assert_eq!(read_file(p, &at(r, "real/file")).unwrap(), b"data");
        // Removing a file link leaves its target.
        p.remove_file(&at(r, "moved")).unwrap();
        assert_eq!(names(p, r), set(&["real"]));
    });
}

#[cfg(unix)]
#[test]
fn a_non_exclusive_write_follows_a_symlink_and_never_replaces_it() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "real"), b"old");
        p.symlink(&at(r, "link"), OsStr::new("real")).unwrap();
        p.symlink(&at(r, "dangling"), OsStr::new("made")).unwrap();
        for (link, target) in [("link", "real"), ("dangling", "made")] {
            let mut out = p
                .create_write(&at(r, link), WriteOptions::truncate())
                .unwrap();
            out.write_all(b"new").unwrap();
            out.finish(false).unwrap();
            assert_eq!(p.read_link(&at(r, link)).unwrap(), target, "{link}");
            assert_eq!(read_file(p, &at(r, target)).unwrap(), b"new", "{link}");
        }
        // Exclusive creation still refuses the name, even for a dangling link.
        assert_eq!(
            kind(&p.create_write(&at(r, "dangling"), WriteOptions::exclusive())),
            "alreadyExists"
        );
    });
}

/// The display form of the location an error names.
fn named(result: Result<(), VfsError>) -> (String, String) {
    let error = result.unwrap_err();
    let json = serde_json::to_value(&error).unwrap();
    (
        json["kind"].as_str().unwrap().to_owned(),
        json["location"]["display"].as_str().unwrap().to_owned(),
    )
}

#[cfg(unix)]
#[test]
fn a_rename_error_names_the_side_that_is_wrong() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "a"), b"a");
        // The source is there and the destination's parent is not: the destination is missing.
        let (kind, location) = named(p.rename(&at(r, "a"), &at(r, "nowhere/b"), false));
        assert_eq!(kind, "notFound", "{}", f.name);
        assert!(location.ends_with("nowhere/b"), "{} {location}", f.name);
        // A missing source is the source.
        let (kind, location) = named(p.rename(&at(r, "gone"), &at(r, "b"), false));
        assert_eq!(kind, "notFound", "{}", f.name);
        assert!(location.ends_with("gone"), "{} {location}", f.name);
        // A folder that would be replaced and is not empty is the destination.
        p.create_dir(&at(r, "src")).unwrap();
        p.create_dir(&at(r, "full")).unwrap();
        write_file(p, &at(r, "full/x"), b"x");
        let (kind, location) = named(p.rename(&at(r, "src"), &at(r, "full"), true));
        assert_eq!(kind, "notEmpty", "{}", f.name);
        assert!(location.ends_with("full"), "{} {location}", f.name);
    });
}

#[test]
fn a_listing_fails_when_one_entry_cannot_be_read() {
    let (m, r, _dir) = memory(CaseRule::Sensitive);
    for name in ["a", "b", "c"] {
        m.create_file(&at(&r, name)).unwrap();
    }
    let cancel = CancelToken::new();
    m.fail_nth(
        MemOp::ListEntry,
        2,
        VfsError::Io {
            message: "input/output error".to_owned(),
            location: Some(r.to_location()),
        },
    );
    let failed = m.list(&r, &cancel, 0, &mut |_| {});
    assert_eq!(kind(&failed), "io");
    // Nothing is left injected: the next listing is whole.
    assert_eq!(m.list(&r, &cancel, 0, &mut |_| {}).unwrap().len(), 3);
}

#[test]
fn permissions_round_trip() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "f"), b"x");
        p.create_dir(&at(r, "d")).unwrap();
        #[cfg(unix)]
        {
            for (path, mode) in [("f", 0o640), ("f", 0o4755), ("d", 0o700)] {
                let perms = Permissions {
                    mode: Some(mode),
                    readonly: false,
                };
                p.set_permissions(&at(r, path), perms).unwrap();
                assert_eq!(p.permissions(&at(r, path)).unwrap().mode, Some(mode));
            }
            p.set_permissions(
                &at(r, "f"),
                Permissions {
                    mode: Some(0o644),
                    readonly: false,
                },
            )
            .unwrap();
        }
        let readonly = Permissions {
            mode: None,
            readonly: true,
        };
        p.set_permissions(&at(r, "f"), readonly).unwrap();
        assert!(p.permissions(&at(r, "f")).unwrap().readonly);
        #[cfg(unix)]
        assert_eq!(p.permissions(&at(r, "f")).unwrap().mode, Some(0o444));
        let writable = Permissions {
            mode: None,
            readonly: false,
        };
        p.set_permissions(&at(r, "f"), writable).unwrap();
        assert!(!p.permissions(&at(r, "f")).unwrap().readonly);
        assert_eq!(kind(&p.permissions(&at(r, "nope"))), "notFound");
        assert_eq!(
            kind(&p.set_permissions(&at(r, "nope"), readonly)),
            "notFound"
        );
    });
}

fn ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

#[test]
fn times_round_trip() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "f"), b"x");
        p.create_dir(&at(r, "d")).unwrap();
        let modified = UNIX_EPOCH + Duration::from_millis(1_234_567_890_123);
        let accessed = UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        for path in ["f", "d"] {
            let times = FileTimes {
                accessed: Some(accessed),
                modified: Some(modified),
            };
            p.set_times(&at(r, path), times).unwrap();
            assert_eq!(
                p.stat(&at(r, path)).unwrap().modified_ms,
                Some(ms(modified)),
                "{path}"
            );
        }
        // A time left `None` is left alone.
        let later = modified + Duration::from_secs(3600);
        p.set_times(
            &at(r, "f"),
            FileTimes {
                accessed: None,
                modified: Some(later),
            },
        )
        .unwrap();
        assert_eq!(p.stat(&at(r, "f")).unwrap().modified_ms, Some(ms(later)));
        p.set_times(&at(r, "f"), FileTimes::default()).unwrap();
        assert_eq!(p.stat(&at(r, "f")).unwrap().modified_ms, Some(ms(later)));
        assert_eq!(
            kind(&p.set_times(&at(r, "nope"), FileTimes::default())),
            "notFound"
        );
    });
}

#[cfg(unix)]
#[test]
fn a_symlinks_own_times_are_set_not_its_targets() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "target"), b"x");
        p.symlink(&at(r, "link"), OsStr::new("target")).unwrap();
        let before = p.stat(&at(r, "target")).unwrap().modified_ms;
        let when = UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        p.set_times(
            &at(r, "link"),
            FileTimes {
                accessed: Some(when),
                modified: Some(when),
            },
        )
        .unwrap();
        assert_eq!(p.stat(&at(r, "target")).unwrap().modified_ms, before);
    });
}

#[test]
fn volume_ids_agree_for_paths_in_one_folder() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        p.create_dir(&at(r, "a")).unwrap();
        p.create_dir(&at(r, "b")).unwrap();
        write_file(p, &at(r, "a/f"), b"x");
        let id = p
            .volume_id(&at(r, "a/f"))
            .expect("an existing file has a volume");
        assert_eq!(p.volume_id(&at(r, "a")), Some(id));
        assert_eq!(p.volume_id(&at(r, "b")), Some(id));
        assert_eq!(p.volume_id(r), Some(id));
        assert_eq!(p.volume_id(&at(r, "missing")), None);
    });
}

#[cfg(unix)]
#[test]
fn a_real_rename_across_volumes_reports_crosses_devices() {
    let local = LocalProvider::new();
    let here = tempfile::tempdir().unwrap();
    let root = VfsPath::File(FilePath::from_path(here.path()).unwrap());
    // Find another writable file system: `/dev/shm` is usually a different tmpfs from `/tmp`.
    let Ok(other) = tempfile::tempdir_in("/dev/shm") else {
        eprintln!("no /dev/shm; skipping");
        return;
    };
    let other_root = VfsPath::File(FilePath::from_path(other.path()).unwrap());
    if local.volume_id(&root) == local.volume_id(&other_root) {
        eprintln!("/tmp and /dev/shm share a volume; skipping");
        return;
    }
    write_file(&local, &at(&root, "f"), b"x");
    let result = local.rename(&at(&root, "f"), &at(&other_root, "f"), false);
    assert!(
        matches!(result, Err(VfsError::CrossesDevices { .. })),
        "{result:?}"
    );
    // Nothing moved.
    assert_eq!(read_file(&local, &at(&root, "f")).unwrap(), b"x");
    assert_eq!(kind(&local.stat(&at(&other_root, "f"))), "notFound");
}

#[cfg(unix)]
#[test]
fn free_space_is_reported_for_the_local_volume() {
    let f = &fixtures()[0];
    {
        let space = f
            .provider
            .free_space(&f.root)
            .expect("a temp folder reports space");
        assert!(space.total_bytes > 0 && space.free_bytes <= space.total_bytes);
    }
    assert_eq!(f.provider.free_space(&at(&f.root, "missing")), None);
}

#[cfg(unix)]
#[test]
fn names_that_are_not_utf8_survive_every_primitive() {
    use std::os::unix::ffi::OsStrExt;
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        let raw = |bytes: &[u8]| OsStr::from_bytes(bytes).to_owned();
        let (a, b) = (raw(b"caf\xe9"), raw(b"\xff\xfe-dir"));
        let (file, dir) = (r.join(&a).unwrap(), r.join(&b).unwrap());
        p.create_dir(&dir).unwrap();
        write_file(p, &file, b"latin-1");
        assert_eq!(names(p, r), BTreeSet::from([a.clone(), b.clone()]));
        let moved = dir.join(raw(b"\xc3\x28")).unwrap();
        p.rename(&file, &moved, false).unwrap();
        assert_eq!(read_file(p, &moved).unwrap(), b"latin-1");
        p.symlink(&r.join(raw(b"l\x80")).unwrap(), &raw(b"\xc3\x28"))
            .unwrap();
        assert_eq!(
            p.read_link(&r.join(raw(b"l\x80")).unwrap()).unwrap(),
            raw(b"\xc3\x28")
        );
        p.remove_file(&r.join(raw(b"l\x80")).unwrap()).unwrap();
        p.remove_file(&moved).unwrap();
        p.remove_dir(&dir).unwrap();
        assert!(names(p, r).is_empty());
    });
}

#[test]
fn case_collisions_follow_the_case_rule() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        let insensitive = p.capabilities().case_rule == CaseRule::Insensitive;
        write_file(p, &at(r, "File.txt"), b"one");
        let second = p.create_file(&at(r, "FILE.TXT"));
        let dir = p.create_dir(&at(r, "file.TXT"));
        if insensitive {
            assert_eq!(kind(&second), "alreadyExists");
            assert_eq!(kind(&dir), "alreadyExists");
            assert_eq!(names(p, r), set(&["File.txt"]));
            // Lookups ignore case, and a case-only rename changes the spelling.
            assert_eq!(read_file(p, &at(r, "file.txt")).unwrap(), b"one");
            p.rename(&at(r, "file.txt"), &at(r, "FILE.TXT"), false)
                .unwrap();
            assert_eq!(names(p, r), set(&["FILE.TXT"]));
            // Renaming another file onto a differently-cased name collides.
            write_file(p, &at(r, "other"), b"two");
            assert_eq!(
                kind(&p.rename(&at(r, "other"), &at(r, "file.txt"), false)),
                "alreadyExists"
            );
        } else {
            second.unwrap();
            dir.unwrap();
            assert_eq!(names(p, r), set(&["File.txt", "FILE.TXT", "file.TXT"]));
        }
    });
}

#[test]
fn fast_copy_copies_the_bytes_or_declines() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        let payload: Vec<u8> = (0..300_000u32).map(|n| (n % 253) as u8).collect();
        write_file(p, &at(r, "src"), &payload);
        let mut seen = Vec::new();
        let outcome = p.copy_file_within(
            &at(r, "src"),
            &at(r, "dst"),
            &mut |n| seen.push(n),
            &CancelToken::new(),
        );
        match outcome {
            // Declined: nothing was touched.
            None => assert_eq!(kind(&p.stat(&at(r, "dst"))), "notFound"),
            Some(result) => {
                assert_eq!(result.unwrap(), payload.len() as u64);
                assert_eq!(read_file(p, &at(r, "dst")).unwrap(), payload);
                assert_eq!(seen.last(), Some(&(payload.len() as u64)));
                assert!(seen.windows(2).all(|w| w[0] <= w[1]), "progress only grows");
                // Never over an existing file.
                let again = p
                    .copy_file_within(
                        &at(r, "src"),
                        &at(r, "dst"),
                        &mut |_| {},
                        &CancelToken::new(),
                    )
                    .expect("a provider that copied once handles the second");
                assert_eq!(kind(&again), "alreadyExists");
                assert_eq!(read_file(p, &at(r, "dst")).unwrap(), payload);
            }
        }
    });
}

#[test]
fn a_cancelled_fast_copy_leaves_no_destination() {
    each(|f| {
        let (p, r) = (&*f.provider, &f.root);
        write_file(p, &at(r, "src"), &vec![7u8; 20 * 1024 * 1024]);
        let cancel = CancelToken::new();
        let token = cancel.clone();
        let outcome = p.copy_file_within(
            &at(r, "src"),
            &at(r, "dst"),
            &mut |_| token.cancel(),
            &cancel,
        );
        match outcome.map(|result| kind(&result)).as_deref() {
            // Declined, or finished in one step (a reflink), which is also correct.
            None => assert_eq!(kind(&p.stat(&at(r, "dst"))), "notFound"),
            Some("ok") => assert_eq!(read_file(p, &at(r, "dst")).unwrap().len(), 20 * 1024 * 1024),
            Some("cancelled") => assert_eq!(kind(&p.stat(&at(r, "dst"))), "notFound"),
            Some(other) => panic!("unexpected {other}"),
        }
    });
}

/// A provider that implements only the required methods, like a read-only remote.
struct ReadOnlyProvider;

impl Provider for ReadOnlyProvider {
    fn scheme(&self) -> &'static str {
        "ro"
    }
    fn capabilities(&self) -> waypoint_vfs::Capabilities {
        waypoint_vfs::Capabilities {
            watch: false,
            case_rule: CaseRule::Sensitive,
        }
    }
    fn stat(&self, _: &VfsPath) -> Result<waypoint_vfs::ScannedEntry, VfsError> {
        unreachable!()
    }
    fn list(
        &self,
        _: &VfsPath,
        _: &CancelToken,
        _: usize,
        _: &mut dyn FnMut(u32),
    ) -> Result<Vec<waypoint_vfs::ScannedEntry>, VfsError> {
        unreachable!()
    }
    fn resolve_link(
        &self,
        _: &VfsPath,
        _: &waypoint_vfs::ScannedEntry,
    ) -> Result<waypoint_vfs::ScannedEntry, VfsError> {
        unreachable!()
    }
}

#[test]
fn a_read_only_provider_answers_every_write_with_unsupported() {
    let p = ReadOnlyProvider;
    let path = VfsPath::File(FilePath::from_path(std::env::temp_dir()).unwrap());
    assert_eq!(kind(&p.create_dir(&path)), "unsupported");
    assert_eq!(kind(&p.create_file(&path)), "unsupported");
    assert_eq!(kind(&p.rename(&path, &path, false)), "unsupported");
    assert_eq!(kind(&p.remove_file(&path)), "unsupported");
    assert_eq!(kind(&p.remove_dir(&path)), "unsupported");
    assert_eq!(kind(&p.open_read(&path)), "unsupported");
    assert_eq!(
        kind(&p.create_write(&path, WriteOptions::exclusive())),
        "unsupported"
    );
    assert_eq!(
        kind(&p.set_times(&path, FileTimes::default())),
        "unsupported"
    );
    assert_eq!(kind(&p.permissions(&path)), "unsupported");
    let perms = Permissions {
        mode: None,
        readonly: false,
    };
    assert_eq!(kind(&p.set_permissions(&path, perms)), "unsupported");
    assert_eq!(kind(&p.symlink(&path, OsStr::new("x"))), "unsupported");
    assert_eq!(kind(&p.read_link(&path)), "unsupported");
    assert_eq!(p.volume_id(&path), None);
    assert_eq!(p.free_space(&path), None);
    assert!(p
        .copy_file_within(&path, &path, &mut |_| {}, &CancelToken::new())
        .is_none());
}

// A scripted and seeded comparison: the same random sequence of creates, renames and removes runs on
// the real file system and in memory, and every step must give the same kind of result and leave the
// same tree.

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const TOP: [&str; 4] = ["a", "b", "c", "d"];
const NESTED: [&str; 3] = ["x", "y", "z"];

fn random_path(rng: &mut Rng) -> (String, bool) {
    let top = TOP[rng.below(TOP.len())];
    if rng.below(2) == 0 {
        (top.to_owned(), false)
    } else {
        (format!("{top}/{}", NESTED[rng.below(NESTED.len())]), true)
    }
}

fn step(p: &dyn Provider, root: &VfsPath, rng: &mut Rng, links: bool) -> String {
    let (path, nested) = random_path(rng);
    let target = at(root, &path);
    match rng.below(8) {
        0 => format!("create_dir {path} {}", kind(&p.create_dir(&target))),
        1 => format!("create_file {path} {}", kind(&p.create_file(&target))),
        2 => {
            let result = p
                .create_write(&target, WriteOptions::exclusive())
                .and_then(|mut out| {
                    out.write_all(path.as_bytes()).unwrap();
                    out.finish(false)
                });
            format!("write {path} {}", kind(&result))
        }
        3 | 4 => {
            let (mut to, mut to_nested) = random_path(rng);
            // With links about, keep nested entries nested so none ends up with children beneath it.
            while links && to_nested != nested {
                (to, to_nested) = random_path(rng);
            }
            let overwrite = rng.below(2) == 0;
            let result = p.rename(&target, &at(root, &to), overwrite);
            format!("rename {path} {to} {overwrite} {}", kind(&result))
        }
        5 => format!("remove_file {path} {}", kind(&p.remove_file(&target))),
        6 => format!("remove_dir {path} {}", kind(&p.remove_dir(&target))),
        _ => {
            // Links only where nothing is created beneath them (the memory provider does not
            // resolve paths through links).
            if nested && links {
                let link_to = if rng.below(2) == 0 { "../a" } else { "gone" };
                format!(
                    "symlink {path} {}",
                    kind(&p.symlink(&target, OsStr::new(link_to)))
                )
            } else {
                format!("stat {path} {}", kind(&p.stat(&target)))
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn seeded_scripts_agree_between_the_real_and_the_in_memory_provider() {
    for seed in 1..=120u64 {
        let links = seed % 2 == 0;
        let fixtures = [fixtures().remove(0), fixtures().remove(1)];
        let mut logs = [Vec::new(), Vec::new()];
        for (fixture, log) in fixtures.iter().zip(&mut logs) {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            for _ in 0..80 {
                log.push(step(&*fixture.provider, &fixture.root, &mut rng, links));
            }
        }
        if let Some(at) = (0..logs[0].len()).find(|&i| logs[0][i] != logs[1][i]) {
            panic!(
                "seed {seed}: step {at} differs (real vs memory): {:?} vs {:?}\nafter {:?}",
                logs[0][at],
                logs[1][at],
                &logs[0][..at]
            );
        }
        let (local, memory) = (&fixtures[0], &fixtures[1]);
        assert_eq!(
            tree(&*local.provider, &local.root),
            tree(&*memory.provider, &memory.root),
            "seed {seed}: trees differ"
        );
    }
}

// Failure injection and volumes on the in-memory provider, which later slices build on.

fn memory(rule: CaseRule) -> (MemoryProvider, VfsPath, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    (
        MemoryProvider::new(root.clone(), rule),
        VfsPath::File(root),
        dir,
    )
}

fn full(root: &VfsPath) -> VfsError {
    VfsError::StorageFull {
        location: root.to_location(),
    }
}

#[test]
fn a_failure_can_be_injected_into_the_next_call_or_the_nth() {
    let (m, r, _dir) = memory(CaseRule::Sensitive);
    m.fail_next(MemOp::CreateFile, full(&r));
    assert_eq!(kind(&m.create_file(&at(&r, "a"))), "storageFull");
    m.create_file(&at(&r, "a")).unwrap();
    assert_eq!(m.calls(MemOp::CreateFile), 2);

    m.fail_nth(MemOp::CreateDir, 3, full(&r));
    m.create_dir(&at(&r, "d1")).unwrap();
    m.create_dir(&at(&r, "d2")).unwrap();
    assert_eq!(kind(&m.create_dir(&at(&r, "d3"))), "storageFull");
    m.create_dir(&at(&r, "d3")).unwrap();

    m.fail_always(
        MemOp::RemoveFile,
        VfsError::PermissionDenied {
            location: r.to_location(),
        },
    );
    for _ in 0..3 {
        assert_eq!(kind(&m.remove_file(&at(&r, "a"))), "permissionDenied");
    }
    m.clear_failures();
    m.remove_file(&at(&r, "a")).unwrap();
}

#[test]
fn a_failure_injected_into_a_stream_comes_back_typed() {
    let (m, r, _dir) = memory(CaseRule::Sensitive);
    m.put_file(&at(&r, "src"), &[1u8; 100]);
    m.fail_nth(MemOp::Write, 2, full(&r));
    let mut out = m
        .create_write(&at(&r, "dst"), WriteOptions::exclusive())
        .unwrap();
    out.write_all(b"first").unwrap();
    let error = out.write_all(b"second").unwrap_err();
    assert_eq!(
        kind::<()>(&Err(from_io(&error, &r.to_location()))),
        "storageFull"
    );
    // The first write is there, as on a disk that filled up.
    assert_eq!(m.file_content(&at(&r, "dst")).unwrap(), b"first");

    m.fail_next(MemOp::Read, VfsError::Cancelled);
    let mut input = m.open_read(&at(&r, "src")).unwrap();
    let error = input.read(&mut [0u8; 8]).unwrap_err();
    assert_eq!(from_io(&error, &r.to_location()), VfsError::Cancelled);
    assert_eq!(input.read(&mut [0u8; 8]).unwrap(), 8);

    m.fail_next(MemOp::Finish, full(&r));
    let out = m
        .create_write(&at(&r, "dst2"), WriteOptions::exclusive())
        .unwrap();
    assert_eq!(kind(&out.finish(true)), "storageFull");
}

#[test]
fn volumes_make_a_rename_cross_devices_and_change_nothing() {
    let (m, r, _dir) = memory(CaseRule::Sensitive);
    m.put_file(&at(&r, "here/f"), b"x");
    m.put_dir(&at(&r, "mnt/usb"));
    m.set_volume(&at(&r, "mnt/usb"), VolumeId(7));
    assert_eq!(m.volume_id(&at(&r, "here/f")), Some(VolumeId(1)));
    assert_eq!(m.volume_id(&at(&r, "mnt/usb")), Some(VolumeId(7)));
    m.put_file(&at(&r, "mnt/usb/deep/g"), b"y");
    assert_eq!(m.volume_id(&at(&r, "mnt/usb/deep/g")), Some(VolumeId(7)));
    assert_eq!(m.volume_id(&at(&r, "mnt")), Some(VolumeId(1)));

    let result = m.rename(&at(&r, "here/f"), &at(&r, "mnt/usb/f"), false);
    assert!(
        matches!(result, Err(VfsError::CrossesDevices { .. })),
        "{result:?}"
    );
    assert_eq!(m.file_content(&at(&r, "here/f")).unwrap(), b"x");
    // Within a volume it still works.
    m.rename(&at(&r, "mnt/usb/deep/g"), &at(&r, "mnt/usb/g"), false)
        .unwrap();
    assert_eq!(m.free_space(&r), None);
    let space = VolumeSpace {
        free_bytes: 5,
        total_bytes: 10,
    };
    m.set_space(VolumeId(7), space);
    assert_eq!(m.free_space(&at(&r, "mnt/usb/g")), Some(space));
}

#[test]
fn memory_fast_copy_is_off_until_enabled_and_stays_on_one_volume() {
    let (m, r, _dir) = memory(CaseRule::Sensitive);
    m.put_file(&at(&r, "src"), b"bytes");
    m.put_dir(&at(&r, "other"));
    m.set_volume(&at(&r, "other"), VolumeId(2));
    let copy = |dst: &str| {
        m.copy_file_within(
            &at(&r, "src"),
            &at(&r, dst),
            &mut |_| {},
            &CancelToken::new(),
        )
    };
    assert!(copy("dst").is_none());
    m.enable_fast_copy(true);
    assert_eq!(copy("dst").unwrap().unwrap(), 5);
    assert_eq!(m.file_content(&at(&r, "dst")).unwrap(), b"bytes");
    assert!(
        copy("other/dst").is_none(),
        "another volume is for the generic loop"
    );
    m.fail_next(MemOp::CopyFileWithin, full(&r));
    assert_eq!(kind(&copy("dst2").unwrap()), "storageFull");
}

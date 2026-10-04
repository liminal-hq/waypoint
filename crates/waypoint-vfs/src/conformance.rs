// The shared conformance suite every provider runs: the read and write contract, typed errors,
// awkward names, the case rule, ranges, resumed writes and honest capabilities (A85).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A provider crate runs the suite from its tests with the `testing` feature of `waypoint-vfs`:
//!
//! ```ignore
//! let subject = Subject { name: "sftp", provider: &provider, root };
//! waypoint_vfs::conformance::run(&subject);
//! ```
//!
//! `root` must be an empty folder the suite may fill (it leaves what it made there); for a
//! provider that cannot write, the suite checks only what reading and the capabilities promise.
//! Every check panics with the subject's name and what was expected, so a failure reads as the
//! rule the provider broke.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::sync::Arc;

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::VfsError;

use crate::provider::{PermissionModel, Provider, RenameSupport};
use crate::write::{FileTimes, WriteOptions};
use crate::{CancelToken, EntryKind};

/// A provider and an empty folder it serves.
pub struct Subject<'a> {
    /// Named in every failure.
    pub name: &'a str,
    pub provider: &'a dyn Provider,
    pub root: VfsPath,
}

/// The `kind` tag of an error (`notFound`), or `ok`: what two providers must agree on, since
/// locations and messages legitimately differ.
pub fn kind<T>(result: &Result<T, VfsError>) -> String {
    match result {
        Ok(_) => "ok".to_owned(),
        Err(error) => serde_json::to_value(error)
            .ok()
            .and_then(|value| value["kind"].as_str().map(str::to_owned))
            .unwrap_or_default(),
    }
}

/// Names that stress URI encoding and every provider's quoting, and are valid on every platform.
pub const AWKWARD_NAMES: [&str; 7] = [
    "with space",
    "100%",
    "#hash",
    "café",
    "日本語",
    "a.b.c",
    "semi;colon,comma",
];

impl Subject<'_> {
    fn at(&self, rel: &str) -> VfsPath {
        self.root
            .join(rel)
            .unwrap_or_else(|e| panic!("[{}] cannot join {rel:?}: {e}", self.name))
    }

    fn expect<T>(&self, what: &str, result: &Result<T, VfsError>, wanted: &[&str]) {
        let got = kind(result);
        assert!(
            wanted.contains(&got.as_str()),
            "[{}] {what}: expected {wanted:?}, got {got} ({:?})",
            self.name,
            result.as_ref().err()
        );
    }

    fn names(&self, folder: &VfsPath) -> BTreeSet<OsString> {
        self.provider
            .list(folder, &CancelToken::new(), usize::MAX, &mut |_| {})
            .unwrap_or_else(|e| panic!("[{}] list {}: {e:?}", self.name, folder.display()))
            .into_iter()
            .map(|entry| entry.name)
            .collect()
    }

    fn write(&self, path: &VfsPath, bytes: &[u8]) {
        let mut stream = self
            .provider
            .create_write(path, WriteOptions::exclusive())
            .unwrap_or_else(|e| panic!("[{}] create {}: {e:?}", self.name, path.display()));
        stream.write_all(bytes).unwrap();
        stream
            .finish(false)
            .unwrap_or_else(|e| panic!("[{}] finish {}: {e:?}", self.name, path.display()));
    }

    fn read(&self, path: &VfsPath) -> Vec<u8> {
        let mut out = Vec::new();
        self.provider
            .open_read(path)
            .unwrap_or_else(|e| panic!("[{}] read {}: {e:?}", self.name, path.display()))
            .read_to_end(&mut out)
            .unwrap();
        out
    }

    fn writable(&self) -> bool {
        self.provider.capabilities().write && !self.provider.read_only()
    }
}

/// Runs the whole suite.
pub fn run(subject: &Subject) {
    reading(subject);
    capabilities(subject);
    if subject.writable() {
        writing(subject);
        names(subject);
        case_rule(subject);
        ranges_and_resumes(subject);
        listing(subject);
    }
}

/// What every provider promises, writable or not.
pub fn reading(subject: &Subject) {
    let p = subject.provider;
    let root = p
        .stat(&subject.root)
        .unwrap_or_else(|e| panic!("[{}] stat the root: {e:?}", subject.name));
    assert_eq!(
        root.kind,
        EntryKind::Directory,
        "[{}] the root is a folder",
        subject.name
    );
    assert!(
        subject.names(&subject.root).is_empty(),
        "[{}] the root starts empty",
        subject.name
    );
    let missing = subject.at("no such entry");
    subject.expect("stat a missing entry", &p.stat(&missing), &["notFound"]);
    subject.expect(
        "list a missing folder",
        &p.list(&missing, &CancelToken::new(), 0, &mut |_| {}),
        &["notFound"],
    );
    assert_eq!(
        p.scheme(),
        subject.root.scheme(),
        "[{}] the provider serves its root's scheme",
        subject.name
    );
    if let Some(key) = p.connection_key(&subject.root) {
        assert_eq!(
            Some(&key),
            subject.root.connection_key().as_ref(),
            "[{}] the connection key is the path's",
            subject.name
        );
    }
}

/// Every flag tells the truth: what is claimed works, and what is not claimed is `Unsupported`
/// (or `ReadOnly`) rather than half done.
pub fn capabilities(subject: &Subject) {
    let p = subject.provider;
    let caps = p.capabilities();
    let watched = p.watch(&subject.root, Arc::new(|_| {}));
    if caps.watch {
        subject.expect("watch, which is claimed", &watched, &["ok"]);
    } else {
        subject.expect("watch, which is not claimed", &watched, &["unsupported"]);
    }
    drop(watched);
    let probe = subject.at("capability probe");
    if !subject.writable() {
        subject.expect(
            "create a folder where writing is not possible",
            &p.create_dir(&probe),
            &["unsupported", "readOnly", "permissionDenied"],
        );
        return;
    }
    p.create_dir(&probe)
        .unwrap_or_else(|e| panic!("[{}] create a folder: {e:?}", subject.name));
    let file = probe.join("file").unwrap();
    subject.write(&file, b"0123456789");
    if !caps.resume_write {
        subject.expect(
            "resume a write, which is not claimed",
            &p.resume_write(&file, 2).map(|_| ()),
            &["unsupported"],
        );
    }
    let link = probe.join("link").unwrap();
    let linked = p.symlink(&link, OsString::from("file").as_os_str());
    if caps.symlinks {
        // Windows needs a privilege to make links.
        subject.expect(
            "make a link, which is claimed",
            &linked,
            &["ok", "permissionDenied"],
        );
    } else {
        subject.expect(
            "make a link, which is not claimed",
            &linked,
            &["unsupported"],
        );
    }
    let permissions = p.permissions(&file);
    match caps.permissions {
        PermissionModel::None => subject.expect(
            "read permissions, which there are none of",
            &permissions,
            &["unsupported"],
        ),
        PermissionModel::ReadOnlyFlag => subject.expect("read permissions", &permissions, &["ok"]),
        PermissionModel::Unix => {
            let permissions = permissions
                .unwrap_or_else(|e| panic!("[{}] read permissions: {e:?}", subject.name));
            assert!(
                permissions.mode.is_some(),
                "[{}] Unix permissions have mode bits",
                subject.name
            );
        }
    }
    let times = FileTimes {
        accessed: None,
        modified: Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000)),
    };
    let set = p.set_times(&file, times);
    if caps.set_times {
        subject.expect("set a time, which is claimed", &set, &["ok"]);
        let modified = p.stat(&file).unwrap().modified_ms;
        assert_eq!(
            modified,
            Some(1_600_000_000_000),
            "[{}] the time that was set reads back",
            subject.name
        );
    } else {
        subject.expect("set a time, which is not claimed", &set, &["unsupported"]);
    }
    let other = probe.join("other").unwrap();
    subject.write(&other, b"x");
    let renamed = p.rename(&other, &file, false);
    match caps.rename {
        RenameSupport::None => {
            subject.expect("rename, which is not claimed", &renamed, &["unsupported"])
        }
        RenameSupport::NoReplace => {
            subject.expect("rename onto an existing name", &renamed, &["alreadyExists"]);
            assert_eq!(
                subject.read(&file),
                b"0123456789",
                "[{}] nothing was replaced",
                subject.name
            );
        }
        // A replacing rename may or may not refuse; the engine checks first.
        RenameSupport::Replacing => {}
    }
    if let Some(max) = caps.max_name_len {
        let long = probe.join("a".repeat(max as usize + 1)).unwrap();
        subject.expect(
            "create a name longer than the limit",
            &p.create_file(&long),
            &["invalidName"],
        );
    }
}

/// Creating, writing, reading, renaming and removing, with the typed errors of A45.
pub fn writing(subject: &Subject) {
    let p = subject.provider;
    let folder = subject.at("writing");
    p.create_dir(&folder).unwrap();
    subject.expect(
        "create a folder twice",
        &p.create_dir(&folder),
        &["alreadyExists"],
    );
    let file = folder.join("data.bin").unwrap();
    let bytes: Vec<u8> = (0..=255u8).cycle().take(70_000).collect();
    subject.write(&file, &bytes);
    assert_eq!(
        subject.read(&file),
        bytes,
        "[{}] bytes read back",
        subject.name
    );
    assert_eq!(
        p.stat(&file).unwrap().size,
        Some(bytes.len() as u64),
        "[{}] the size is the bytes written",
        subject.name
    );
    subject.expect(
        "create a file exclusively over an existing one",
        &p.create_write(&file, WriteOptions::exclusive()).map(|_| ()),
        &["alreadyExists"],
    );
    subject.expect(
        "create an empty file over an existing one",
        &p.create_file(&file),
        &["alreadyExists"],
    );
    subject.expect(
        "list a file",
        &p.list(&file, &CancelToken::new(), 0, &mut |_| {}),
        &["notADirectory"],
    );
    let moved = folder.join("moved.bin").unwrap();
    if p.capabilities().rename != RenameSupport::None {
        p.rename(&file, &moved, false).unwrap();
        subject.expect("stat the old name", &p.stat(&file), &["notFound"]);
        assert_eq!(
            subject.read(&moved),
            bytes,
            "[{}] a rename keeps the bytes",
            subject.name
        );
    } else {
        subject.write(&moved, &bytes);
        p.remove_file(&file).unwrap();
    }
    subject.expect(
        "remove a folder that holds a file",
        &p.remove_dir(&folder),
        &["notEmpty"],
    );
    subject.expect(
        "remove a folder as a file",
        &p.remove_file(&folder),
        &["isADirectory"],
    );
    p.remove_file(&moved).unwrap();
    subject.expect(
        "remove a removed file",
        &p.remove_file(&moved),
        &["notFound"],
    );
    p.remove_dir(&folder).unwrap();
    subject.expect("stat a removed folder", &p.stat(&folder), &["notFound"]);
}

/// Names with spaces, `%`, `#`, accents and other scripts survive every primitive.
pub fn names(subject: &Subject) {
    let p = subject.provider;
    let folder = subject.at("names");
    p.create_dir(&folder).unwrap();
    for name in AWKWARD_NAMES {
        let file = folder.join(name).unwrap();
        subject.write(&file, name.as_bytes());
        assert_eq!(
            subject.read(&file),
            name.as_bytes(),
            "[{}] {name:?}",
            subject.name
        );
        // The location of the entry reads back to the same entry.
        let again = VfsPath::from_location(&file.to_location()).unwrap();
        assert_eq!(
            p.stat(&again).unwrap().name,
            OsString::from(name),
            "[{}] {name:?}",
            subject.name
        );
    }
    let listed = subject.names(&folder);
    let wanted: BTreeSet<OsString> = AWKWARD_NAMES.iter().map(OsString::from).collect();
    assert_eq!(
        listed, wanted,
        "[{}] every name is listed as written",
        subject.name
    );
}

/// Names that differ only in case are one entry or two, as the provider's case rule says.
pub fn case_rule(subject: &Subject) {
    let p = subject.provider;
    let folder = subject.at("case");
    p.create_dir(&folder).unwrap();
    p.create_file(&folder.join("Readme").unwrap()).unwrap();
    let other = p.stat(&folder.join("README").unwrap());
    match p.capabilities().case_rule {
        CaseRule::Sensitive => subject.expect("stat another case of a name", &other, &["notFound"]),
        CaseRule::Insensitive => subject.expect("stat another case of a name", &other, &["ok"]),
    }
}

/// Reading from an offset and, where claimed, resuming a write.
pub fn ranges_and_resumes(subject: &Subject) {
    let p = subject.provider;
    let file = subject.at("ranges.txt");
    subject.write(&file, b"hello world");
    let mut tail = Vec::new();
    p.open_read_at(&file, 6)
        .unwrap()
        .read_to_end(&mut tail)
        .unwrap();
    assert_eq!(tail, b"world", "[{}] a read from an offset", subject.name);
    let mut past = Vec::new();
    p.open_read_at(&file, 100)
        .unwrap()
        .read_to_end(&mut past)
        .unwrap();
    assert!(
        past.is_empty(),
        "[{}] a read past the end is empty",
        subject.name
    );
    if p.capabilities().resume_write {
        let mut stream = p.resume_write(&file, 5).unwrap();
        stream.write_all(b", again").unwrap();
        stream.finish(true).unwrap();
        assert_eq!(
            subject.read(&file),
            b"hello, again",
            "[{}] a resumed write",
            subject.name
        );
    }
}

/// Batches add up to the listing, and a cancelled listing says so.
pub fn listing(subject: &Subject) {
    let p = subject.provider;
    let folder = subject.at("listing");
    p.create_dir(&folder).unwrap();
    for n in 0..25 {
        p.create_file(&folder.join(format!("entry {n:02}")).unwrap())
            .unwrap();
    }
    let listed = subject.names(&folder);
    let mut batched = BTreeSet::new();
    p.list_batches(&folder, &CancelToken::new(), usize::MAX, &mut |batch| {
        for entry in batch {
            assert!(
                batched.insert(entry.name),
                "[{}] an entry came in two batches",
                subject.name
            );
        }
    })
    .unwrap();
    assert_eq!(
        batched, listed,
        "[{}] the batches are the listing",
        subject.name
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    subject.expect(
        "list with a cancelled token",
        &p.list(&folder, &cancel, 0, &mut |_| {}),
        &["cancelled"],
    );
}

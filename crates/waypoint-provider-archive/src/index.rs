// The folder tree of one archive, built once from its listing and kept in memory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::Arc;

use tempfile::TempPath;
use waypoint_vfs::EntryKind;

use crate::format::ArchiveFormat;
use crate::names::{resolve_link_target, sanitise, SafeName, UnsafeName};

/// Where an entry's data is, for each format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Locator {
    /// The entry's position among the zip crate's files, and where its local header starts (checked
    /// before reading, so a mismatch between this scan and the library's is an error, not wrong data).
    Zip {
        ordinal: usize,
        header_start: u64,
    },
    /// The entry's ordinal in the tar stream and where its data starts in the uncompressed stream.
    Tar {
        ordinal: usize,
        data_offset: u64,
    },
    SevenZ {
        file_index: usize,
    },
}

/// What a scan knows about one entry.
#[derive(Debug, Clone, Default)]
pub(crate) struct NewEntry {
    pub kind: Option<EntryKind>,
    pub size: Option<u64>,
    pub compressed: Option<u64>,
    pub modified_ms: Option<i64>,
    pub mode: Option<u32>,
    /// The text a symlink holds.
    pub link: Option<Vec<u8>>,
    /// The name a hard link points at.
    pub hardlink: Option<Vec<u8>>,
    pub encrypted: bool,
    pub sparse: bool,
    pub locator: Option<Locator>,
}

#[derive(Debug)]
pub(crate) struct Node {
    pub name: Box<[u8]>,
    pub parent: u32,
    pub kind: EntryKind,
    pub size: Option<u64>,
    pub compressed: Option<u64>,
    pub modified_ms: Option<i64>,
    pub mode: Option<u32>,
    pub link: Option<Box<[u8]>>,
    pub encrypted: bool,
    pub sparse: bool,
    pub unsafe_name: Option<UnsafeName>,
    /// Made up because an entry below it needs a parent the archive does not list.
    pub synthetic: bool,
    /// The name as stored, kept only when it differs from the name shown.
    pub raw_name: Option<Box<[u8]>>,
    pub locator: Option<Locator>,
    pub children: Vec<u32>,
}

impl Node {
    fn folder(name: &[u8], parent: u32, synthetic: bool) -> Self {
        Self {
            name: name.into(),
            parent,
            kind: EntryKind::Directory,
            size: None,
            compressed: None,
            modified_ms: None,
            mode: None,
            link: None,
            encrypted: false,
            sparse: false,
            unsafe_name: None,
            synthetic,
            raw_name: None,
            locator: None,
            children: Vec::new(),
        }
    }
}

/// The tree of an archive. Node 0 is its top.
pub(crate) struct ArchiveIndex {
    pub format: ArchiveFormat,
    nodes: Vec<Node>,
    lookup: HashMap<(u32, Box<[u8]>), u32>,
    pending_hardlinks: Vec<(u32, Vec<u8>)>,
    /// Entries the archive lists (folders made up for them are not counted).
    pub entries: usize,
    pub encrypted_entries: usize,
    pub unsafe_names: usize,
    /// The archive's names and sizes are encrypted too, so it cannot be listed without a password.
    pub header_encrypted: bool,
    /// Listing had to read the file from its start.
    pub slow_scan: bool,
    /// How big the archive file was when this was built, and when it was changed: a different
    /// signature means the tree is stale.
    pub signature: (u64, Option<i64>),
    /// A scratch copy of the archive, for one inside another archive. Kept alive with the index.
    pub spool: Option<Arc<TempPath>>,
    pub comment: Option<String>,
}

impl ArchiveIndex {
    pub fn new(format: ArchiveFormat, signature: (u64, Option<i64>)) -> Self {
        Self {
            format,
            nodes: vec![Node::folder(b"", 0, true)],
            lookup: HashMap::new(),
            pending_hardlinks: Vec::new(),
            entries: 0,
            encrypted_entries: 0,
            unsafe_names: 0,
            header_encrypted: false,
            slow_scan: false,
            signature,
            spool: None,
            comment: None,
        }
    }

    pub fn node(&self, index: u32) -> &Node {
        &self.nodes[index as usize]
    }

    pub fn children(&self, index: u32) -> &[u32] {
        &self.nodes[index as usize].children
    }

    /// Adds one entry as the archive stores it. A name used twice keeps the later entry, as
    /// extracting would.
    pub fn insert(&mut self, raw_name: &[u8], backslash: bool, entry: NewEntry) {
        let safe = sanitise(raw_name, backslash);
        self.insert_safe(raw_name, safe, entry);
    }

    pub fn insert_safe(&mut self, raw_name: &[u8], safe: SafeName, entry: NewEntry) {
        self.entries += 1;
        if entry.encrypted {
            self.encrypted_entries += 1;
        }
        let mut flag = safe.unsafe_name;
        let Some((last, folders)) = safe.components.split_last() else {
            // The entry is the top of the archive itself (`./`): it only carries its folder's data.
            if entry.kind == Some(EntryKind::Directory) {
                let root = &mut self.nodes[0];
                root.modified_ms = entry.modified_ms.or(root.modified_ms);
                root.mode = entry.mode.or(root.mode);
            }
            return;
        };
        let mut parent = 0u32;
        for part in folders {
            parent = self.step_into(parent, part, &mut flag);
        }
        let mut kind = entry.kind.unwrap_or(EntryKind::File);
        if safe.flat && kind == EntryKind::Directory {
            // A folder whose name climbs out is listed, but is not one that opens.
            kind = EntryKind::File;
        }
        let existing = self.lookup.get(&(parent, last.as_slice().into())).copied();
        let index = match existing {
            Some(index) => {
                let node = &self.nodes[index as usize];
                if node.kind == EntryKind::Directory
                    && !node.children.is_empty()
                    && kind != EntryKind::Directory
                {
                    log::debug!(
                        "archive: a file replaces a folder with contents; keeping the folder"
                    );
                    return;
                }
                index
            }
            None => {
                let index = self.nodes.len() as u32;
                self.nodes.push(Node::folder(last, parent, false));
                self.nodes[parent as usize].children.push(index);
                self.lookup.insert((parent, last.as_slice().into()), index);
                index
            }
        };
        if flag.is_some() {
            self.unsafe_names += 1;
        }
        let shown_differs =
            safe_join(&safe.components) != raw_name.strip_suffix(b"/").unwrap_or(raw_name);
        let node = &mut self.nodes[index as usize];
        node.kind = kind;
        node.size = if kind == EntryKind::Directory {
            None
        } else {
            entry.size
        };
        node.compressed = entry.compressed;
        node.modified_ms = entry.modified_ms;
        node.mode = entry.mode;
        node.link = entry.link.map(Vec::into_boxed_slice);
        node.encrypted = entry.encrypted;
        node.sparse = entry.sparse;
        node.unsafe_name = flag;
        node.synthetic = false;
        node.raw_name = shown_differs.then(|| raw_name.into());
        node.locator = entry.locator;
        if let Some(target) = entry.hardlink {
            self.pending_hardlinks.push((index, target));
        }
    }

    /// The folder `part` inside `parent`, made if missing and made a folder if it is something else.
    fn step_into(&mut self, parent: u32, part: &Vec<u8>, flag: &mut Option<UnsafeName>) -> u32 {
        if let Some(&index) = self.lookup.get(&(parent, part.as_slice().into())) {
            let node = &mut self.nodes[index as usize];
            if node.kind != EntryKind::Directory {
                // Something is stored below a name that is a file or a link: it becomes a folder,
                // so the entries below it can be reached, and they are marked.
                node.kind = EntryKind::Directory;
                node.size = None;
                node.link = None;
                node.locator = None;
                node.encrypted = false;
                node.synthetic = true;
                flag.get_or_insert(UnsafeName::ThroughLink);
            }
            return index;
        }
        let index = self.nodes.len() as u32;
        self.nodes.push(Node::folder(part, parent, true));
        self.nodes[parent as usize].children.push(index);
        self.lookup.insert((parent, part.as_slice().into()), index);
        index
    }

    /// Links hard links to what they point at. Call once after the last `insert`.
    pub fn finish(&mut self) {
        for (index, target) in std::mem::take(&mut self.pending_hardlinks) {
            let safe = sanitise(&target, false);
            let Some(found) = self.find(&safe.components) else {
                continue;
            };
            let source = &self.nodes[found as usize];
            if source.kind != EntryKind::File {
                continue;
            }
            let (size, compressed, locator, sparse) = (
                source.size,
                source.compressed,
                source.locator.clone(),
                source.sparse,
            );
            let node = &mut self.nodes[index as usize];
            node.kind = EntryKind::File;
            node.size = size;
            node.compressed = compressed;
            node.locator = locator;
            node.sparse = sparse;
        }
    }

    pub fn find(&self, components: &[Vec<u8>]) -> Option<u32> {
        let mut at = 0u32;
        for part in components {
            at = *self.lookup.get(&(at, part.as_slice().into()))?;
        }
        Some(at)
    }

    /// The names from the top of the archive down to `index`.
    pub fn path_of(&self, index: u32) -> Vec<Vec<u8>> {
        let mut parts = Vec::new();
        let mut at = index;
        while at != 0 {
            let node = &self.nodes[at as usize];
            parts.push(node.name.to_vec());
            at = node.parent;
        }
        parts.reverse();
        parts
    }

    /// Follows a link inside the archive, up to eight links deep. `None` for a link that is
    /// broken, points outside the archive or loops.
    pub fn follow(&self, index: u32) -> Option<u32> {
        let mut at = index;
        for _ in 0..8 {
            let node = &self.nodes[at as usize];
            if node.kind != EntryKind::Symlink {
                return Some(at);
            }
            let target = node.link.as_deref()?;
            let folder = self.path_of(node.parent);
            let resolved = resolve_link_target(&folder, target)?;
            at = self.find(&resolved)?;
        }
        None
    }

    /// The kind of what a symlink points at, or `None` when it is broken.
    pub fn link_target_kind(&self, index: u32) -> Option<EntryKind> {
        let target = self.follow(index)?;
        Some(self.nodes[target as usize].kind)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
}

/// The components joined with `/`, to compare with the name as stored.
fn safe_join(components: &[Vec<u8>]) -> Vec<u8> {
    components.join(&b'/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::ArchiveFormat;

    fn index() -> ArchiveIndex {
        ArchiveIndex::new(ArchiveFormat::Zip, (0, None))
    }

    fn file(size: u64) -> NewEntry {
        NewEntry {
            kind: Some(EntryKind::File),
            size: Some(size),
            ..NewEntry::default()
        }
    }

    fn folder() -> NewEntry {
        NewEntry {
            kind: Some(EntryKind::Directory),
            ..NewEntry::default()
        }
    }

    fn path(parts: &[&str]) -> Vec<Vec<u8>> {
        parts.iter().map(|part| part.as_bytes().to_vec()).collect()
    }

    #[test]
    fn folders_the_archive_omits_are_made_up() {
        let mut index = index();
        index.insert(b"a/b/c.txt", false, file(3));
        let a = index.find(&path(&["a"])).unwrap();
        assert!(index.node(a).synthetic);
        assert_eq!(index.node(a).kind, EntryKind::Directory);
        assert_eq!(index.entries, 1);
        let c = index.find(&path(&["a", "b", "c.txt"])).unwrap();
        assert_eq!(index.node(c).size, Some(3));
        assert_eq!(index.path_of(c), path(&["a", "b", "c.txt"]));
    }

    #[test]
    fn a_listed_folder_replaces_the_made_up_one() {
        let mut index = index();
        index.insert(b"a/x", false, file(1));
        index.insert(b"a/", false, folder());
        let a = index.find(&path(&["a"])).unwrap();
        assert!(!index.node(a).synthetic);
        assert_eq!(index.children(a).len(), 1);
    }

    #[test]
    fn a_name_used_twice_keeps_the_later_entry() {
        let mut index = index();
        index.insert(b"f", false, file(1));
        index.insert(b"f", false, file(2));
        let f = index.find(&path(&["f"])).unwrap();
        assert_eq!(index.node(f).size, Some(2));
        assert_eq!(index.children(0).len(), 1);
    }

    #[test]
    fn unsafe_names_are_shown_safely_and_flagged() {
        let mut index = index();
        index.insert(b"../../evil", false, file(1));
        index.insert(b"/abs/path", false, file(1));
        let evil = index.find(&path(&["..\u{2215}..\u{2215}evil"])).unwrap();
        assert_eq!(index.node(evil).unsafe_name, Some(UnsafeName::Traversal));
        assert_eq!(
            index.node(evil).raw_name.as_deref(),
            Some(&b"../../evil"[..])
        );
        let abs = index.find(&path(&["abs", "path"])).unwrap();
        assert_eq!(index.node(abs).unsafe_name, Some(UnsafeName::Absolute));
        assert_eq!(index.unsafe_names, 2);
    }

    #[test]
    fn an_entry_below_a_link_turns_the_link_into_a_folder_and_is_flagged() {
        let mut index = index();
        index.insert(
            b"link",
            false,
            NewEntry {
                kind: Some(EntryKind::Symlink),
                link: Some(b"/etc".to_vec()),
                ..NewEntry::default()
            },
        );
        index.insert(b"link/passwd", false, file(1));
        let link = index.find(&path(&["link"])).unwrap();
        assert_eq!(index.node(link).kind, EntryKind::Directory);
        let passwd = index.find(&path(&["link", "passwd"])).unwrap();
        assert_eq!(
            index.node(passwd).unsafe_name,
            Some(UnsafeName::ThroughLink)
        );
    }

    #[test]
    fn links_resolve_within_the_archive() {
        let mut index = index();
        index.insert(b"dir/target", false, file(5));
        for (name, target) in [
            ("dir/ok", "target"),
            ("dir/up", "../dir/target"),
            ("dir/out", "../../x"),
            ("dir/abs", "/etc"),
        ] {
            index.insert(
                name.as_bytes(),
                false,
                NewEntry {
                    kind: Some(EntryKind::Symlink),
                    link: Some(target.as_bytes().to_vec()),
                    ..NewEntry::default()
                },
            );
        }
        let kind = |name: &str| {
            let at = index.find(&path(&["dir", name])).unwrap();
            index.link_target_kind(at)
        };
        assert_eq!(kind("ok"), Some(EntryKind::File));
        assert_eq!(kind("up"), Some(EntryKind::File));
        assert_eq!(kind("out"), None);
        assert_eq!(kind("abs"), None);
    }

    #[test]
    fn a_link_loop_is_broken_not_endless() {
        let mut index = index();
        for (name, target) in [("a", "b"), ("b", "a")] {
            index.insert(
                name.as_bytes(),
                false,
                NewEntry {
                    kind: Some(EntryKind::Symlink),
                    link: Some(target.as_bytes().to_vec()),
                    ..NewEntry::default()
                },
            );
        }
        let a = index.find(&path(&["a"])).unwrap();
        assert_eq!(index.link_target_kind(a), None);
    }

    #[test]
    fn a_hard_link_takes_its_targets_data() {
        let mut index = index();
        index.insert(
            b"orig",
            false,
            NewEntry {
                locator: Some(Locator::Tar {
                    ordinal: 0,
                    data_offset: 512,
                }),
                ..file(9)
            },
        );
        index.insert(
            b"copy",
            false,
            NewEntry {
                kind: Some(EntryKind::File),
                hardlink: Some(b"orig".to_vec()),
                ..NewEntry::default()
            },
        );
        index.finish();
        let copy = index.find(&path(&["copy"])).unwrap();
        assert_eq!(index.node(copy).size, Some(9));
        assert_eq!(
            index.node(copy).locator,
            Some(Locator::Tar {
                ordinal: 0,
                data_offset: 512
            })
        );
    }
}

// Reading a tree into plain data and building one from it, through a provider, so a test can compare
// what an operation left against what a simple model says it should.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::io::{Read, Write};

use waypoint_path::VfsPath;
use waypoint_vfs::{CancelToken, EntryKind, Provider, WriteOptions};

/// One entry of a tree, without its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Dir,
    File(Vec<u8>),
    Link(String),
}

/// Entries by their path below a root, joined with `/`. The root itself is not in it.
pub type Tree = BTreeMap<String, Node>;

/// Reads everything below `root`, never following a link.
pub fn tree_of(provider: &dyn Provider, root: &VfsPath) -> Tree {
    let mut out = Tree::new();
    let mut stack = vec![(root.clone(), String::new())];
    while let Some((folder, prefix)) = stack.pop() {
        let entries = provider
            .list(&folder, &CancelToken::new(), 0, &mut |_| {})
            .unwrap_or_else(|e| panic!("listing {}: {e:?}", folder.display()));
        for entry in entries {
            let name = entry.name.to_string_lossy().into_owned();
            let key = format!("{prefix}{name}");
            let path = folder.join(&entry.name).expect("a listed name joins");
            match entry.kind {
                EntryKind::Directory => {
                    out.insert(key.clone(), Node::Dir);
                    stack.push((path, format!("{key}/")));
                }
                EntryKind::Symlink => {
                    let text = provider.read_link(&path).expect("a link reads");
                    out.insert(key, Node::Link(text.to_string_lossy().into_owned()));
                }
                _ => {
                    let mut bytes = Vec::new();
                    provider
                        .open_read(&path)
                        .expect("a file opens")
                        .read_to_end(&mut bytes)
                        .expect("a file reads");
                    out.insert(key, Node::File(bytes));
                }
            }
        }
    }
    out
}

/// Builds `tree` below `root` (which exists) with the provider's own write primitives. Parents come
/// before children because the map is ordered by path.
pub fn populate(provider: &dyn Provider, root: &VfsPath, tree: &Tree) {
    for (key, node) in tree {
        let path = key
            .split('/')
            .fold(root.clone(), |p, name| p.join(name).expect("a name joins"));
        match node {
            Node::Dir => provider
                .create_dir(&path)
                .unwrap_or_else(|e| panic!("creating {key}: {e:?}")),
            Node::File(bytes) => {
                let mut stream = provider
                    .create_write(&path, WriteOptions::exclusive())
                    .unwrap_or_else(|e| panic!("creating {key}: {e:?}"));
                stream.write_all(bytes).expect("a file writes");
                stream.finish(false).expect("a file closes");
            }
            Node::Link(text) => provider
                .symlink(&path, text.as_ref())
                .unwrap_or_else(|e| panic!("linking {key}: {e:?}")),
        }
    }
}

/// The entries whose name is a partial file's, which a finished or cancelled job leaves none of.
pub fn partials(tree: &Tree) -> Vec<String> {
    tree.keys()
        .filter(|key| key.split('/').any(|n| n.starts_with(".waypoint-partial-")))
        .cloned()
        .collect()
}

/// The tree without the partial entries.
pub fn without_partials(tree: &Tree) -> Tree {
    tree.iter()
        .filter(|(key, _)| !key.split('/').any(|n| n.starts_with(".waypoint-partial-")))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// A small deterministic random source (a linear congruential generator), so a failing seed
/// reproduces.
#[derive(Debug, Clone)]
pub struct Rng(pub u64);

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    /// A number below `bound` (which may be zero, giving zero).
    pub fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % bound.max(1)
    }

    pub fn chance(&mut self, in_n: usize) -> bool {
        self.below(in_n) == 0
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// A random tree of folders, files and links over a few names that clash under both case rules and
/// that both rules accept.
pub fn random_tree(rng: &mut Rng, size: usize) -> Tree {
    const NAMES: [&str; 8] = [
        "a", "B", "b", "c.txt", "C.TXT", "d (2).md", "e.tar.gz", ".hid",
    ];
    let mut tree = Tree::new();
    let mut folders = vec![String::new()];
    let mut insensitive: std::collections::HashSet<String> = std::collections::HashSet::new();
    for _ in 0..size {
        let parent = rng.pick(&folders).clone();
        let name = *rng.pick(&NAMES);
        let key = if parent.is_empty() {
            name.to_owned()
        } else {
            format!("{parent}/{name}")
        };
        // Names that differ only in case would be one entry on a case-insensitive provider, so a
        // random tree never holds two of them side by side.
        if !insensitive.insert(key.to_uppercase()) {
            continue;
        }
        match rng.below(5) {
            0 | 1 => {
                tree.insert(key.clone(), Node::Dir);
                folders.push(key);
            }
            2 => {
                tree.insert(key, Node::Link("target-that-may-not-exist".to_owned()));
            }
            _ => {
                // Now and then a file longer than one copy chunk.
                let len = if rng.chance(25) {
                    70_000
                } else {
                    rng.below(300)
                };
                let byte = rng.below(250) as u8;
                tree.insert(key, Node::File(vec![byte; len]));
            }
        }
    }
    tree
}

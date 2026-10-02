// Natural, case-folded ordering with a cached key per entry.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cmp::Ordering;
use std::ffi::OsStr;

use crate::icon::extension;
use crate::model::{EntryKind, SortKey, SortSpec};

/// Builds the sort key for a file name: a byte string whose plain byte order is the natural order
/// of the name.
///
/// Letters are folded with Unicode lower-casing, so `Ab` and `aB` produce the same key. A run of
/// ASCII digits is written as the marker `0`, a length byte and the digits without leading zeros,
/// so `file9` sorts before `file10` and `a01` equals `a1` (the raw name breaks the tie). Bytes that are not valid
/// UTF-8 (a Linux name may hold any bytes) are copied as they are, so they order deterministically
/// after ASCII. A digit run longer than 255 is written in pieces, which only matters for names no
/// one types.
pub fn natural_key(name: &OsStr) -> Box<[u8]> {
    let bytes = name.as_encoded_bytes();
    let mut out = Vec::with_capacity(bytes.len() + 4);
    for chunk in bytes.utf8_chunks() {
        push_folded(&mut out, chunk.valid());
        out.extend_from_slice(chunk.invalid());
    }
    out.into_boxed_slice()
}

fn push_folded(out: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte.is_ascii_digit() {
            // ASCII digits are single bytes and never occur inside a multi-byte UTF-8 sequence.
            let mut end = i;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            let run = &bytes[i..end];
            let start = run.iter().position(|&d| d != b'0').unwrap_or(run.len());
            let significant = &run[start..];
            if significant.is_empty() {
                out.extend_from_slice(&[b'0', 0]);
            }
            for piece in significant.chunks(255) {
                out.push(b'0');
                out.push(piece.len() as u8);
                out.extend_from_slice(piece);
            }
            i = end;
        } else if byte.is_ascii() {
            out.push(byte.to_ascii_lowercase());
            i += 1;
        } else {
            let c = text[i..].chars().next().unwrap_or('\u{fffd}');
            let mut buf = [0u8; 4];
            for lower in c.to_lowercase() {
                out.extend_from_slice(lower.encode_utf8(&mut buf).as_bytes());
            }
            i += c.len_utf8();
        }
    }
}

/// What the ordering needs to know about an entry.
pub(crate) struct Sortable<'a> {
    /// The unique name that identifies the entry: the last tie-break.
    pub name: &'a OsStr,
    /// The name people read, which the Kind column takes its extension from (the same as `name`
    /// except in the Trash, where `name` is an id).
    pub label: &'a OsStr,
    pub key: &'a [u8],
    pub kind: EntryKind,
    pub link_target: Option<EntryKind>,
    pub group: u8,
    pub size: Option<u64>,
    pub modified_ms: Option<i64>,
    pub deleted_ms: Option<i64>,
}

impl Sortable<'_> {
    fn is_directory(&self) -> bool {
        self.kind == EntryKind::Directory || self.link_target == Some(EntryKind::Directory)
    }
}

fn cmp_extension(a: &[u8], b: &[u8]) -> Ordering {
    a.iter()
        .map(u8::to_ascii_lowercase)
        .cmp(b.iter().map(u8::to_ascii_lowercase))
}

/// Compares two entries under `sort`. The result is a total order: ties fall back to the natural
/// name order and then to the raw name bytes, so no two entries of a folder compare equal and
/// binary search finds an entry exactly.
///
/// `descending` flips the chosen column only; folders stay first when `directories_first` is on and
/// name ties always read in ascending order.
pub(crate) fn compare(sort: SortSpec, a: &Sortable, b: &Sortable) -> Ordering {
    if sort.directories_first {
        match (a.is_directory(), b.is_directory()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
    }
    let primary = match sort.key {
        SortKey::Name => a.key.cmp(b.key),
        SortKey::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
        SortKey::Modified => a
            .modified_ms
            .unwrap_or(i64::MIN)
            .cmp(&b.modified_ms.unwrap_or(i64::MIN)),
        SortKey::Kind => a.group.cmp(&b.group).then_with(|| {
            cmp_extension(
                extension(a.label.as_encoded_bytes()),
                extension(b.label.as_encoded_bytes()),
            )
        }),
        SortKey::Deleted => a
            .deleted_ms
            .unwrap_or(i64::MIN)
            .cmp(&b.deleted_ms.unwrap_or(i64::MIN)),
    };
    let primary = if sort.descending {
        primary.reverse()
    } else {
        primary
    };
    primary
        .then_with(|| a.key.cmp(b.key))
        .then_with(|| a.name.as_encoded_bytes().cmp(b.name.as_encoded_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut keyed: Vec<(Box<[u8]>, &str)> = names
            .iter()
            .map(|n| (natural_key(OsStr::new(n)), *n))
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
        keyed.into_iter().map(|(_, n)| n.to_owned()).collect()
    }

    #[test]
    fn digit_runs_compare_as_numbers() {
        assert_eq!(
            sorted(&["file10", "file9", "file2", "file1", "file100"]),
            ["file1", "file2", "file9", "file10", "file100"]
        );
        assert_eq!(sorted(&["a10b", "a9b", "a9a"]), ["a9a", "a9b", "a10b"]);
    }

    #[test]
    fn leading_zeros_do_not_change_the_value_and_ties_are_deterministic() {
        assert_eq!(
            natural_key(OsStr::new("a01")),
            natural_key(OsStr::new("a1"))
        );
        assert_eq!(
            sorted(&["a2", "a01", "a1", "a001"]),
            ["a001", "a01", "a1", "a2"]
        );
        assert_eq!(
            natural_key(OsStr::new("a0")),
            natural_key(OsStr::new("a00"))
        );
    }

    #[test]
    fn case_is_folded_across_unicode() {
        assert_eq!(
            natural_key(OsStr::new("ÉCOLE")),
            natural_key(OsStr::new("école"))
        );
        assert_eq!(
            natural_key(OsStr::new("Straße")),
            natural_key(OsStr::new("straße"))
        );
        assert_eq!(sorted(&["b", "A", "a", "B"]), ["A", "a", "B", "b"]);
    }

    #[test]
    fn digits_sort_before_letters_and_punctuation_by_byte_value() {
        assert_eq!(sorted(&["ab", "a1", "a-", "a_"]), ["a-", "a1", "a_", "ab"]);
    }

    #[cfg(unix)]
    #[test]
    fn names_that_are_not_utf8_keep_their_bytes() {
        use std::os::unix::ffi::OsStrExt;
        let raw = OsStr::from_bytes(b"caf\xe9-9");
        let key = natural_key(raw);
        assert_eq!(&key[..5], b"caf\xe9-");
        assert!(natural_key(OsStr::from_bytes(b"a\xffb")) > natural_key(OsStr::new("az")));
    }

    #[test]
    fn very_long_digit_runs_do_not_overflow_the_length_byte() {
        let long = format!("x{}", "7".repeat(600));
        assert!(natural_key(OsStr::new(&long)).len() > 600);
    }
}

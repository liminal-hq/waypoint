// What the conflict dialog shows of two files that want one name: both sides' size and time, and for
// small text files a line diff (A46). Reads only, through providers, so a remote provider that
// cannot be read answers `Unavailable`, and bounded in bytes, lines, rows and time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use similar::{Algorithm, ChangeTag, TextDiff as LineDiff};
use ts_rs::TS;
use waypoint_protocol::Location;
use waypoint_vfs::{CancelToken, EntryKind, Provider, ScannedEntry};

use crate::model::{Conflict, ConflictKind, OpsError};
use crate::traits::Providers;

/// The most bytes of either side a diff reads: 256 KiB, as the text preview does.
pub const PREVIEW_MAX_BYTES: u64 = 256 * 1024;
/// The most lines either side may have before the diff is not attempted.
pub const PREVIEW_MAX_LINES: usize = 20_000;
/// The most diff rows sent; the rest are counted in `TextDiff::more`.
pub const PREVIEW_MAX_ROWS: usize = 1_000;
/// The most characters of a line sent; a longer line ends with an ellipsis.
pub const PREVIEW_MAX_LINE_CHARS: usize = 400;
/// The time the diff may take; past it the result is still correct but not always the shortest.
pub const PREVIEW_DIFF_BUDGET: Duration = Duration::from_millis(500);
/// Unchanged lines kept on each side of a change.
pub const PREVIEW_CONTEXT: usize = 3;

const CHUNK: usize = 32 * 1024;

/// One side of a clash as it is now: read again when the preview is made, so it is what the
/// answer will act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct PreviewSide {
    pub location: Location,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    #[ts(type = "number | null")]
    pub modified_ms: Option<i64>,
}

/// One row of a unified diff. Line numbers count from 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum DiffLine {
    Context {
        old_line: u32,
        new_line: u32,
        text: String,
    },
    Added {
        new_line: u32,
        text: String,
    },
    Removed {
        old_line: u32,
        text: String,
    },
    /// Unchanged lines left out between two changes.
    Gap {
        lines: u32,
    },
}

/// The line diff from the existing file to the incoming one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct TextDiff {
    /// At most `PREVIEW_MAX_ROWS` rows.
    pub lines: Vec<DiffLine>,
    /// Added and removed lines in the whole diff, shown or not.
    pub added: u32,
    pub removed: u32,
    /// Changed and context rows left out past the row cap.
    pub more: u32,
    /// The time budget ran out, so the diff may be longer than the shortest one.
    pub approximate: bool,
    /// A side held bytes that are not UTF-8, shown as replacement characters.
    pub lossy: bool,
}

/// What comparing the two files found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum PreviewKind {
    /// Byte for byte the same.
    Identical,
    Text {
        diff: TextDiff,
    },
    /// Different, and not text (a NUL byte).
    Binary,
    /// Past the byte or line limit, so nothing was compared.
    TooLarge,
    /// Not two readable files here: a folder, a clash inside the batch, a provider that cannot
    /// read, or a read that failed.
    Unavailable,
}

/// The answer to `conflict_preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ConflictPreview {
    pub existing: PreviewSide,
    pub incoming: PreviewSide,
    pub kind: PreviewKind,
}

/// Compares the two files of a clash. The result is `Unavailable` rather than an error when a side
/// cannot be read; only a cancel is an error (`OpsError::Cancelled`), so a caller that is told to
/// stop can tell.
pub fn conflict_preview(
    providers: &Providers,
    conflict: &Conflict,
    cancel: &CancelToken,
) -> Result<ConflictPreview, OpsError> {
    let mut existing = PreviewSide {
        location: conflict.existing.clone(),
        size: conflict.existing_size,
        modified_ms: conflict.existing_modified_ms,
    };
    let mut incoming = PreviewSide {
        location: conflict.source.clone(),
        size: conflict.source_size,
        modified_ms: conflict.source_modified_ms,
    };
    let done = |existing, incoming, kind| {
        Ok(ConflictPreview {
            existing,
            incoming,
            kind,
        })
    };
    // Nothing exists yet when two sources of one request clash, and only two files compare.
    if conflict.within_batch || conflict.kind != ConflictKind::FileOverFile {
        return done(existing, incoming, PreviewKind::Unavailable);
    }
    let (Ok((old_path, old_provider)), Ok((new_path, new_provider))) = (
        providers.for_location(&conflict.existing),
        providers.for_location(&conflict.source),
    ) else {
        return done(existing, incoming, PreviewKind::Unavailable);
    };
    let (Ok(old_entry), Ok(new_entry)) =
        (old_provider.stat(&old_path), new_provider.stat(&new_path))
    else {
        return done(existing, incoming, PreviewKind::Unavailable);
    };
    existing.size = old_entry.size.or(existing.size);
    existing.modified_ms = old_entry.modified_ms.or(existing.modified_ms);
    incoming.size = new_entry.size.or(incoming.size);
    incoming.modified_ms = new_entry.modified_ms.or(incoming.modified_ms);
    if !is_file(&old_entry) || !is_file(&new_entry) {
        return done(existing, incoming, PreviewKind::Unavailable);
    }
    if old_entry.size.is_some_and(|s| s > PREVIEW_MAX_BYTES)
        || new_entry.size.is_some_and(|s| s > PREVIEW_MAX_BYTES)
    {
        return done(existing, incoming, PreviewKind::TooLarge);
    }
    let old_bytes = match read_capped(old_provider.as_ref(), &old_path, cancel)? {
        ReadOutcome::Bytes(bytes) => bytes,
        ReadOutcome::TooLarge => return done(existing, incoming, PreviewKind::TooLarge),
        ReadOutcome::Failed => return done(existing, incoming, PreviewKind::Unavailable),
    };
    let new_bytes = match read_capped(new_provider.as_ref(), &new_path, cancel)? {
        ReadOutcome::Bytes(bytes) => bytes,
        ReadOutcome::TooLarge => return done(existing, incoming, PreviewKind::TooLarge),
        ReadOutcome::Failed => return done(existing, incoming, PreviewKind::Unavailable),
    };
    existing.size = Some(old_bytes.len() as u64);
    incoming.size = Some(new_bytes.len() as u64);
    if old_bytes == new_bytes {
        return done(existing, incoming, PreviewKind::Identical);
    }
    if old_bytes.contains(&0) || new_bytes.contains(&0) {
        return done(existing, incoming, PreviewKind::Binary);
    }
    let (old_text, old_lossy) = decode(&old_bytes);
    let (new_text, new_lossy) = decode(&new_bytes);
    if count_lines(&old_text) > PREVIEW_MAX_LINES || count_lines(&new_text) > PREVIEW_MAX_LINES {
        return done(existing, incoming, PreviewKind::TooLarge);
    }
    let mut diff = line_diff(&old_text, &new_text);
    diff.lossy = old_lossy || new_lossy;
    if cancel.is_cancelled() {
        return Err(OpsError::Cancelled);
    }
    done(existing, incoming, PreviewKind::Text { diff })
}

fn is_file(entry: &ScannedEntry) -> bool {
    match entry.kind {
        EntryKind::File => true,
        EntryKind::Symlink => entry.link_target == Some(EntryKind::File),
        _ => false,
    }
}

enum ReadOutcome {
    Bytes(Vec<u8>),
    TooLarge,
    Failed,
}

/// Reads a file up to the byte limit in chunks, looking at the cancel token between them.
fn read_capped(
    provider: &dyn Provider,
    path: &waypoint_path::VfsPath,
    cancel: &CancelToken,
) -> Result<ReadOutcome, OpsError> {
    let Ok(mut stream) = provider.open_read(path) else {
        return Ok(ReadOutcome::Failed);
    };
    let mut bytes = Vec::new();
    let mut chunk = vec![0u8; CHUNK];
    loop {
        if cancel.is_cancelled() {
            return Err(OpsError::Cancelled);
        }
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(ReadOutcome::Bytes(bytes)),
            Ok(n) => {
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.len() as u64 > PREVIEW_MAX_BYTES {
                    return Ok(ReadOutcome::TooLarge);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Ok(ReadOutcome::Failed),
        }
    }
}

/// Decodes as UTF-8, replacing bad bytes, without a byte-order mark.
fn decode(bytes: &[u8]) -> (String, bool) {
    let text = String::from_utf8_lossy(bytes);
    let lossy = matches!(text, std::borrow::Cow::Owned(_));
    (
        text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned(),
        lossy,
    )
}

fn count_lines(text: &str) -> usize {
    text.lines().count()
}

fn clip(line: &str) -> String {
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    if line.chars().count() > PREVIEW_MAX_LINE_CHARS {
        let mut cut: String = line.chars().take(PREVIEW_MAX_LINE_CHARS).collect();
        cut.push('…');
        cut
    } else {
        line.to_owned()
    }
}

fn number(index: Option<usize>) -> u32 {
    index.map_or(0, |i| (i + 1).min(u32::MAX as usize) as u32)
}

/// A unified line diff: changes with `PREVIEW_CONTEXT` lines around them and a `Gap` where the
/// unchanged lines between them were left out.
fn line_diff(old: &str, new: &str) -> TextDiff {
    let started = std::time::Instant::now();
    let diff = LineDiff::configure()
        .algorithm(Algorithm::Myers)
        .timeout(PREVIEW_DIFF_BUDGET)
        .diff_lines(old, new);
    let mut out = TextDiff {
        lines: Vec::new(),
        added: 0,
        removed: 0,
        more: 0,
        approximate: false,
        lossy: false,
    };
    let old_total = diff.old_slices().len();
    let mut old_end = 0usize;
    let mut any = false;
    let push = |out: &mut TextDiff, row: DiffLine| {
        if out.lines.len() < PREVIEW_MAX_ROWS {
            out.lines.push(row);
        } else if !matches!(row, DiffLine::Gap { .. }) {
            out.more += 1;
        }
    };
    for group in diff.grouped_ops(PREVIEW_CONTEXT) {
        let Some(first) = group.first() else {
            continue;
        };
        any = true;
        let start = first.old_range().start;
        if start > old_end {
            push(
                &mut out,
                DiffLine::Gap {
                    lines: (start - old_end).min(u32::MAX as usize) as u32,
                },
            );
        }
        for op in &group {
            for change in diff.iter_changes(op) {
                let text = clip(change.value());
                match change.tag() {
                    ChangeTag::Equal => push(
                        &mut out,
                        DiffLine::Context {
                            old_line: number(change.old_index()),
                            new_line: number(change.new_index()),
                            text,
                        },
                    ),
                    ChangeTag::Insert => {
                        out.added += 1;
                        push(
                            &mut out,
                            DiffLine::Added {
                                new_line: number(change.new_index()),
                                text,
                            },
                        );
                    }
                    ChangeTag::Delete => {
                        out.removed += 1;
                        push(
                            &mut out,
                            DiffLine::Removed {
                                old_line: number(change.old_index()),
                                text,
                            },
                        );
                    }
                }
            }
        }
        old_end = group.last().map_or(old_end, |op| op.old_range().end);
    }
    if any && old_total > old_end {
        push(
            &mut out,
            DiffLine::Gap {
                lines: (old_total - old_end).min(u32::MAX as usize) as u32,
            },
        );
    }
    // The deadline stops the search where it is, so a diff that used the whole budget may be
    // longer than the shortest one.
    out.approximate = started.elapsed() >= PREVIEW_DIFF_BUDGET;
    out
}

#[cfg(test)]
mod tests {
    use waypoint_path::CaseRule;
    use waypoint_path::{FilePath, VfsPath};
    use waypoint_protocol::VfsError;
    use waypoint_vfs::{MemOp, MemoryProvider};

    use super::*;

    struct Fixture {
        provider: std::sync::Arc<MemoryProvider>,
        providers: Providers,
        root: VfsPath,
    }

    fn fixture() -> Fixture {
        let root = FilePath::from_path(std::path::Path::new(if cfg!(windows) {
            "C:\\preview"
        } else {
            "/preview"
        }))
        .unwrap();
        let provider = std::sync::Arc::new(MemoryProvider::new(root.clone(), CaseRule::Sensitive));
        let root = VfsPath::File(root);
        provider.put_dir(&root);
        Fixture {
            providers: Providers::single(provider.clone()),
            provider,
            root,
        }
    }

    impl Fixture {
        fn put(&self, name: &str, bytes: &[u8]) -> Location {
            let path = self.root.join(name).unwrap();
            self.provider.put_file(&path, bytes);
            path.to_location()
        }

        fn clash(&self, existing: &[u8], incoming: &[u8]) -> Conflict {
            conflict(
                self.put("old.txt", existing),
                self.put("new.txt", incoming),
                ConflictKind::FileOverFile,
            )
        }
    }

    fn conflict(existing: Location, source: Location, kind: ConflictKind) -> Conflict {
        Conflict {
            source,
            existing,
            name: "a.txt".to_owned(),
            kind,
            within_batch: false,
            source_size: None,
            existing_size: None,
            source_modified_ms: None,
            existing_modified_ms: None,
        }
    }

    fn run(f: &Fixture, c: &Conflict) -> ConflictPreview {
        conflict_preview(&f.providers, c, &CancelToken::new()).unwrap()
    }

    fn diff_of(preview: ConflictPreview) -> TextDiff {
        match preview.kind {
            PreviewKind::Text { diff } => diff,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_same_bytes_are_identical_and_the_sides_carry_what_was_read() {
        let f = fixture();
        let preview = run(&f, &f.clash(b"same\n", b"same\n"));
        assert_eq!(preview.kind, PreviewKind::Identical);
        assert_eq!(preview.existing.size, Some(5));
        assert_eq!(preview.incoming.size, Some(5));
    }

    #[test]
    fn a_changed_line_is_one_removed_row_and_one_added_row_with_context() {
        let f = fixture();
        let diff = diff_of(run(&f, &f.clash(b"a\nb\nc\n", b"a\nB\nc\n")));
        assert_eq!((diff.added, diff.removed, diff.more), (1, 1, 0));
        assert_eq!(
            diff.lines,
            vec![
                DiffLine::Context {
                    old_line: 1,
                    new_line: 1,
                    text: "a".into()
                },
                DiffLine::Removed {
                    old_line: 2,
                    text: "b".into()
                },
                DiffLine::Added {
                    new_line: 2,
                    text: "B".into()
                },
                DiffLine::Context {
                    old_line: 3,
                    new_line: 3,
                    text: "c".into()
                },
            ]
        );
        assert!(!diff.approximate && !diff.lossy);
    }

    #[test]
    fn unchanged_stretches_between_changes_become_a_gap() {
        let f = fixture();
        let old: String = (1..=30).map(|n| format!("line {n}\n")).collect();
        let new = old
            .replace("line 2\n", "two\n")
            .replace("line 29\n", "29\n");
        let diff = diff_of(run(&f, &f.clash(old.as_bytes(), new.as_bytes())));
        assert!(diff
            .lines
            .iter()
            .any(|l| matches!(l, DiffLine::Gap { lines } if *lines == 20)));
        assert_eq!((diff.added, diff.removed), (2, 2));
    }

    #[test]
    fn a_long_diff_is_cut_at_the_row_cap_and_counts_the_rest() {
        let f = fixture();
        let new: String = (0..PREVIEW_MAX_ROWS + 50)
            .map(|n| format!("n{n}\n"))
            .collect();
        let diff = diff_of(run(&f, &f.clash(b"x\n", new.as_bytes())));
        assert_eq!(diff.lines.len(), PREVIEW_MAX_ROWS);
        assert_eq!(diff.added as usize, PREVIEW_MAX_ROWS + 50);
        assert_eq!(diff.removed, 1);
        assert_eq!(diff.more as usize, 51);
    }

    #[test]
    fn a_nul_byte_makes_different_files_binary() {
        let f = fixture();
        let preview = run(&f, &f.clash(b"a\0b", b"a\0c"));
        assert_eq!(preview.kind, PreviewKind::Binary);
        // The same binary bytes are still identical.
        assert_eq!(
            run(&f, &f.clash(b"a\0b", b"a\0b")).kind,
            PreviewKind::Identical
        );
    }

    #[test]
    fn a_file_past_the_byte_limit_is_too_large_without_being_read() {
        let f = fixture();
        let big = vec![b'a'; PREVIEW_MAX_BYTES as usize + 1];
        let c = f.clash(&big, b"small");
        assert_eq!(run(&f, &c).kind, PreviewKind::TooLarge);
        assert_eq!(f.provider.calls(MemOp::OpenRead), 0);
        // Exactly at the limit still compares.
        let edge = vec![b'a'; PREVIEW_MAX_BYTES as usize];
        assert!(matches!(
            run(&f, &f.clash(&edge, b"small")).kind,
            PreviewKind::Text { .. }
        ));
    }

    #[test]
    fn too_many_lines_is_too_large() {
        let f = fixture();
        let many = "\n".repeat(PREVIEW_MAX_LINES + 1);
        assert_eq!(
            run(&f, &f.clash(many.as_bytes(), b"x\n")).kind,
            PreviewKind::TooLarge
        );
    }

    #[test]
    fn invalid_utf8_is_shown_replaced_and_marked_lossy() {
        let f = fixture();
        let diff = diff_of(run(&f, &f.clash(b"caf\xe9\n", b"cafe\n")));
        assert!(diff.lossy);
    }

    #[test]
    fn an_unreadable_side_a_missing_side_and_a_folder_are_unavailable() {
        let f = fixture();
        let c = f.clash(b"a", b"b");
        f.provider.fail_always(
            MemOp::OpenRead,
            VfsError::PermissionDenied {
                location: c.existing.clone(),
            },
        );
        assert_eq!(run(&f, &c).kind, PreviewKind::Unavailable);
        f.provider.clear_failures();
        let missing = conflict(
            f.root.join("nope").unwrap().to_location(),
            c.source.clone(),
            ConflictKind::FileOverFile,
        );
        assert_eq!(run(&f, &missing).kind, PreviewKind::Unavailable);
        let dir = f.root.join("dir").unwrap();
        f.provider.put_dir(&dir);
        let over_dir = conflict(
            dir.to_location(),
            c.source.clone(),
            ConflictKind::FileOverFile,
        );
        assert_eq!(run(&f, &over_dir).kind, PreviewKind::Unavailable);
        let mut folders = c.clone();
        folders.kind = ConflictKind::FolderOverFolder;
        assert_eq!(run(&f, &folders).kind, PreviewKind::Unavailable);
        let mut batch = c;
        batch.within_batch = true;
        assert_eq!(run(&f, &batch).kind, PreviewKind::Unavailable);
    }

    #[test]
    fn an_unknown_scheme_is_unavailable() {
        let f = fixture();
        let c = conflict(
            Location::new("x", "sftp://host/a"),
            Location::new("y", "sftp://host/b"),
            ConflictKind::FileOverFile,
        );
        assert_eq!(run(&f, &c).kind, PreviewKind::Unavailable);
    }

    #[test]
    fn a_cancelled_token_stops_the_preview() {
        let f = fixture();
        let c = f.clash(b"a\n", b"b\n");
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(
            conflict_preview(&f.providers, &c, &cancel),
            Err(OpsError::Cancelled)
        );
    }
}

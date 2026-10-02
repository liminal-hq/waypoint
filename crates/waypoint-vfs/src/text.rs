// A file's first bytes as text, for a preview.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;

use crate::{EntryKind, Provider, TextHead};

/// The most a text head holds: 256 KiB.
pub const TEXT_HEAD_MAX: usize = 256 * 1024;

/// Whether `entry` is something with bytes to read: a file, or a link that resolves to one.
/// Opening a pipe or a device to peek at it could block, so nothing else is read.
pub(crate) fn require_file(
    path: &VfsPath,
    kind: EntryKind,
    resolves_to: Option<EntryKind>,
) -> Result<(), VfsError> {
    let effective = if kind == EntryKind::Symlink {
        resolves_to
    } else {
        Some(kind)
    };
    match effective {
        Some(EntryKind::File) => Ok(()),
        Some(EntryKind::Directory) => Err(VfsError::IsADirectory {
            location: path.to_location(),
        }),
        _ => Err(VfsError::Unsupported {
            what: "reading this kind of entry".to_owned(),
        }),
    }
}

/// Reads at most `max` bytes (and never more than `TEXT_HEAD_MAX`) from the start of a file and
/// decodes them as UTF-8, replacing invalid sequences. A file with a NUL byte in what was read is
/// binary and is refused with `NotText`. A multi-byte character cut by the limit is dropped, not
/// replaced, so the head ends cleanly.
pub fn read_text_head<P: Provider + ?Sized>(
    provider: &P,
    path: &VfsPath,
    max: usize,
) -> Result<TextHead, VfsError> {
    let max = max.min(TEXT_HEAD_MAX);
    let entry = provider.stat(path)?;
    require_file(path, entry.kind, entry.link_target)?;
    let location = path.to_location();
    let stream = provider.open_read(path)?;
    let mut bytes = Vec::with_capacity(max.min(64 * 1024) + 1);
    stream
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| crate::from_io(&error, &location))?;
    let truncated = bytes.len() > max;
    bytes.truncate(max);
    if bytes.contains(&0) {
        return Err(VfsError::NotText { location });
    }
    if truncated {
        if let Err(error) = std::str::from_utf8(&bytes) {
            if error.error_len().is_none() {
                bytes.truncate(error.valid_up_to());
            }
        }
    }
    let lossy = std::str::from_utf8(&bytes).is_err();
    let bytes_read = bytes.len() as u64;
    let decoded = String::from_utf8_lossy(&bytes);
    let text = decoded
        .strip_prefix('\u{feff}')
        .unwrap_or(&decoded)
        .to_owned();
    Ok(TextHead {
        text,
        truncated,
        lossy,
        bytes_read,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;

    use waypoint_path::{FilePath, VfsPath};

    use super::*;
    use crate::LocalProvider;

    fn file(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> VfsPath {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        VfsPath::File(FilePath::from_path(&path).unwrap())
    }

    #[test]
    fn a_short_file_comes_back_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(&dir, "a.txt", "héllo\nworld\n".as_bytes());
        let head = read_text_head(&LocalProvider::new(), &path, TEXT_HEAD_MAX).unwrap();
        assert_eq!(head.text, "héllo\nworld\n");
        assert!(!head.truncated && !head.lossy);
        assert_eq!(head.bytes_read, 13);
    }

    #[test]
    fn a_long_file_is_cut_at_the_limit_on_a_character_boundary() {
        let dir = tempfile::tempdir().unwrap();
        // 'é' is two bytes, so a limit of 5 falls in the middle of the third one.
        let path = file(&dir, "a.txt", "ééé".as_bytes());
        let head = read_text_head(&LocalProvider::new(), &path, 5).unwrap();
        assert_eq!(head.text, "éé");
        assert!(head.truncated);
        assert!(!head.lossy);
        assert_eq!(head.bytes_read, 4);
    }

    #[test]
    fn the_default_limit_is_256_kib() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(&dir, "big.txt", &vec![b'a'; TEXT_HEAD_MAX + 10]);
        let head = read_text_head(&LocalProvider::new(), &path, usize::MAX).unwrap();
        assert_eq!(head.text.len(), TEXT_HEAD_MAX);
        assert!(head.truncated);
        let exact = file(&dir, "exact.txt", &vec![b'a'; TEXT_HEAD_MAX]);
        let head = read_text_head(&LocalProvider::new(), &exact, usize::MAX).unwrap();
        assert!(!head.truncated);
    }

    #[test]
    fn invalid_bytes_are_replaced_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(&dir, "latin.txt", b"caf\xe9 ok");
        let head = read_text_head(&LocalProvider::new(), &path, 100).unwrap();
        assert_eq!(head.text, "caf\u{fffd} ok");
        assert!(head.lossy);
    }

    #[test]
    fn a_byte_order_mark_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(&dir, "bom.txt", b"\xef\xbb\xbfhi");
        let head = read_text_head(&LocalProvider::new(), &path, 100).unwrap();
        assert_eq!(head.text, "hi");
    }

    #[test]
    fn binary_data_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = file(&dir, "blob", b"abc\0def");
        assert!(matches!(
            read_text_head(&LocalProvider::new(), &path, 100),
            Err(VfsError::NotText { .. })
        ));
    }

    #[test]
    fn a_folder_and_a_missing_file_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let folder = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
        assert!(matches!(
            read_text_head(&LocalProvider::new(), &folder, 100),
            Err(VfsError::IsADirectory { .. })
        ));
        let missing = file(&dir, "gone", b"x");
        fs::remove_file(dir.path().join("gone")).unwrap();
        assert!(matches!(
            read_text_head(&LocalProvider::new(), &missing, 100),
            Err(VfsError::NotFound { .. })
        ));
    }

    #[test]
    fn a_pipe_is_never_opened() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("pipe");
        let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        // SAFETY: `c` is a valid NUL-terminated path.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let path = VfsPath::File(FilePath::from_path(&fifo).unwrap());
        assert!(matches!(
            read_text_head(&LocalProvider::new(), &path, 100),
            Err(VfsError::Unsupported { .. })
        ));
    }
}

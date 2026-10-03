// Headless tests of serving a file with `Range`: 200, 206, 416 and refusals.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::path::Path;

use waypoint_path::{FilePath, VfsPath};
use waypoint_vfs::{serve_file, LocalProvider, MAX_CHUNK};

fn path(p: &Path) -> VfsPath {
    VfsPath::File(FilePath::from_path(p).unwrap())
}

fn numbered(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

#[test]
fn no_range_serves_the_whole_file_with_its_type() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("pic.png");
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(numbered(100));
    fs::write(&file, &bytes).unwrap();
    let served = serve_file(&LocalProvider::new(), &path(&file), None);
    assert_eq!(served.status, 200);
    assert_eq!(served.body, bytes);
    assert_eq!(served.header("Content-Type"), Some("image/png"));
    assert_eq!(
        served.header("Content-Length"),
        Some(bytes.len().to_string().as_str())
    );
    assert_eq!(served.header("Accept-Ranges"), Some("bytes"));
    assert_eq!(served.header("X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(served.header("Content-Range"), None);
}

#[test]
fn a_range_is_answered_with_206_and_exactly_those_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("clip.mp4");
    let bytes = numbered(1000);
    fs::write(&file, &bytes).unwrap();
    let provider = LocalProvider::new();
    let served = serve_file(&provider, &path(&file), Some("bytes=100-199"));
    assert_eq!(served.status, 206);
    assert_eq!(served.body, &bytes[100..200]);
    assert_eq!(served.header("Content-Range"), Some("bytes 100-199/1000"));
    assert_eq!(served.header("Content-Length"), Some("100"));
    let tail = serve_file(&provider, &path(&file), Some("bytes=-10"));
    assert_eq!(tail.status, 206);
    assert_eq!(tail.body, &bytes[990..]);
    assert_eq!(tail.header("Content-Range"), Some("bytes 990-999/1000"));
    let open_ended = serve_file(&provider, &path(&file), Some("bytes=995-"));
    assert_eq!(open_ended.body, &bytes[995..]);
}

#[test]
fn a_range_past_the_end_is_416_with_the_size() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.bin");
    fs::write(&file, numbered(10)).unwrap();
    let served = serve_file(&LocalProvider::new(), &path(&file), Some("bytes=50-60"));
    assert_eq!(served.status, 416);
    assert_eq!(served.header("Content-Range"), Some("bytes */10"));
}

#[test]
fn an_ignorable_range_serves_the_whole_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.bin");
    let bytes = numbered(10);
    fs::write(&file, &bytes).unwrap();
    let served = serve_file(&LocalProvider::new(), &path(&file), Some("bytes=0-1,4-5"));
    assert_eq!(served.status, 200);
    assert_eq!(served.body, bytes);
}

#[test]
fn a_huge_range_is_cut_to_one_chunk_and_the_next_one_continues() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("big.mkv");
    let len = MAX_CHUNK as usize + 1000;
    let bytes = numbered(len);
    fs::write(&file, &bytes).unwrap();
    let provider = LocalProvider::new();
    let first = serve_file(&provider, &path(&file), Some("bytes=0-"));
    assert_eq!(first.status, 206);
    assert_eq!(first.body.len() as u64, MAX_CHUNK);
    assert_eq!(
        first.header("Content-Range"),
        Some(format!("bytes 0-{}/{len}", MAX_CHUNK - 1).as_str())
    );
    let rest = serve_file(
        &provider,
        &path(&file),
        Some(&format!("bytes={MAX_CHUNK}-")),
    );
    assert_eq!(rest.body, &bytes[MAX_CHUNK as usize..]);
}

#[test]
fn an_empty_file_is_served_empty_and_any_range_of_it_is_416() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("empty.txt");
    fs::write(&file, b"").unwrap();
    let provider = LocalProvider::new();
    let served = serve_file(&provider, &path(&file), None);
    assert_eq!(served.status, 200);
    assert!(served.body.is_empty());
    assert_eq!(
        serve_file(&provider, &path(&file), Some("bytes=0-")).status,
        416
    );
}

#[test]
fn a_script_capable_type_is_sandboxed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("page.html");
    fs::write(&file, "<script>alert(1)</script>").unwrap();
    let served = serve_file(&LocalProvider::new(), &path(&file), None);
    assert_eq!(served.header("Content-Security-Policy"), Some("sandbox"));
}

#[test]
fn a_folder_and_a_missing_file_are_404_and_say_nothing_about_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let provider = LocalProvider::new();
    let folder = serve_file(&provider, &path(dir.path()), None);
    assert_eq!(folder.status, 404);
    let missing = serve_file(&provider, &path(&dir.path().join("gone")), None);
    assert_eq!(missing.status, 404);
    let text = String::from_utf8(missing.body).unwrap();
    assert!(!text.contains("gone"));
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_file_is_followed_and_a_pipe_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("real.txt"), "hello").unwrap();
    std::os::unix::fs::symlink("real.txt", dir.path().join("link.txt")).unwrap();
    let provider = LocalProvider::new();
    let served = serve_file(&provider, &path(&dir.path().join("link.txt")), None);
    assert_eq!(served.status, 200);
    assert_eq!(served.body, b"hello");
    let fifo = dir.path().join("pipe");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    // SAFETY: `c` is a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    assert_eq!(serve_file(&provider, &path(&fifo), None).status, 415);
}

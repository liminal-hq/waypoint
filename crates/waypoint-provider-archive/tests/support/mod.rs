// Fixtures and helpers the archive provider's tests share.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use tempfile::TempDir;
use waypoint_path::{ArchivePath, FilePath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_provider_archive::{ArchiveOptions, ArchiveProvider, ContainerSource};
use waypoint_vfs::{CancelToken, LocalProvider, Provider};

/// A scratch folder on a real disk: `WAYPOINT_TEST_TMP` when set (the tmpfs under `/tmp` is small),
/// else the system's.
pub fn scratch() -> TempDir {
    let mut builder = tempfile::Builder::new();
    builder.prefix("waypoint-archive-test-");
    match std::env::var_os("WAYPOINT_TEST_TMP") {
        Some(dir) => builder.tempdir_in(dir).unwrap(),
        None => builder.tempdir().unwrap(),
    }
}

/// Finds archive files on the local disk.
pub struct Local;

impl ContainerSource for Local {
    fn provider_for(&self, container: &VfsPath) -> Result<Arc<dyn Provider>, VfsError> {
        match container {
            VfsPath::File(_) => Ok(Arc::new(LocalProvider::new())),
            other => Err(VfsError::Unsupported {
                what: other.scheme().to_owned(),
            }),
        }
    }
}

pub fn provider() -> Arc<ArchiveProvider> {
    ArchiveProvider::new(Arc::new(Local), ArchiveOptions::default())
}

pub fn provider_with(options: ArchiveOptions) -> Arc<ArchiveProvider> {
    ArchiveProvider::new(Arc::new(Local), options)
}

pub fn file(path: &Path) -> VfsPath {
    VfsPath::File(FilePath::from_path(path).unwrap())
}

/// The top of the archive file at `path`.
pub fn top(path: &Path) -> VfsPath {
    VfsPath::Archive(ArchivePath::new(file(path)).unwrap())
}

/// `base` and the `/`-separated names below it.
pub fn at(base: &VfsPath, rel: &str) -> VfsPath {
    rel.split('/')
        .filter(|part| !part.is_empty())
        .fold(base.clone(), |path, part| path.join(part).unwrap())
}

pub fn names(provider: &dyn Provider, path: &VfsPath) -> BTreeSet<String> {
    provider
        .list(path, &CancelToken::new(), usize::MAX, &mut |_| {})
        .unwrap_or_else(|e| panic!("list {}: {e:?}", path.display()))
        .into_iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .collect()
}

pub fn read(provider: &dyn Provider, path: &VfsPath) -> Vec<u8> {
    let mut out = Vec::new();
    provider
        .open_read(path)
        .unwrap_or_else(|e| panic!("read {}: {e:?}", path.display()))
        .read_to_end(&mut out)
        .unwrap();
    out
}

pub fn read_err(provider: &dyn Provider, path: &VfsPath) -> VfsError {
    match provider.open_read(path) {
        Err(error) => error,
        Ok(mut stream) => {
            let mut out = Vec::new();
            let error = stream
                .read_to_end(&mut out)
                .expect_err("the read should fail");
            waypoint_vfs::from_io(&error, &path.to_location())
        }
    }
}

pub fn kind<T>(result: &Result<T, VfsError>) -> String {
    waypoint_vfs::conformance::kind(result)
}

/// Whether a command-line tool is installed. With `WAYPOINT_ARCHIVE_REQUIRE` set (CI does), a
/// missing tool fails the test instead of skipping it. The tests that read archives made by `tar`,
/// `zip` and `7z` assume the GNU and Info-ZIP tools, so on Windows (where `tar` is bsdtar and the
/// others are rarely there) they always skip, and the Windows jobs run the pure tests.
pub fn have(tool: &str) -> bool {
    if cfg!(windows) {
        return false;
    }
    let found = Command::new(tool)
        .arg("--help")
        .output()
        .map(|out| out.status.success() || !out.stderr.is_empty() || !out.stdout.is_empty())
        .unwrap_or(false);
    assert!(
        found || std::env::var_os("WAYPOINT_ARCHIVE_REQUIRE").is_none(),
        "the `{tool}` tool is required here and is not installed"
    );
    found
}

/// Runs a tool, panicking with its output when it fails.
pub fn run(dir: &Path, program: &str, args: &[&str]) {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("cannot run {program}: {e}"));
    assert!(
        out.status.success(),
        "{program} {args:?} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A source folder with a few files, folders, an empty file and a symlink, for the tools to pack.
pub fn sample_tree(root: &Path) {
    std::fs::create_dir_all(root.join("docs/deep")).unwrap();
    std::fs::create_dir_all(root.join("empty dir")).unwrap();
    std::fs::write(root.join("hello.txt"), b"hello, archive\n").unwrap();
    std::fs::write(root.join("docs/readme.md"), b"# Readme\n").unwrap();
    std::fs::write(
        root.join("docs/deep/caf\u{e9} \u{65e5}\u{672c}.txt"),
        "unicode name",
    )
    .unwrap();
    std::fs::write(root.join("empty.bin"), b"").unwrap();
    let big: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
    std::fs::write(root.join("docs/big.bin"), &big).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("hello.txt", root.join("link-to-hello")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.join("hello.txt"),
            std::fs::Permissions::from_mode(0o640),
        )
        .unwrap();
    }
}

pub fn big_bytes() -> Vec<u8> {
    (0..200_000u32).map(|n| (n % 251) as u8).collect()
}

/// A zip archive written by hand, so a test controls every byte of the names and flags.
#[derive(Default)]
pub struct RawZip {
    entries: Vec<RawEntry>,
    prefix: Vec<u8>,
    comment: Vec<u8>,
}

struct RawEntry {
    name: Vec<u8>,
    data: Vec<u8>,
    flags: u16,
    external: u32,
    made_by_host: u8,
}

impl RawZip {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bytes in front of the archive, as a self-extractor has.
    pub fn prefix(mut self, bytes: &[u8]) -> Self {
        self.prefix = bytes.to_vec();
        self
    }

    pub fn comment(mut self, text: &str) -> Self {
        self.comment = text.as_bytes().to_vec();
        self
    }

    /// A stored (uncompressed) entry from a Unix host with these mode bits.
    pub fn unix(mut self, name: &[u8], data: &[u8], mode: u32) -> Self {
        self.entries.push(RawEntry {
            name: name.to_vec(),
            data: data.to_vec(),
            flags: 0x800,
            external: mode << 16,
            made_by_host: 3,
        });
        self
    }

    /// A stored entry with a name in code page 437 (no UTF-8 flag), from an MS-DOS host.
    pub fn dos(mut self, name: &[u8], data: &[u8]) -> Self {
        self.entries.push(RawEntry {
            name: name.to_vec(),
            data: data.to_vec(),
            flags: 0,
            external: 0,
            made_by_host: 0,
        });
        self
    }

    pub fn file(self, name: &str, data: &[u8]) -> Self {
        self.unix(name.as_bytes(), data, 0o100_644)
    }

    pub fn build(&self) -> Vec<u8> {
        let mut out = self.prefix.clone();
        let base = out.len();
        let mut central = Vec::new();
        for entry in &self.entries {
            let offset = (out.len() - base) as u32;
            let mut crc = flate2::Crc::new();
            crc.update(&entry.data);
            // Local header.
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&entry.flags.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // stored
            out.extend_from_slice(&0x6000u16.to_le_bytes()); // time 12:00:00
            out.extend_from_slice(&0x5821u16.to_le_bytes()); // date 2024-01-01
            out.extend_from_slice(&crc.sum().to_le_bytes());
            out.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&entry.name);
            out.extend_from_slice(&entry.data);
            // Central header.
            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&((u16::from(entry.made_by_host) << 8) | 20).to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&entry.flags.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0x6000u16.to_le_bytes());
            central.extend_from_slice(&0x5821u16.to_le_bytes());
            central.extend_from_slice(&crc.sum().to_le_bytes());
            central.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(entry.data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&entry.external.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(&entry.name);
        }
        let directory_at = (out.len() - base) as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(self.entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&directory_at.to_le_bytes());
        out.extend_from_slice(&(self.comment.len() as u16).to_le_bytes());
        out.extend_from_slice(&self.comment);
        out
    }

    pub fn write(&self, path: &Path) -> PathBuf {
        std::fs::write(path, self.build()).unwrap();
        path.to_path_buf()
    }
}

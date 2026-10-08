// The helper program, started as the current user and driven over its real standard input and output.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use waypoint_elevated::{ElevatedProvider, Launcher, Transport};
use waypoint_path::{ConnectionKey, FilePath, VfsPath};
use waypoint_protocol::VfsError;
use waypoint_vfs::{CancelToken, Provider, WriteOptions};

/// Starts the helper binary and hands its standard streams over as the transport. The child is kept
/// so the test can see how it ended.
struct ChildLauncher {
    child: Arc<Mutex<Option<Child>>>,
}

impl Launcher for ChildLauncher {
    fn launch(&self) -> Result<Transport, VfsError> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_waypoint-elevate-helper"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| VfsError::Io {
                message: error.to_string(),
                location: None,
            })?;
        let writer = child.stdin.take().unwrap();
        let reader = child.stdout.take().unwrap();
        *self.child.lock().unwrap() = Some(child);
        Ok(Transport {
            reader: Box::new(reader),
            writer: Box::new(writer),
        })
    }
}

fn admin(dir: &std::path::Path) -> VfsPath {
    VfsPath::File(FilePath::from_path(dir).unwrap())
        .elevated()
        .unwrap()
}

#[test]
fn the_helper_serves_over_its_standard_streams() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("seen.txt"), b"hello").unwrap();
    let child = Arc::new(Mutex::new(None));
    let client = ElevatedProvider::new(Box::new(ChildLauncher {
        child: child.clone(),
    }));
    let key = ConnectionKey::elevated();
    client.connect(&key, None, &CancelToken::new()).unwrap();
    let root = admin(dir.path());

    // A list.
    let entries = client
        .list(&root, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "seen.txt");

    // Create, rename and remove.
    let folder = root.join("made").unwrap();
    client.create_dir(&folder).unwrap();
    client.create_file(&folder.join("a").unwrap()).unwrap();
    client
        .rename(
            &folder.join("a").unwrap(),
            &folder.join("b").unwrap(),
            false,
        )
        .unwrap();
    assert!(dir.path().join("made/b").exists() && !dir.path().join("made/a").exists());
    client.remove_file(&folder.join("b").unwrap()).unwrap();
    client.remove_dir(&folder).unwrap();
    assert!(!dir.path().join("made").exists());

    // A streamed write and read of a file of several chunks.
    let big = root.join("big.bin").unwrap();
    let content: Vec<u8> = (0..1_000_000u32).map(|n| (n % 253) as u8).collect();
    let mut stream = client
        .create_write(&big, WriteOptions::exclusive())
        .unwrap();
    stream.write_all(&content).unwrap();
    stream.finish(false).unwrap();
    assert_eq!(std::fs::read(dir.path().join("big.bin")).unwrap(), content);
    let mut back = Vec::new();
    client
        .open_read(&big)
        .unwrap()
        .read_to_end(&mut back)
        .unwrap();
    assert_eq!(back, content);

    // An error crosses the process boundary typed, and as `admin:`.
    match client.stat(&root.join("missing").unwrap()).unwrap_err() {
        VfsError::NotFound { location } => assert!(location.uri.starts_with("admin:///")),
        other => panic!("{other:?}"),
    }

    // Closing the input ends the helper, quietly and successfully.
    client.disconnect(&key);
    let mut child = child.lock().unwrap().take().unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the helper did not exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "{status:?}");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stderr.is_empty(), "{stderr:?}");
}

#[test]
fn the_helper_exits_with_its_own_code_on_a_protocol_error() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_waypoint-elevate-helper"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&[0, 0, 0, 2, 9, 0]).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("protocol error"), "{stderr}");
    drop(stdin);
}

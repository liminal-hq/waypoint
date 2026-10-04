// Watching any provider by polling: re-list a folder on an interval and diff it by name, for a
// connection that asks to refresh every N seconds (A83).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;

use crate::provider::{Provider, ScannedEntry, Watch, WatchEvent, WatchSink};
use crate::watch::diff_snapshots;
use crate::CancelToken;

/// How finely the polling thread checks that it should stop.
const TICK: Duration = Duration::from_millis(50);

/// A watch that re-lists a folder through its provider every `interval` and reports the
/// difference. Dropping it stops the thread (within one tick, or when a listing in flight ends,
/// which its cancel token cuts short).
pub struct PollWatch {
    stop: Arc<AtomicBool>,
    cancel: CancelToken,
    thread: Option<JoinHandle<()>>,
}

impl Watch for PollWatch {}

impl Drop for PollWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.cancel.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn snapshot(
    provider: &dyn Provider,
    path: &VfsPath,
    cancel: &CancelToken,
) -> Result<HashMap<OsString, ScannedEntry>, VfsError> {
    let entries = provider.list(path, cancel, usize::MAX, &mut |_| {})?;
    Ok(entries.into_iter().map(|e| (e.name.clone(), e)).collect())
}

/// Whether a failed poll means the folder is gone for good, rather than for now. A connection
/// error is reported as lost too, so the listing shows the disconnected state; reconnecting opens
/// a new listing and a new watch.
fn is_loss(error: &VfsError) -> bool {
    !matches!(
        error,
        VfsError::Timeout { .. } | VfsError::RateLimited { .. } | VfsError::Io { .. }
    )
}

impl PollWatch {
    /// Takes a first snapshot (an error here is returned, as a watch that cannot start) and then
    /// polls on a thread of its own.
    pub fn start(
        provider: Arc<dyn Provider>,
        path: VfsPath,
        interval: Duration,
        sink: WatchSink,
    ) -> Result<Self, VfsError> {
        let cancel = CancelToken::new();
        let mut before = snapshot(provider.as_ref(), &path, &cancel)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let token = cancel.clone();
        let thread = thread::Builder::new()
            .name("waypoint-poll-watch".to_owned())
            .spawn(move || {
                let mut next = Instant::now() + interval;
                loop {
                    if flag.load(Ordering::Relaxed) {
                        return;
                    }
                    let now = Instant::now();
                    if now < next {
                        thread::sleep((next - now).min(TICK));
                        continue;
                    }
                    next = now + interval;
                    let taken = snapshot(provider.as_ref(), &path, &token);
                    if flag.load(Ordering::Relaxed) {
                        return;
                    }
                    match taken {
                        Ok(after) => {
                            let changes = diff_snapshots(&before, &after);
                            before = after;
                            if !changes.is_empty() {
                                sink(WatchEvent::Changes(changes));
                            }
                        }
                        Err(VfsError::Cancelled) => return,
                        Err(error) if is_loss(&error) => return sink(WatchEvent::Lost(error)),
                        // A slow or busy server: try again at the next interval.
                        Err(_) => {}
                    }
                }
            })
            .map_err(|error| VfsError::Io {
                message: format!("could not start polling: {error}"),
                location: None,
            })?;
        Ok(Self {
            stop,
            cancel,
            thread: Some(thread),
        })
    }
}

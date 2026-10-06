// Watching an archive: the file that holds it is checked now and then, and when it has changed (an
// edit rewrote it, or another program did) the listing is asked to read it again.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{Provider, RescanReason, Watch, WatchEvent, WatchSink};

/// How often the archive file is looked at.
const INTERVAL: Duration = Duration::from_millis(1500);
/// How finely the thread checks that it should stop.
const TICK: Duration = Duration::from_millis(50);

/// What the file looked like: its size and time.
type Signature = (u64, Option<i64>);

fn signature(provider: &dyn Provider, container: &VfsPath) -> Result<Signature, VfsError> {
    let entry = provider.stat(container)?;
    Ok((entry.size.unwrap_or(0), entry.modified_ms))
}

/// Looks at the archive file on a thread of its own until dropped.
pub(crate) struct FileWatch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Watch for FileWatch {}

impl Drop for FileWatch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl FileWatch {
    /// Starts watching `container`, which is read through `provider`. An error is a file that
    /// cannot be seen at all.
    pub(crate) fn start(
        provider: Arc<dyn Provider>,
        container: VfsPath,
        sink: WatchSink,
    ) -> Result<Self, VfsError> {
        let mut seen = signature(provider.as_ref(), &container)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = thread::Builder::new()
            .name("waypoint-archive-watch".to_owned())
            .spawn(move || {
                let mut next = Instant::now() + INTERVAL;
                loop {
                    if flag.load(Ordering::Relaxed) {
                        return;
                    }
                    let now = Instant::now();
                    if now < next {
                        thread::sleep((next - now).min(TICK));
                        continue;
                    }
                    next = now + INTERVAL;
                    match signature(provider.as_ref(), &container) {
                        Ok(now) if now != seen => {
                            seen = now;
                            if flag.load(Ordering::Relaxed) {
                                return;
                            }
                            sink(WatchEvent::Rescan(RescanReason::Unknown(
                                "the archive file changed".to_owned(),
                            )));
                        }
                        Ok(_) => {}
                        Err(
                            error @ (VfsError::NotFound { .. } | VfsError::PermissionDenied { .. }),
                        ) => {
                            return sink(WatchEvent::Lost(error));
                        }
                        // A server that is busy or away: look again at the next time.
                        Err(_) => {}
                    }
                }
            })
            .map_err(|error| VfsError::Io {
                message: format!("could not start watching the archive: {error}"),
                location: None,
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

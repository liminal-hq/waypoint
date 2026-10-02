// Measures free space with a timeout, so a stalled mount never blocks a list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::Instant;

/// The size of a volume and what is free on it, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space {
    pub total: u64,
    pub free: u64,
}

/// Measures the volume mounted at a path, blocking until the system answers. It may block for ever on a dead network mount: the plugin only ever calls it on a thread of its own and gives up waiting after a timeout.
pub type SpaceFn = Arc<dyn Fn(&str) -> Option<Space> + Send + Sync>;

/// Runs the injected measurement for many mount points at once, within one shared deadline.
#[derive(Clone)]
pub struct Measurer {
    measure: SpaceFn,
    timeout: Duration,
    /// Mount points whose last measurement has not returned. A stalled mount keeps its thread until the system lets go, so the plugin does not start another for it on every refresh.
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl Measurer {
    pub fn new(measure: SpaceFn, timeout: Duration) -> Self {
        Measurer {
            measure,
            timeout,
            in_flight: Arc::default(),
        }
    }

    /// Measures every mount point in parallel and returns what answered before the timeout. The call takes about the timeout at most, however many mounts stall.
    pub async fn measure_all(&self, mounts: Vec<String>) -> HashMap<String, Space> {
        let deadline = Instant::now() + self.timeout;
        let mut pending = Vec::new();
        for mount in mounts {
            if let Some(receiver) = self.start(&mount) {
                pending.push((mount, receiver));
            }
        }
        let mut answers = HashMap::new();
        for (mount, receiver) in pending {
            if let Ok(Ok(Some(space))) = tokio::time::timeout_at(deadline, receiver).await {
                answers.insert(mount, space);
            }
        }
        answers
    }

    /// Starts one measurement on its own thread, or `None` when the previous one for this mount is still stuck or no thread could be made.
    fn start(&self, mount: &str) -> Option<oneshot::Receiver<Option<Space>>> {
        if !self.in_flight.lock().ok()?.insert(mount.to_string()) {
            return None;
        }
        let (sender, receiver) = oneshot::channel();
        let measure = Arc::clone(&self.measure);
        let in_flight = Arc::clone(&self.in_flight);
        let path = mount.to_string();
        let spawned = std::thread::Builder::new()
            .name("volumes-space".into())
            .spawn(move || {
                let space = measure(&path);
                if let Ok(mut set) = in_flight.lock() {
                    set.remove(&path);
                }
                // The receiver is gone when the wait timed out.
                let _ = sender.send(space);
            });
        if spawned.is_err() {
            if let Ok(mut set) = self.in_flight.lock() {
                set.remove(mount);
            }
            return None;
        }
        Some(receiver)
    }
}

/// The real measurement: what an ordinary user may use, which is not what root could.
#[cfg(unix)]
#[allow(clippy::useless_conversion, clippy::unnecessary_cast)]
pub fn system_space(mount: &str) -> Option<Space> {
    use std::ffi::CString;

    let path = CString::new(mount).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: `path` is a valid NUL-terminated string and `stat` points at writable memory of the right type; `statvfs` fully initialises it when it returns 0.
    let status = unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if status != 0 {
        return None;
    }
    // SAFETY: the call succeeded, so the structure is initialised.
    let stat = unsafe { stat.assume_init() };
    let unit = u64::from(stat.f_frsize);
    let total = (stat.f_blocks as u64).saturating_mul(unit);
    if total == 0 {
        // A virtual file system reports no blocks; there is nothing to show.
        return None;
    }
    Some(Space {
        total,
        free: (stat.f_bavail as u64).saturating_mul(unit),
    })
}

#[cfg(windows)]
pub fn system_space(mount: &str) -> Option<Space> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = mount.encode_utf16().chain(std::iter::once(0)).collect();
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` is NUL-terminated and outlives the call; the out pointers are valid.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(wide.as_ptr()),
            Some(&mut available),
            Some(&mut total),
            Some(&mut free),
        )
    }
    .ok()?;
    Some(Space {
        total,
        free: available,
    })
}

#[cfg(not(any(unix, windows)))]
pub fn system_space(_mount: &str) -> Option<Space> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant as StdInstant;

    fn measurer(
        timeout_ms: u64,
        f: impl Fn(&str) -> Option<Space> + Send + Sync + 'static,
    ) -> Measurer {
        Measurer::new(Arc::new(f), Duration::from_millis(timeout_ms))
    }

    #[tokio::test]
    async fn answers_within_the_timeout_are_returned() {
        let m = measurer(1000, |mount| {
            Some(Space {
                total: mount.len() as u64,
                free: 1,
            })
        });
        let answers = m.measure_all(vec!["/a".into(), "/bb".into()]).await;
        assert_eq!(answers["/a"].total, 2);
        assert_eq!(answers["/bb"].total, 3);
    }

    #[tokio::test]
    async fn a_stalled_mount_gives_no_answer_after_about_the_timeout() {
        let m = measurer(150, |mount| {
            if mount == "/stalled" {
                std::thread::sleep(Duration::from_secs(3));
            }
            Some(Space { total: 10, free: 5 })
        });
        let started = StdInstant::now();
        let answers = m.measure_all(vec!["/ok".into(), "/stalled".into()]).await;
        let took = started.elapsed();
        assert!(answers.contains_key("/ok"));
        assert!(!answers.contains_key("/stalled"));
        assert!(took >= Duration::from_millis(140), "{took:?}");
        assert!(took < Duration::from_millis(1500), "{took:?}");
    }

    #[tokio::test]
    async fn many_stalled_mounts_share_one_deadline() {
        let m = measurer(150, |_| {
            std::thread::sleep(Duration::from_secs(2));
            None
        });
        let started = StdInstant::now();
        let mounts = (0..8).map(|n| format!("/m{n}")).collect();
        let answers = m.measure_all(mounts).await;
        assert!(answers.is_empty());
        assert!(started.elapsed() < Duration::from_millis(1000));
    }

    #[tokio::test]
    async fn a_mount_still_stuck_is_not_measured_again() {
        let calls = Arc::new(Mutex::new(0u32));
        let counter = Arc::clone(&calls);
        let m = measurer(50, move |_| {
            *counter.lock().unwrap() += 1;
            std::thread::sleep(Duration::from_millis(600));
            None
        });
        m.measure_all(vec!["/stuck".into()]).await;
        m.measure_all(vec!["/stuck".into()]).await;
        assert_eq!(*calls.lock().unwrap(), 1);
        // Once the stuck call returns the mount is measured again.
        tokio::time::sleep(Duration::from_millis(700)).await;
        m.measure_all(vec!["/stuck".into()]).await;
        assert_eq!(*calls.lock().unwrap(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn a_real_folder_reports_space() {
        let dir = tempfile::tempdir().unwrap();
        let space = system_space(dir.path().to_str().unwrap())
            .expect("a temp folder's volume reports space");
        assert!(space.total > 0 && space.free <= space.total);
        assert_eq!(system_space("/no/such/folder/anywhere"), None);
    }
}

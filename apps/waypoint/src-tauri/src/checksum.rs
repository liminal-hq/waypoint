// The Properties window's checksum: a file's hash on request, run on a thread of its own and cancellable
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::ipc::Channel;
use tauri::{Manager, Runtime, State, Window};
use tauri_plugin_waypoint_vfs::Vfs;
use waypoint_ops::{run_checksum, ChecksumEvent, VerifyAlgorithm};
use waypoint_path::VfsPath;
use waypoint_protocol::{EntryId, VfsError};
use waypoint_vfs::{CancelToken, ListingHandle, LocalProvider, SelectionSpec};

/// The runs in progress, by id, with the window that started each: a window may only cancel its own.
#[derive(Default)]
pub struct Checksums {
    next: AtomicU64,
    runs: Mutex<HashMap<u64, (String, CancelToken)>>,
}

impl Checksums {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, (String, CancelToken)>> {
        self.runs.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn start(&self, window: &str) -> (u64, CancelToken) {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = CancelToken::new();
        self.lock().insert(id, (window.to_owned(), cancel.clone()));
        (id, cancel)
    }

    fn finish(&self, id: u64) {
        self.lock().remove(&id);
    }

    /// Stops one of `window`'s runs; one that has ended, or another window's, is left alone.
    pub fn cancel(&self, window: &str, id: u64) {
        if let Some((owner, cancel)) = self.lock().get(&id) {
            if owner == window {
                cancel.cancel();
            }
        }
    }

    /// Stops every run `window` started, as it closes.
    pub fn cancel_window(&self, window: &str) {
        for (owner, cancel) in self.lock().values() {
            if owner == window {
                cancel.cancel();
            }
        }
    }
}

/// Starts hashing entry `id` of the calling window's listing `handle`, on a thread of its own, and
/// returns the run's id at once. `on_event` gets `progress` and then exactly one `done`,
/// `cancelled` or `failed`. Only local files are hashed; the digest is of the whole file.
#[tauri::command]
pub async fn file_checksum<R: Runtime>(
    window: Window<R>,
    vfs: State<'_, Vfs>,
    runs: State<'_, Checksums>,
    handle: ListingHandle,
    id: EntryId,
    algorithm: VerifyAlgorithm,
    on_event: Channel<ChecksumEvent>,
) -> Result<u64, VfsError> {
    let locations = vfs.resolve_selection(
        window.label(),
        handle,
        &SelectionSpec::Chosen { ids: vec![id] },
    )?;
    let location = locations.into_iter().next().ok_or(VfsError::StaleHandle)?;
    let path = match VfsPath::from_location(&location) {
        Ok(path @ VfsPath::File(_)) => path,
        _ => {
            return Err(VfsError::Unsupported {
                what: "a checksum of something that is not a local file".to_owned(),
            })
        }
    };
    let (job, cancel) = runs.start(window.label());
    let finished = window.app_handle().clone();
    let spawned = std::thread::Builder::new()
        .name("waypoint-checksum".to_owned())
        .spawn(move || {
            waypoint_vfs::lower_thread_priority();
            let stop = cancel.clone();
            run_checksum(
                &LocalProvider::new(),
                &path,
                algorithm,
                &cancel,
                &mut |event| {
                    // Nobody is listening any more (the page went away): stop reading.
                    if on_event.send(event).is_err() {
                        stop.cancel();
                    }
                },
            );
            if let Some(runs) = finished.try_state::<Checksums>() {
                runs.finish(job);
            }
        });
    if let Err(error) = spawned {
        runs.finish(job);
        return Err(VfsError::Io {
            message: format!("could not start the checksum: {error}"),
            location: None,
        });
    }
    Ok(job)
}

/// Stops one of the calling window's checksum runs. A run that has ended is not an error.
#[tauri::command]
pub fn cancel_checksum<R: Runtime>(window: Window<R>, runs: State<'_, Checksums>, job: u64) {
    runs.cancel(window.label(), job);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_cancels_only_its_own_runs() {
        let runs = Checksums::default();
        let (mine, mine_token) = runs.start("properties-1");
        let (theirs, their_token) = runs.start("properties-2");
        runs.cancel("properties-1", theirs);
        assert!(!their_token.is_cancelled());
        runs.cancel("properties-1", mine);
        assert!(mine_token.is_cancelled());
    }

    #[test]
    fn closing_a_window_cancels_its_runs_and_leaves_the_others() {
        let runs = Checksums::default();
        let (_, one) = runs.start("properties-1");
        let (_, two) = runs.start("properties-2");
        runs.cancel_window("properties-1");
        assert!(one.is_cancelled());
        assert!(!two.is_cancelled());
    }

    #[test]
    fn a_finished_run_cannot_be_cancelled_again() {
        let runs = Checksums::default();
        let (id, token) = runs.start("properties-1");
        runs.finish(id);
        runs.cancel("properties-1", id);
        assert!(!token.is_cancelled());
    }
}

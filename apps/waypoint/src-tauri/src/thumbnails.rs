// Composes the thumbnails plugin: turns what a window shows into paths, so the page never sends one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `tauri-plugin-thumbnails` knows files by path and calls no other plugin (A4). A window knows its
// files as `(ListingHandle, EntryId)` or as a `Location`, which is what its page sends here; this
// module resolves each to a local path through the file system plugin (only the window that opened
// a listing can resolve it), reads the modified time the cache checks, and queues the batch with
// the plugin. The plugin's keys are made from the path, the size and the time, so two windows (or
// two listings) that each number their entries from zero never share a job; the events are turned
// back into the page's own keys before they cross the channel. Cancelling and reprioritising go
// through here too, because the page only knows its own keys. The size cap in the settings is kept
// in force on the plugin (`wire`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime, State, Window, Wry};
use tauri_plugin_thumbnails::{
    SkipWhy, ThumbEvent, ThumbRequest, ThumbSize, Thumbnails, ThumbnailsExt, Ticket,
};
use tauri_plugin_waypoint_settings::SettingsStore;
use tauri_plugin_waypoint_vfs::Vfs;
use waypoint_path::VfsPath;
use waypoint_protocol::{EntryId, Location};
use waypoint_settings::Settings;
use waypoint_vfs::ListingHandle;

/// The queue behind the bridge. The plugin's `Thumbnails` is one; a test supplies its own.
pub trait Queue: Send + Sync {
    fn request(&self, items: Vec<ThumbRequest>, sink: tauri_plugin_thumbnails::Sink) -> Ticket;
    fn cancel(&self, ticket: Ticket) -> bool;
    fn prioritise(&self, ticket: Ticket, keys: &[String]);
}

impl Queue for Thumbnails {
    fn request(&self, items: Vec<ThumbRequest>, sink: tauri_plugin_thumbnails::Sink) -> Ticket {
        Thumbnails::request(self, items, sink)
    }

    fn cancel(&self, ticket: Ticket) -> bool {
        Thumbnails::cancel(self, ticket)
    }

    fn prioritise(&self, ticket: Ticket, keys: &[String]) {
        Thumbnails::prioritise(self, ticket, keys)
    }
}

/// The page's keys for one ticket, and the plugin's keys they stand for.
struct TicketKeys {
    /// The plugin's key to every page key that asked for it (two rows can be one file).
    pages: Mutex<HashMap<String, Vec<String>>>,
    /// Plugin keys that have not reported yet.
    remaining: AtomicUsize,
}

impl TicketKeys {
    fn new(pages: HashMap<String, Vec<String>>) -> Self {
        Self {
            remaining: AtomicUsize::new(pages.len()),
            pages: Mutex::new(pages),
        }
    }

    /// The events for the page keys one plugin event stands for, and whether it was the last pending one.
    fn translate(&self, event: ThumbEvent) -> (Vec<ThumbEvent>, bool) {
        let keys = self
            .pages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(event.key())
            .unwrap_or_default();
        if keys.is_empty() {
            return (Vec::new(), false);
        }
        let last = self.remaining.fetch_sub(1, Ordering::SeqCst) == 1;
        let events = keys
            .into_iter()
            .map(|key| match &event {
                ThumbEvent::Ready { url, .. } => ThumbEvent::Ready {
                    key,
                    url: url.clone(),
                },
                ThumbEvent::Failed { reason, .. } => ThumbEvent::Failed {
                    key,
                    reason: reason.clone(),
                },
                ThumbEvent::Skipped { why, .. } => ThumbEvent::Skipped { key, why: *why },
            })
            .collect();
        (events, last)
    }

    /// The plugin's keys for the page's, in the order given, for those still pending.
    fn plugin_keys(&self, page_keys: &[String]) -> Vec<String> {
        let pages = self.pages.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        for wanted in page_keys {
            if let Some((plugin_key, _)) = pages.iter().find(|(_, keys)| keys.contains(wanted)) {
                if !out.contains(plugin_key) {
                    out.push(plugin_key.clone());
                }
            }
        }
        out
    }

    fn finished(&self) -> bool {
        self.remaining.load(Ordering::SeqCst) == 0
    }
}

/// The batches in flight, by the ticket the plugin gave them.
#[derive(Default, Clone)]
pub struct ThumbnailBridge {
    tickets: Arc<Mutex<HashMap<Ticket, Arc<TicketKeys>>>>,
}

/// One thing the page wants a thumbnail of, resolved as far as this window can: a path, or why not.
pub struct Wanted {
    pub key: String,
    pub path: Result<PathBuf, String>,
}

/// The plugin's key for a file at a size and time: another file, size or time is another job.
fn plugin_key(path: &Path, size: ThumbSize, mtime_ms: i64) -> String {
    format!("{}|{}|{}", path.display(), size.dir_name(), mtime_ms)
}

/// The file's modified time in milliseconds since the Unix epoch; 0 where the system does not say.
fn modified_ms(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_millis().try_into().unwrap_or(i64::MAX))
}

impl ThumbnailBridge {
    /// Queues thumbnails for `wanted` and sends each result to `send`, under the page's keys. What
    /// cannot be asked for (not a local path, gone, a folder) is answered at once.
    pub fn submit(
        &self,
        queue: &dyn Queue,
        wanted: Vec<Wanted>,
        size: ThumbSize,
        send: Arc<dyn Fn(ThumbEvent) + Send + Sync>,
    ) -> Ticket {
        let mut requests = Vec::new();
        let mut pages: HashMap<String, Vec<String>> = HashMap::new();
        for item in wanted {
            let outcome = item.path.and_then(|path| {
                let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                Ok((path, metadata))
            });
            match outcome {
                Err(reason) => send(ThumbEvent::Failed {
                    key: item.key,
                    reason,
                }),
                Ok((_, metadata)) if metadata.is_dir() => send(ThumbEvent::Skipped {
                    key: item.key,
                    why: SkipWhy::Unsupported,
                }),
                Ok((path, metadata)) => {
                    let mtime_ms = modified_ms(&metadata);
                    let key = plugin_key(&path, size, mtime_ms);
                    let slot = pages.entry(key.clone()).or_default();
                    if slot.is_empty() {
                        requests.push(ThumbRequest {
                            key,
                            path: path.to_string_lossy().into_owned(),
                            size,
                            mtime_ms,
                        });
                    }
                    slot.push(item.key);
                }
            }
        }
        let keys = Arc::new(TicketKeys::new(pages));
        let ticket_cell: Arc<OnceLock<Ticket>> = Arc::new(OnceLock::new());
        let sink: tauri_plugin_thumbnails::Sink = {
            let keys = Arc::clone(&keys);
            let ticket_cell = Arc::clone(&ticket_cell);
            let bridge = self.clone();
            Arc::new(move |event| {
                let (events, last) = keys.translate(event);
                for event in events {
                    send(event);
                }
                if last {
                    if let Some(ticket) = ticket_cell.get() {
                        bridge.forget(*ticket);
                    }
                }
            })
        };
        let ticket = queue.request(requests, sink);
        let _ = ticket_cell.set(ticket);
        self.tickets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(ticket, Arc::clone(&keys));
        // Everything may have been answered before the ticket was known.
        if keys.finished() {
            self.forget(ticket);
        }
        ticket
    }

    /// Withdraws a batch.
    pub fn cancel(&self, queue: &dyn Queue, ticket: Ticket) -> bool {
        self.forget(ticket);
        queue.cancel(ticket)
    }

    /// Moves the pending items for the page's `keys` to the front of their batch.
    pub fn prioritise(&self, queue: &dyn Queue, ticket: Ticket, keys: &[String]) {
        let known = self
            .tickets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&ticket)
            .cloned();
        if let Some(known) = known {
            let mapped = known.plugin_keys(keys);
            if !mapped.is_empty() {
                queue.prioritise(ticket, &mapped);
            }
        }
    }

    fn forget(&self, ticket: Ticket) {
        self.tickets
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&ticket);
    }
}

// ---- resolving what the page names ----

/// The local path of a location, or why it has none here.
fn path_of_location(location: &Location) -> Result<PathBuf, String> {
    match VfsPath::from_location(location) {
        Ok(VfsPath::File(path)) => Ok(path.into_path_buf()),
        Ok(_) => Err("not a local file".to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

/// An entry of a listing the page shows, by the page's own key.
#[derive(Debug, Deserialize)]
pub struct EntryRef {
    pub key: String,
    pub id: EntryId,
}

/// A location the page shows (a Shelf item), by the page's own key.
#[derive(Debug, Deserialize)]
pub struct LocationRef {
    pub key: String,
    pub location: Location,
}

fn channel_sender(channel: Channel<ThumbEvent>) -> Arc<dyn Fn(ThumbEvent) + Send + Sync> {
    Arc::new(move |event| {
        let _ = channel.send(event);
    })
}

/// Queues thumbnails for entries of the window's own listing. The page names an entry by its id and
/// never sends a path: the listing resolves it, and only the window that opened the listing can.
#[tauri::command]
pub async fn thumbnails_request_entries<R: Runtime>(
    window: Window<R>,
    bridge: State<'_, ThumbnailBridge>,
    handle: ListingHandle,
    items: Vec<EntryRef>,
    size: ThumbSize,
    on_event: Channel<ThumbEvent>,
) -> Result<Ticket, String> {
    let app = window.app_handle().clone();
    let label = window.label().to_owned();
    // One lookup per entry: resolving them as a selection would walk the whole listing for each.
    let ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
    let located = match app.try_state::<Vfs>() {
        Some(vfs) => vfs
            .locate_entries(&label, handle, &ids)
            .map_err(|e| format!("{e:?}")),
        None => Err("listings are not available".to_owned()),
    };
    let wanted = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let path = match &located {
                Err(reason) => Err(reason.clone()),
                Ok(locations) => match locations.get(index) {
                    Some(Some(location)) => path_of_location(location),
                    _ => Err("no longer in the listing".to_owned()),
                },
            };
            Wanted {
                key: item.key,
                path,
            }
        })
        .collect::<Vec<_>>();
    Ok(submit_off_thread(&app, bridge.inner().clone(), wanted, size, on_event).await)
}

/// Queues thumbnails for locations the window shows (the Shelf's items).
#[tauri::command]
pub async fn thumbnails_request_locations<R: Runtime>(
    window: Window<R>,
    bridge: State<'_, ThumbnailBridge>,
    items: Vec<LocationRef>,
    size: ThumbSize,
    on_event: Channel<ThumbEvent>,
) -> Result<Ticket, String> {
    let app = window.app_handle().clone();
    let wanted = items
        .into_iter()
        .map(|item| Wanted {
            path: path_of_location(&item.location),
            key: item.key,
        })
        .collect();
    Ok(submit_off_thread(&app, bridge.inner().clone(), wanted, size, on_event).await)
}

/// Reading each file's modified time touches the disk, so it is done off the main thread.
async fn submit_off_thread<R: Runtime>(
    app: &AppHandle<R>,
    bridge: ThumbnailBridge,
    wanted: Vec<Wanted>,
    size: ThumbSize,
    on_event: Channel<ThumbEvent>,
) -> Ticket {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        bridge.submit(app.thumbnails(), wanted, size, channel_sender(on_event))
    })
    .await
    .unwrap_or(Ticket(0))
}

/// Withdraws a batch: nothing more is sent to it.
#[tauri::command]
pub fn thumbnails_cancel<R: Runtime>(
    app: AppHandle<R>,
    bridge: State<'_, ThumbnailBridge>,
    ticket: Ticket,
) -> bool {
    bridge.cancel(app.thumbnails(), ticket)
}

/// Moves a batch's pending items for the page's `keys` to the front.
#[tauri::command]
pub fn thumbnails_prioritise<R: Runtime>(
    app: AppHandle<R>,
    bridge: State<'_, ThumbnailBridge>,
    ticket: Ticket,
    keys: Vec<String>,
) {
    bridge.prioritise(app.thumbnails(), ticket, &keys);
}

// ---- the settings ----

const MEGABYTE: u64 = 1024 * 1024;

/// The largest file the built-in generator decodes under `settings`, in bytes.
pub fn max_file_bytes(settings: &Settings) -> u64 {
    u64::from(settings.previews.max_file_mb) * MEGABYTE
}

/// Keeps the plugin's size cap level with the settings, now and after every change.
pub fn wire(app: &AppHandle<Wry>) {
    let Some(thumbnails) = app.try_state::<Thumbnails>() else {
        return;
    };
    thumbnails.set_max_file_bytes(max_file_bytes(&crate::settings::current(app)));
    if let Some(store) = app.try_state::<SettingsStore<Wry>>() {
        let handle = app.clone();
        store.on_change(move |settings| {
            handle
                .thumbnails()
                .set_max_file_bytes(max_file_bytes(settings));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    /// A queue that records what it was given and answers on demand.
    #[derive(Default)]
    struct FakeQueue {
        next: AtomicU64,
        requests: Mutex<Vec<(Ticket, Vec<ThumbRequest>, tauri_plugin_thumbnails::Sink)>>,
        cancelled: Mutex<Vec<Ticket>>,
        prioritised: Mutex<Vec<(Ticket, Vec<String>)>>,
    }

    impl Queue for FakeQueue {
        fn request(&self, items: Vec<ThumbRequest>, sink: tauri_plugin_thumbnails::Sink) -> Ticket {
            let ticket = Ticket(self.next.fetch_add(1, Ordering::SeqCst) + 1);
            self.requests.lock().unwrap().push((ticket, items, sink));
            ticket
        }

        fn cancel(&self, ticket: Ticket) -> bool {
            self.cancelled.lock().unwrap().push(ticket);
            true
        }

        fn prioritise(&self, ticket: Ticket, keys: &[String]) {
            self.prioritised
                .lock()
                .unwrap()
                .push((ticket, keys.to_vec()));
        }
    }

    type Seen = Arc<Mutex<Vec<ThumbEvent>>>;

    fn collector() -> (Seen, Arc<dyn Fn(ThumbEvent) + Send + Sync>) {
        let seen: Seen = Arc::default();
        let sink = Arc::clone(&seen);
        (
            seen,
            Arc::new(move |event| sink.lock().unwrap().push(event)),
        )
    }

    fn wanted(key: &str, path: &Path) -> Wanted {
        Wanted {
            key: key.to_owned(),
            path: Ok(path.to_path_buf()),
        }
    }

    #[test]
    fn a_batch_is_queued_under_keys_of_its_own_and_answered_under_the_pages() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        let bridge = ThumbnailBridge::default();
        let queue = FakeQueue::default();
        let (seen, send) = collector();
        let ticket = bridge.submit(
            &queue,
            vec![wanted("1:0", &a), wanted("2:0", &b)],
            ThumbSize::Normal,
            send,
        );
        let requests = queue.requests.lock().unwrap();
        let (queued, items, sink) = &requests[0];
        assert_eq!(*queued, ticket);
        assert_eq!(items.len(), 2);
        // The plugin never sees the page's keys, and the page never sees a path.
        assert!(items
            .iter()
            .all(|item| item.key.contains("a.png|normal|") || item.key.contains("b.png|normal|")));
        assert_eq!(items[0].path, a.to_string_lossy());
        sink(ThumbEvent::Ready {
            key: items[1].key.clone(),
            url: "thumb://localhost/normal/x.png".to_owned(),
        });
        assert_eq!(
            *seen.lock().unwrap(),
            vec![ThumbEvent::Ready {
                key: "2:0".to_owned(),
                url: "thumb://localhost/normal/x.png".to_owned()
            }]
        );
    }

    #[test]
    fn the_same_file_asked_for_twice_is_one_job_with_an_answer_for_each_key() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        std::fs::write(&a, b"a").unwrap();
        let bridge = ThumbnailBridge::default();
        let queue = FakeQueue::default();
        let (seen, send) = collector();
        bridge.submit(
            &queue,
            vec![wanted("row", &a), wanted("shelf", &a)],
            ThumbSize::Large,
            send,
        );
        let requests = queue.requests.lock().unwrap();
        let (_, items, sink) = &requests[0];
        assert_eq!(items.len(), 1);
        sink(ThumbEvent::Skipped {
            key: items[0].key.clone(),
            why: SkipWhy::NoGenerator,
        });
        let keys: Vec<String> = seen
            .lock()
            .unwrap()
            .iter()
            .map(|event| event.key().to_owned())
            .collect();
        assert_eq!(keys, vec!["row".to_owned(), "shelf".to_owned()]);
    }

    #[test]
    fn what_cannot_be_asked_for_is_answered_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = ThumbnailBridge::default();
        let queue = FakeQueue::default();
        let (seen, send) = collector();
        bridge.submit(
            &queue,
            vec![
                Wanted {
                    key: "remote".to_owned(),
                    path: Err("not a local file".to_owned()),
                },
                wanted("gone", &dir.path().join("missing.png")),
                wanted("folder", dir.path()),
            ],
            ThumbSize::Normal,
            send,
        );
        let events = seen.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0], ThumbEvent::Failed { key, .. } if key == "remote"));
        assert!(matches!(&events[1], ThumbEvent::Failed { key, .. } if key == "gone"));
        assert!(matches!(
            &events[2],
            ThumbEvent::Skipped { key, why: SkipWhy::Unsupported } if key == "folder"
        ));
        assert!(queue.requests.lock().unwrap()[0].1.is_empty());
    }

    #[test]
    fn prioritising_and_cancelling_use_the_page_keys() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        let bridge = ThumbnailBridge::default();
        let queue = FakeQueue::default();
        let (_, send) = collector();
        let ticket = bridge.submit(
            &queue,
            vec![wanted("1", &a), wanted("2", &b)],
            ThumbSize::Normal,
            send,
        );
        bridge.prioritise(&queue, ticket, &["2".to_owned(), "unknown".to_owned()]);
        let prioritised = queue.prioritised.lock().unwrap();
        assert_eq!(prioritised.len(), 1);
        assert_eq!(prioritised[0].1.len(), 1);
        assert!(prioritised[0].1[0].contains("b.png"));
        drop(prioritised);
        assert!(bridge.cancel(&queue, ticket));
        assert_eq!(*queue.cancelled.lock().unwrap(), vec![ticket]);
        // A cancelled batch is forgotten: prioritising it does nothing.
        bridge.prioritise(&queue, ticket, &["1".to_owned()]);
        assert_eq!(queue.prioritised.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_finished_batch_is_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        std::fs::write(&a, b"a").unwrap();
        let bridge = ThumbnailBridge::default();
        let queue = FakeQueue::default();
        let (_, send) = collector();
        bridge.submit(&queue, vec![wanted("1", &a)], ThumbSize::Normal, send);
        assert_eq!(bridge.tickets.lock().unwrap().len(), 1);
        let requests = queue.requests.lock().unwrap();
        let (_, items, sink) = &requests[0];
        sink(ThumbEvent::Failed {
            key: items[0].key.clone(),
            reason: "bad".to_owned(),
        });
        assert!(bridge.tickets.lock().unwrap().is_empty());
    }

    #[test]
    fn only_a_local_file_has_a_path() {
        let local = waypoint_path::FilePath::from_path(std::env::temp_dir())
            .unwrap()
            .to_location();
        assert!(path_of_location(&local).is_ok());
        assert!(path_of_location(&Location::new("x", "sftp://host/x")).is_err());
    }

    #[test]
    fn the_size_cap_is_the_setting_in_bytes() {
        let mut settings = Settings::default();
        settings.previews.max_file_mb = 50;
        assert_eq!(max_file_bytes(&settings), 50 * 1024 * 1024);
    }
}

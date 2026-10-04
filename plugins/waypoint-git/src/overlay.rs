// The overlay that puts the Git status on the listings of folders in a working tree
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use waypoint_path::VfsPath;
use waypoint_provider_git::{
    folder_marks, repositories_in, StatusService, Subscription, TrackEvent, TrackSink,
};
use waypoint_vfs::{FolderMarks, FolderOverlay, GitMark, MarkSink, OverlayGuard};

/// What a running attachment holds.
#[derive(Default)]
struct Running {
    subscription: Option<Subscription>,
}

/// One listing the overlay decorates.
struct Attachment {
    folder: PathBuf,
    sink: MarkSink,
    running: Mutex<Running>,
}

/// Holds the attachment for as long as the listing keeps the guard.
struct Alive(#[allow(dead_code)] Arc<Attachment>);

impl OverlayGuard for Alive {}

/// Adds the repository flag to the marks of the subfolders that are repositories.
fn with_repositories(mut marks: FolderMarks, repositories: &HashSet<OsString>) -> FolderMarks {
    for name in repositories {
        let mark: &mut GitMark = marks.names.entry(name.clone()).or_default();
        mark.repository = true;
    }
    marks
}

/// Decorates local folders, for as long as the overlay is on: the folders of a working tree with
/// their Git status, and any folder with a mark on the subfolders that are repositories. Turning it
/// off clears the marks of every listing it decorates and stops watching; turning it on starts
/// again for the same listings.
pub struct GitOverlay {
    service: Arc<StatusService>,
    enabled: Arc<AtomicBool>,
    attached: Mutex<Vec<Weak<Attachment>>>,
}

impl GitOverlay {
    pub(crate) fn new(service: Arc<StatusService>, enabled: Arc<AtomicBool>) -> Self {
        Self {
            service,
            enabled,
            attached: Mutex::new(Vec::new()),
        }
    }

    /// Reads what the folder has to say and starts following it. Returns whether there is
    /// anything to decorate: a working tree, or subfolders that are repositories.
    fn start(&self, attachment: &Arc<Attachment>) -> bool {
        let repositories = repositories_in(&attachment.folder);
        let root = self.service.repository_of(&attachment.folder);
        match root {
            None if repositories.is_empty() => false,
            None => {
                (attachment.sink)(with_repositories(FolderMarks::default(), &repositories));
                true
            }
            Some(root) => {
                let folder = attachment.folder.clone();
                let sink = Arc::clone(&attachment.sink);
                let track: TrackSink = Arc::new(move |event| {
                    if let TrackEvent::Updated(snapshot) = event {
                        sink(with_repositories(
                            folder_marks(&snapshot.status, &folder),
                            &repositories,
                        ));
                    }
                });
                let subscription = self.service.subscribe(&root, track);
                attachment
                    .running
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .subscription = Some(subscription);
                true
            }
        }
    }

    fn stop(attachment: &Attachment) {
        let stopped = attachment
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .subscription
            .take();
        drop(stopped);
        (attachment.sink)(FolderMarks::default());
    }

    /// Follows the switch: off clears every decorated listing, on decorates them again.
    pub(crate) fn enabled_changed(&self, on: bool) {
        let live: Vec<Arc<Attachment>> = {
            let mut attached = self.attached.lock().unwrap_or_else(|e| e.into_inner());
            attached.retain(|weak| weak.strong_count() > 0);
            attached.iter().filter_map(Weak::upgrade).collect()
        };
        for attachment in live {
            if on {
                self.start(&attachment);
            } else {
                Self::stop(&attachment);
            }
        }
    }

    /// How many listings are decorated now.
    pub fn attached(&self) -> usize {
        let attached = self.attached.lock().unwrap_or_else(|e| e.into_inner());
        attached
            .iter()
            .filter(|weak| weak.strong_count() > 0)
            .count()
    }
}

impl FolderOverlay for GitOverlay {
    fn attach(&self, folder: &VfsPath, sink: MarkSink) -> Option<Box<dyn OverlayGuard>> {
        let VfsPath::File(file) = folder else {
            return None;
        };
        let attachment = Arc::new(Attachment {
            folder: file.as_path().to_path_buf(),
            sink,
            running: Mutex::new(Running::default()),
        });
        // While the switch is off nothing is read, so it is not yet known whether there is
        // anything to decorate: the attachment waits for the switch.
        if self.enabled.load(Ordering::SeqCst) && !self.start(&attachment) {
            return None;
        }
        self.attached
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Arc::downgrade(&attachment));
        Some(Box::new(Alive(attachment)))
    }
}

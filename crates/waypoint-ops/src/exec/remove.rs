// Removing a tree through a provider: children before their folder, links removed as links.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{CancelToken, EntryKind, Provider};

/// Removes `root` and everything below it. A symlink is removed (never followed) and everything
/// else goes children first, so a failure leaves a smaller tree that is still a valid one.
/// `each` hears of every entry as it goes. `cancel` is checked before each removal.
///
/// A folder on another volume than `root` (a mount point, a bind mount) is never entered: the walk
/// fails with `VfsError::InUse` before it touches what the other file system holds, as the removal
/// of the mount point itself would.
pub(crate) fn remove_tree(
    provider: &dyn Provider,
    root: &VfsPath,
    cancel: &CancelToken,
    each: &mut dyn FnMut(&VfsPath),
) -> Result<(), VfsError> {
    let top = provider.stat(root)?;
    // A symlink root is removed as a link: its volume is not asked (the answer follows the link).
    let volume = if top.kind == EntryKind::Directory {
        provider.volume_id(root)
    } else {
        None
    };
    // (path, kind, children already queued)
    let mut stack = vec![(root.clone(), top.kind, false)];
    while let Some((path, kind, expanded)) = stack.pop() {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        match (kind, expanded) {
            (EntryKind::Directory, false) => {
                if volume.is_some() && provider.volume_id(&path) != volume {
                    return Err(VfsError::InUse {
                        location: path.to_location(),
                    });
                }
                stack.push((path.clone(), kind, true));
                for child in provider.list(&path, cancel, 0, &mut |_| {})? {
                    let child_path = path.join(&child.name).map_err(|_| VfsError::InvalidName {
                        name: child.name.to_string_lossy().into_owned(),
                        reason: "not a usable name".to_owned(),
                    })?;
                    stack.push((child_path, child.kind, false));
                }
            }
            (EntryKind::Directory, true) => {
                provider.remove_dir(&path)?;
                each(&path);
            }
            _ => {
                provider.remove_file(&path)?;
                each(&path);
            }
        }
    }
    Ok(())
}

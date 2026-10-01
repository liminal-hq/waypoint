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
pub(crate) fn remove_tree(
    provider: &dyn Provider,
    root: &VfsPath,
    cancel: &CancelToken,
    each: &mut dyn FnMut(&VfsPath),
) -> Result<(), VfsError> {
    let top = provider.stat(root)?;
    // (path, kind, children already queued)
    let mut stack = vec![(root.clone(), top.kind, false)];
    while let Some((path, kind, expanded)) = stack.pop() {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        match (kind, expanded) {
            (EntryKind::Directory, false) => {
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

// Trashes through the Trash portal, which is all a Flatpak sandbox can do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::os::fd::AsFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::error::TrashError;
use crate::models::TrashReceipt;

/// Moves one path to the trash through `org.freedesktop.portal.Trash`. The portal returns nothing about where the item went, so the receipt's id cannot be used to restore it.
pub async fn trash(path: &Path) -> Result<TrashReceipt, TrashError> {
    if !path.is_absolute() {
        return Err(TrashError::io("the path must be absolute"));
    }
    // `O_PATH` names the file without reading it, and `O_NOFOLLOW` makes a link trash the link and not its target.
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    ashpd::desktop::trash::trash_file(&file.as_fd())
        .await
        .map_err(|error| TrashError::TrashUnavailable {
            reason: format!("the Trash portal refused: {error}"),
        })?;
    Ok(TrashReceipt {
        trash_id: format!("portal|{}", path.display()),
        original_path: path.to_path_buf(),
        deleted_at: chrono::Utc::now().timestamp(),
    })
}

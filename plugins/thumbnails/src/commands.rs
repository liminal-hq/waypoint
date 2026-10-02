// Tauri command handlers exposed to the webview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::{command, AppHandle, Runtime};

use crate::models::{PluginStatus, ThumbEvent, ThumbRequest, Ticket};
use crate::ThumbnailsExt;

#[command]
pub(crate) fn get_status<R: Runtime>(app: AppHandle<R>) -> PluginStatus {
    app.thumbnails().get_status()
}

/// Queues the thumbnails; each result arrives on `on_ready` as it is made. Returns the ticket to cancel or reprioritise with.
#[command]
pub(crate) fn request<R: Runtime>(
    app: AppHandle<R>,
    items: Vec<ThumbRequest>,
    on_ready: Channel<ThumbEvent>,
) -> Ticket {
    app.thumbnails().request(
        items,
        Arc::new(move |event| {
            let _ = on_ready.send(event);
        }),
    )
}

#[command]
pub(crate) fn cancel<R: Runtime>(app: AppHandle<R>, ticket: Ticket) -> bool {
    app.thumbnails().cancel(ticket)
}

#[command]
pub(crate) fn prioritise<R: Runtime>(app: AppHandle<R>, ticket: Ticket, keys: Vec<String>) {
    app.thumbnails().prioritise(ticket, &keys);
}

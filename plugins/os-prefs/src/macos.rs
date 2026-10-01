// Reads the 12/24-hour setting on macOS and follows locale-change notifications and a slow poll
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{ptr::NonNull, time::Duration};

use block2::RcBlock;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::{
    NSCurrentLocaleDidChangeNotification, NSNotification, NSNotificationCenter, NSObjectProtocol,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    apple,
    models::{TimeFormat, TimeFormatSource},
    service::{Poller, Readiness, Reading},
};

/// How often the setting is read again as a safety net: not every System Settings change that
/// affects the hour cycle is guaranteed to post the locale notification.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Holds the notification observer and the poller; dropping it stops both.
pub struct Watcher {
    _poller: Poller,
    observer: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

// SAFETY: the observer token is only retained and handed back to `removeObserver:` when the
// watcher is dropped, and `NSNotificationCenter` is documented as thread-safe.
unsafe impl Send for Watcher {}

impl Watcher {
    pub fn take_readiness(&mut self) -> Readiness {
        Readiness::Listening
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.take() {
            // SAFETY: `observer` is the token `addObserverForName:...` returned.
            unsafe { NSNotificationCenter::defaultCenter().removeObserver(observer.as_ref()) };
        }
    }
}

pub async fn read() -> Reading {
    match tauri::async_runtime::spawn_blocking(apple::is_24_hour)
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result)
    {
        Ok(is_24_hour) => Reading {
            format: TimeFormat {
                is_24_hour,
                source: TimeFormatSource::MacosDateTemplate,
            },
            note: None,
        },
        Err(reason) => Reading {
            format: TimeFormat {
                is_24_hour: false,
                source: TimeFormatSource::Default,
            },
            note: Some(reason),
        },
    }
}

pub fn watch(changed: UnboundedSender<()>) -> Watcher {
    let notify = changed.clone();
    let block = RcBlock::new(move |_notification: NonNull<NSNotification>| {
        let _ = notify.send(());
    });
    // SAFETY: the block only sends on a channel, which is safe from any thread, and `queue: None`
    // runs it on the posting thread. `NSCurrentLocaleDidChangeNotification` is a valid name, and
    // no object filter is given.
    let observer = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(NSCurrentLocaleDidChangeNotification),
            None,
            None,
            &block,
        )
    };
    Watcher {
        _poller: Poller::start(changed, POLL_INTERVAL),
        observer: Some(observer),
    }
}

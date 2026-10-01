// Reads and watches the 12/24-hour setting on Linux, choosing a source by desktop environment
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// GNOME-family desktops expose `clock-format` through the xdg-desktop-portal Settings interface
// (with `gsettings` as a second route) and Cinnamon keeps `clock-use-24h` in its own schema. The
// portal is not consulted elsewhere: on desktops that do not own that key it reports the schema
// default (24-hour), which would override the locale. KDE applies its Region settings as
// `LC_TIME`, and Xfce and MATE keep the clock in panel plugin options with no stable key, so those
// desktops use the locale's convention.

mod cli;
mod gsettings;
mod locale;
mod portal;

use tokio::sync::mpsc::UnboundedSender;

use crate::{
    models::{TimeFormat, TimeFormatSource},
    parse::{self, Desktop},
    service::{Readiness, Reading},
};

/// Keeps the change watchers alive; dropping it stops them and kills any child processes.
pub struct Watcher {
    _guards: Vec<Box<dyn Send>>,
    readiness: Option<Readiness>,
}

impl Watcher {
    /// Takes how the watcher reports that it is listening.
    pub fn take_readiness(&mut self) -> Readiness {
        self.readiness.take().unwrap_or(Readiness::Listening)
    }
}

fn current_desktop() -> Desktop {
    parse::desktop(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
}

/// Runs a blocking reader off the async executor.
async fn blocking<T, F>(reader: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(reader)
        .await
        .map_err(|error| error.to_string())?
}

fn format(is_24_hour: bool, source: TimeFormatSource) -> TimeFormat {
    TimeFormat { is_24_hour, source }
}

/// Answers with the locale's convention, noting why the desktop's own setting was not used.
fn locale_reading(note: Option<String>) -> Reading {
    match locale::read() {
        Ok(is_24_hour) => Reading {
            format: format(is_24_hour, TimeFormatSource::Locale),
            note,
        },
        Err(reason) => Reading {
            format: format(false, TimeFormatSource::Default),
            note: Some(match note {
                Some(note) => format!("{note}; locale: {reason}"),
                None => format!("locale: {reason}"),
            }),
        },
    }
}

pub async fn read() -> Reading {
    match current_desktop() {
        Desktop::Gnome => {
            let portal_error = match portal::read().await {
                Ok(is_24_hour) => {
                    return Reading {
                        format: format(is_24_hour, TimeFormatSource::GnomePortal),
                        note: None,
                    }
                }
                Err(reason) => format!("portal: {reason}"),
            };
            match blocking(gsettings::read_gnome).await {
                Ok(is_24_hour) => Reading {
                    format: format(is_24_hour, TimeFormatSource::GnomeGsettings),
                    note: Some(format!("{portal_error}; read through gsettings instead")),
                },
                Err(reason) => {
                    log::warn!("{portal_error}; gsettings: {reason}");
                    locale_reading(Some(format!(
                        "{portal_error}; gsettings: {reason}; using the locale's convention"
                    )))
                }
            }
        }
        Desktop::Cinnamon => match blocking(gsettings::read_cinnamon).await {
            Ok(is_24_hour) => Reading {
                format: format(is_24_hour, TimeFormatSource::CinnamonGsettings),
                note: None,
            },
            Err(reason) => {
                log::warn!("gsettings: {reason}");
                locale_reading(Some(format!(
                    "gsettings: {reason}; using the locale's convention"
                )))
            }
        },
        Desktop::Other => locale_reading(Some(
            "this desktop has no readable clock setting; using the locale's convention".to_string(),
        )),
    }
}

pub fn watch(changed: UnboundedSender<()>) -> Watcher {
    let (guards, readiness): (Vec<Box<dyn Send>>, _) = match current_desktop() {
        Desktop::Gnome => {
            let (guards, ready) = portal::watch(changed);
            (guards, Readiness::Signal(ready))
        }
        // This starts a child process whose subscription cannot be observed.
        Desktop::Cinnamon => (gsettings::watch_cinnamon(changed), Readiness::Unconfirmed),
        Desktop::Other => (
            Vec::new(),
            Readiness::Unavailable(
                "this desktop does not announce clock changes; the locale is read on demand"
                    .to_string(),
            ),
        ),
    };
    Watcher {
        _guards: guards,
        readiness: Some(readiness),
    }
}

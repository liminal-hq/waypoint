// Reads and watches the appearance preferences on Linux: the portal first, then the desktop's own settings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod gsettings;
mod kdeglobals;
mod portal;

use tokio::sync::mpsc::UnboundedSender;

use super::{
    models::{AppearanceSource, UnavailableReason},
    resolve::{resolve, Resolution, SourceReading},
};
use crate::{linux::cli, models::DesktopEnvironment, parse, service::Readiness};

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

fn current_desktop() -> DesktopEnvironment {
    parse::desktop_environment(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
}

/// Runs a blocking reader off the async executor; if the task panics, the reading fails as `source`.
async fn blocking<F>(source: AppearanceSource, reader: F) -> SourceReading
where
    F: FnOnce() -> SourceReading + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(reader)
        .await
        .unwrap_or_else(|error| {
            SourceReading::failed(source, UnavailableReason::ReadFailed, error.to_string())
        })
}

/// Reads every source this desktop has, most authoritative first.
///
/// The portal always goes first. KDE adds `kdeglobals` (and `kcmfonts` for the text scale); Cinnamon and the GNOME family (and
/// desktops this plugin does not recognise) add `gsettings`, asked only for what the portal did
/// not answer. MATE and Xfce keep their settings elsewhere, so only the portal is asked there.
pub async fn read() -> Resolution {
    let desktop = current_desktop();
    let portal = portal::read().await;
    let missing = portal.values.missing();
    let mut readings = vec![portal];
    match desktop {
        DesktopEnvironment::Kde => {
            readings.push(blocking(AppearanceSource::KdeGlobals, kdeglobals::read).await)
        }
        DesktopEnvironment::Cinnamon => readings.push(
            blocking(AppearanceSource::Gsettings, move || {
                gsettings::read(&gsettings::CINNAMON, &missing)
            })
            .await,
        ),
        DesktopEnvironment::Mate | DesktopEnvironment::Xfce => {}
        _ if missing.is_empty() => {}
        _ => readings.push(
            blocking(AppearanceSource::Gsettings, move || {
                gsettings::read(&gsettings::GNOME, &missing)
            })
            .await,
        ),
    }
    resolve(&readings)
}

pub fn watch(changed: UnboundedSender<()>) -> Watcher {
    let (mut guards, portal_ready) = portal::watch(changed.clone());
    let readiness = match current_desktop() {
        DesktopEnvironment::Kde => {
            // The file watcher is registered before `watch_file` returns.
            for name in [kdeglobals::FILE_NAME, kdeglobals::FONTS_FILE_NAME] {
                guards.extend(crate::linux::kwin::watch_file(name, changed.clone()));
            }
            Readiness::Signal(portal_ready)
        }
        DesktopEnvironment::Cinnamon => {
            for schema in gsettings::CINNAMON_SCHEMAS {
                guards.extend(cli::monitor_guard(
                    "gsettings",
                    &["monitor", schema],
                    changed.clone(),
                ));
            }
            // The monitors are child processes whose subscription cannot be observed.
            Readiness::Unconfirmed
        }
        _ => Readiness::Signal(portal_ready),
    };
    Watcher {
        _guards: guards,
        readiness: Some(readiness),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panicking_reader_is_labelled_with_its_own_source() {
        let reading =
            tauri::async_runtime::block_on(blocking(AppearanceSource::KdeGlobals, || {
                panic!("boom")
            }));
        assert_eq!(reading.source, AppearanceSource::KdeGlobals);
        assert!(reading
            .misses
            .iter()
            .all(|m| m.reason == UnavailableReason::ReadFailed));
    }

    /// Live read of this machine's appearance preferences; prints what each source reported and
    /// what won. Read-only. Run with `cargo test live_appearance_read -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_appearance_read() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let resolution = runtime.block_on(read());
        println!("desktop: {:?}", current_desktop());
        println!(
            "{}",
            serde_json::to_string_pretty(&resolution.values).unwrap()
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&resolution.status).unwrap()
        );
    }
}

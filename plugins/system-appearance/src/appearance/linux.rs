// Reads and watches the appearance preferences on Linux: the portal first, then the desktop's own settings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod gsettings;
mod kdeglobals;
mod portal;

use tokio::sync::mpsc::UnboundedSender;

use super::{
    models::{AppearanceFeature, AppearanceSource, UnavailableReason},
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

/// Whether asking `gsettings` could answer any of the `missing` features, so that spawning it is
/// worth it. Features no key holds on this desktop (reduced transparency everywhere) never count.
fn gsettings_can_help(keys: &gsettings::Keys, missing: &[AppearanceFeature]) -> bool {
    missing.iter().any(|feature| keys.supplies(*feature))
}

/// Reads the `gsettings` keys of `keys` for the features still missing, if any could be there.
async fn gsettings_reading(
    keys: &'static gsettings::Keys,
    missing: Vec<AppearanceFeature>,
    reader: GsettingsReader,
) -> Option<SourceReading> {
    if !gsettings_can_help(keys, &missing) {
        return None;
    }
    Some(blocking(AppearanceSource::Gsettings, move || reader(keys, &missing)).await)
}

type GsettingsReader = fn(&'static gsettings::Keys, &[AppearanceFeature]) -> SourceReading;

/// Adds to the portal's reading the readings of the other sources this desktop has, most
/// authoritative first.
///
/// KDE adds `kdeglobals` (and `kcmfonts` for the text scale); Cinnamon and the GNOME family (and
/// desktops this plugin does not recognise) add `gsettings`, asked only for what the portal did
/// not answer and only when a key could hold it. MATE and Xfce keep their settings elsewhere, so
/// only the portal is asked there.
async fn gather(
    desktop: DesktopEnvironment,
    portal: SourceReading,
    kdeglobals_reader: fn() -> SourceReading,
    gsettings_reader: GsettingsReader,
) -> Vec<SourceReading> {
    let missing = portal.values.missing();
    let mut readings = vec![portal];
    match desktop {
        DesktopEnvironment::Kde => {
            readings.push(blocking(AppearanceSource::KdeGlobals, kdeglobals_reader).await)
        }
        DesktopEnvironment::Cinnamon => readings
            .extend(gsettings_reading(&gsettings::CINNAMON, missing, gsettings_reader).await),
        DesktopEnvironment::Mate | DesktopEnvironment::Xfce => {}
        _ => readings.extend(gsettings_reading(&gsettings::GNOME, missing, gsettings_reader).await),
    }
    readings
}

/// Reads every source this desktop has, most authoritative first; the portal always goes first.
pub async fn read() -> Resolution {
    let portal = portal::read().await;
    let readings = gather(current_desktop(), portal, kdeglobals::read, gsettings::read).await;
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::appearance::models::{ColourScheme, Contrast};

    static GSETTINGS_RUNS: AtomicUsize = AtomicUsize::new(0);

    fn counting_gsettings(
        _keys: &'static gsettings::Keys,
        _wanted: &[AppearanceFeature],
    ) -> SourceReading {
        GSETTINGS_RUNS.fetch_add(1, Ordering::SeqCst);
        SourceReading::new(AppearanceSource::Gsettings)
    }

    fn failing_kdeglobals() -> SourceReading {
        panic!("kdeglobals is not read in these tests");
    }

    /// A portal reading that answered everything the portal can, as on GNOME.
    fn full_portal() -> SourceReading {
        let mut reading = SourceReading::new(AppearanceSource::Portal);
        reading.values.colour_scheme = Some(ColourScheme::Dark);
        reading.values.accent = Some("#3584e4".to_string());
        reading.values.contrast = Some(Contrast::Normal);
        reading.values.reduced_motion = Some(false);
        reading.values.text_scale = Some(1.0);
        reading.values.icon_theme = Some("Adwaita".to_string());
        reading.miss(
            AppearanceFeature::ReducedTransparency,
            UnavailableReason::NoSource,
            "the portal has no reduced-transparency setting",
        );
        reading
    }

    fn gathered(desktop: DesktopEnvironment, portal: SourceReading) -> usize {
        let before = GSETTINGS_RUNS.load(Ordering::SeqCst);
        tauri::async_runtime::block_on(gather(
            desktop,
            portal,
            failing_kdeglobals,
            counting_gsettings,
        ));
        GSETTINGS_RUNS.load(Ordering::SeqCst) - before
    }

    #[test]
    fn gsettings_is_spawned_only_when_it_could_answer_something() {
        // The portal answered all it can: only reduced transparency is missing, which no
        // desktop's keys hold.
        assert_eq!(gathered(DesktopEnvironment::Gnome, full_portal()), 0);
        assert_eq!(gathered(DesktopEnvironment::Unknown, full_portal()), 0);
        // Cinnamon has no accent key either, so a portal without one leaves nothing to ask.
        let mut no_accent = full_portal();
        no_accent.values.accent = None;
        assert_eq!(gathered(DesktopEnvironment::Cinnamon, no_accent), 0);
        // A feature a key could hold is still asked for, once.
        let mut no_icons = full_portal();
        no_icons.values.icon_theme = None;
        assert_eq!(gathered(DesktopEnvironment::Gnome, no_icons.clone()), 1);
        assert_eq!(gathered(DesktopEnvironment::Cinnamon, no_icons), 1);
        // A failed portal asks for everything.
        let failed = SourceReading::failed(
            AppearanceSource::Portal,
            UnavailableReason::PortalUnavailable,
            "no portal",
        );
        assert_eq!(gathered(DesktopEnvironment::Gnome, failed), 1);
        // MATE and Xfce never use gsettings.
        assert_eq!(
            gathered(
                DesktopEnvironment::Mate,
                SourceReading::new(AppearanceSource::Portal)
            ),
            0
        );
    }

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

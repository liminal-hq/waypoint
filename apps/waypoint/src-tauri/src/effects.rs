// Puts the window effects the Transparency settings ask for behind each window, and keeps the shadow inset right
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The page decides how see-through its own backgrounds are (alpha, from the settings and the OS
// preferences); Rust decides the one thing the page cannot do, which is to ask the compositor or
// DWM to put blur, Mica or Acrylic behind the window. The page never sends a value to the
// `window-effects` plugin (its capability is `get_status` only): the settings are the one source,
// `plan` turns them into exactly one request per window, and `reconcile` makes the window match
// it after every change of the settings, every window created, and every focus, size, maximise or
// theme change. A request the system cannot do is never made: `plan` reads the plugin's status.
//
// The shadow inset is the other half. Every window is transparent and frameless, and on Linux its
// page draws the shadow in an 8 px margin (D89); `gdk_window_set_shadow_width` tells the compositor
// that margin is not part of the window, so a window tiled to half the screen sits flush. It is
// set whatever the Transparency settings say, and zeroed while the window is maximised.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime, Theme, Webview, Window, WindowEvent};
use tauri_plugin_waypoint_settings::SettingsStore;
use tauri_plugin_window_effects::{
    EffectKind, Effects, Flavour, Insets, PluginStatus, Rect, WindowEffects, FEATURE_ACRYLIC,
    FEATURE_BLUR, FEATURE_MICA, FEATURE_OPACITY, FEATURE_SHADOW_INSET,
};
use waypoint_protocol::WindowKind;
use waypoint_settings::{BlurLevel, ColourMode, OsPreference, Settings};

/// The transparent margin the frame draws around a Linux window, in logical pixels: the page's
/// `--wp-window-shadow-margin` for `data-platform='linux'` (`theme/tokens.css`).
pub const SHADOW_MARGIN: i32 = 8;

/// How long a burst of resizes settles before the blur region is fitted to the new size.
const RESIZE_SETTLE: Duration = Duration::from_millis(150);

/// What a window is like now, as far as its effects are concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowState {
    /// The window is in front.
    pub focused: bool,
    /// Maximised or full screen: it meets the screen's edges, so the frame has no margin.
    pub maximised: bool,
    /// The window draws dark, which picks the dark Mica and the tint of Acrylic.
    pub dark: bool,
    /// The window's size in logical pixels, which a blur region is in.
    pub size: (i64, i64),
}

/// What one window should have behind it: the whole of what `reconcile` asks the plugin for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The effect behind the window, or none.
    pub effect: Option<Effects>,
    /// The shadow inset, on a window that can have one (a GTK window).
    pub inset: Option<Insets>,
}

/// Whether the settings and the system ask for an effect behind a window now, and which. Nothing
/// is asked for unless the master switch is on, the person has not forced high contrast or reduced
/// transparency on (the OS's own preference is the page's to read, and the page then draws solid,
/// which hides the effect), blur is not Off, a window that is not in front is allowed to be
/// translucent, and the plugin says the system can do it. Blur is there to be seen, so it keeps
/// showing in a window that is not in front whatever "Solid when not in front" says; the page
/// switches that setting off while blur is on.
fn effect_for(settings: &Settings, status: &PluginStatus, window: &WindowState) -> Option<Effects> {
    let wanted = &settings.transparency;
    let access = &settings.accessibility;
    if !wanted.enabled || wanted.blur == BlurLevel::Off {
        return None;
    }
    if access.high_contrast == OsPreference::On || access.reduced_transparency == OsPreference::On {
        return None;
    }
    if !status.has(FEATURE_OPACITY) {
        return None;
    }
    let kind = match status.flavour {
        // Acrylic is the blur; Mica, which only tints the window with the wallpaper, stands in for it
        // where it is all there is.
        Flavour::Windows => {
            let (mica, acrylic) = (status.has(FEATURE_MICA), status.has(FEATURE_ACRYLIC));
            match (mica, acrylic) {
                (_, true) => EffectKind::Acrylic,
                (true, false) => EffectKind::Mica,
                _ => return None,
            }
        }
        // The compositors have one blur and no strength to ask for.
        Flavour::Wayland | Flavour::X11 => {
            if !status.has(FEATURE_BLUR) {
                return None;
            }
            EffectKind::Blur
        }
        Flavour::Unsupported => return None,
    };
    // Blur only the visible window, not the margin its shadow is drawn in. Windows covers the whole
    // window and ignores a region.
    let margin = i64::from(SHADOW_MARGIN);
    let region = (status.flavour != Flavour::Windows
        && !window.maximised
        && window.size.0 > 2 * margin
        && window.size.1 > 2 * margin)
        .then(|| {
            vec![Rect {
                x: SHADOW_MARGIN,
                y: SHADOW_MARGIN,
                width: (window.size.0 - 2 * margin) as i32,
                height: (window.size.1 - 2 * margin) as i32,
            }]
        });
    Some(Effects {
        kind,
        dark: window.dark,
        region,
    })
}

/// The shadow inset a window has: the frame's margin, or none while it is maximised. `None` where
/// the window cannot have one.
fn inset_for(status: &PluginStatus, window: &WindowState) -> Option<Insets> {
    if !status.has(FEATURE_SHADOW_INSET) {
        return None;
    }
    let all = if window.maximised { 0 } else { SHADOW_MARGIN };
    Some(Insets {
        top: all,
        right: all,
        bottom: all,
        left: all,
    })
}

/// What a window should have behind it under `settings`, on a system that reports `status`.
pub fn plan(settings: &Settings, status: &PluginStatus, window: &WindowState) -> Plan {
    Plan {
        effect: effect_for(settings, status, window),
        inset: inset_for(status, window),
    }
}

/// Whether a window with this label has effects: every window the frame draws, but not the
/// tear-off ghost, a click-through preview with no frame of its own.
pub fn applies_to(label: &str) -> bool {
    matches!(
        WindowKind::from_label(label),
        Some(WindowKind::Main | WindowKind::Settings | WindowKind::Properties | WindowKind::Ops)
    )
}

#[derive(Default)]
struct Debounce {
    generation: u64,
    running: bool,
}

/// What the app remembers: the last plan made for each window, so nothing is asked twice, and the
/// plugin's status.
#[derive(Default)]
pub struct Driver {
    applied: Mutex<HashMap<String, Plan>>,
    status: Mutex<Option<PluginStatus>>,
    /// The last failure per window, so a request the system keeps refusing is logged once.
    failed: Mutex<HashMap<String, String>>,
    settle: Mutex<HashMap<String, Debounce>>,
    /// One request at a time, so a late one never overtakes an earlier one.
    gate: tauri::async_runtime::Mutex<()>,
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn settings_now<R: Runtime>(app: &AppHandle<R>) -> Settings {
    app.try_state::<SettingsStore<R>>()
        .map(|store| store.get())
        .unwrap_or_default()
}

/// What a window is like now. `None` for a window that is gone.
fn window_state<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
    settings: &Settings,
) -> Option<WindowState> {
    let window = app.get_webview_window(label)?;
    let dark = match settings.appearance.mode {
        ColourMode::Dark => true,
        ColourMode::Light => false,
        ColourMode::System => window.theme().map(|t| t == Theme::Dark).unwrap_or(false),
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window
        .inner_size()
        .map(|s| {
            let logical = s.to_logical::<f64>(scale);
            (logical.width.round() as i64, logical.height.round() as i64)
        })
        .unwrap_or((0, 0));
    Some(WindowState {
        focused: window.is_focused().unwrap_or(true),
        maximised: window.is_maximized().unwrap_or(false)
            || window.is_fullscreen().unwrap_or(false),
        dark,
        size,
    })
}

/// Makes the window `label` match its plan. Idempotent: it asks the plugin only for what differs
/// from what it asked last, and reads the settings and the window's state when it runs, so two
/// requests that overlap end in the same place.
pub async fn reconcile<R: Runtime>(app: AppHandle<R>, label: String, refresh_status: bool) {
    let Some(driver) = app.try_state::<Driver>() else {
        return;
    };
    let _turn = driver.gate.lock().await;
    let Some(effects) = app.try_state::<WindowEffects<R>>() else {
        return;
    };
    let settings = settings_now(&app);
    let Some(window) = window_state(&app, &label, &settings) else {
        locked(&driver.applied).remove(&label);
        locked(&driver.failed).remove(&label);
        locked(&driver.settle).remove(&label);
        return;
    };
    let known = locked(&driver.status).clone();
    let status = match known {
        Some(status) if !refresh_status => status,
        _ => {
            let fresh = effects.get_status().await;
            *locked(&driver.status) = Some(fresh.clone());
            fresh
        }
    };
    let wanted = plan(&settings, &status, &window);
    let before = locked(&driver.applied).get(&label).cloned();
    if before.as_ref() == Some(&wanted) {
        return;
    }
    let before_effect = before.as_ref().and_then(|p| p.effect.clone());
    let mut done = wanted.clone();
    if wanted.effect != before_effect {
        let result = match &wanted.effect {
            Some(effect) => effects.apply(&label, effect.clone()).await,
            None if before_effect.is_some() => effects.clear(&label).await,
            None => Ok(()),
        };
        if let Err(e) = result {
            report(&driver, &label, "apply the window effect", &e.to_string());
            // Not recorded, so the next change tries again.
            done.effect = before_effect;
        }
    }
    let before_inset = before.as_ref().and_then(|p| p.inset);
    if wanted.inset != before_inset {
        if let Some(inset) = wanted.inset {
            if let Err(e) = effects.set_shadow_inset(&label, inset).await {
                report(&driver, &label, "set the shadow inset", &e.to_string());
                done.inset = before_inset;
            }
        }
    }
    locked(&driver.applied).insert(label, done);
}

/// Logs a failure once per window and message.
fn report(driver: &Driver, label: &str, what: &str, why: &str) {
    let mut failed = locked(&driver.failed);
    if failed.get(label).map(String::as_str) != Some(why) {
        log::warn!("could not {what} of {label}: {why}");
        failed.insert(label.to_string(), why.to_string());
    }
}

fn reconcile_soon<R: Runtime>(app: &AppHandle<R>, label: &str, refresh_status: bool) {
    let (app, label) = (app.clone(), label.to_string());
    tauri::async_runtime::spawn(reconcile(app, label, refresh_status));
}

/// Reconciles `label` once a burst of events has settled (a resize sends one per frame).
fn reconcile_settled<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let Some(driver) = app.try_state::<Driver>() else {
        return;
    };
    let generation = {
        let mut settle = locked(&driver.settle);
        let entry = settle.entry(label.to_string()).or_default();
        entry.generation += 1;
        if entry.running {
            return;
        }
        entry.running = true;
        entry.generation
    };
    let (app, label) = (app.clone(), label.to_string());
    std::thread::spawn(move || {
        let mut seen = generation;
        loop {
            std::thread::sleep(RESIZE_SETTLE);
            let Some(driver) = app.try_state::<Driver>() else {
                return;
            };
            let mut settle = locked(&driver.settle);
            let Some(entry) = settle.get_mut(&label) else {
                return;
            };
            if entry.generation != seen {
                seen = entry.generation;
                continue;
            }
            entry.running = false;
            drop(settle);
            tauri::async_runtime::block_on(reconcile(app.clone(), label, false));
            return;
        }
    });
}

/// Makes every window match the settings: after a change of them, when the status is read again
/// because the compositor can change under the app.
pub fn reconcile_all<R: Runtime>(app: &AppHandle<R>) {
    for label in app.webview_windows().into_keys().filter(|l| applies_to(l)) {
        reconcile_soon(app, &label, true);
    }
}

/// Follows the settings: every change reconciles every window. Call once, after the settings
/// plugin and the window-effects plugin are set up.
pub fn wire<R: Runtime>(app: &AppHandle<R>) {
    let Some(store) = app.try_state::<SettingsStore<R>>() else {
        return;
    };
    let handle = app.clone();
    store.on_change(move |_| reconcile_all(&handle));
}

/// A window's page started loading, so the window exists: give it what the settings ask for.
pub fn on_page_load<R: Runtime>(webview: &Webview<R>) {
    let label = webview.label();
    if applies_to(label) {
        reconcile_soon(webview.app_handle(), label, false);
    }
}

/// Follows the window's own changes: in front or not, resized (which includes maximised), the
/// theme changed.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if !applies_to(window.label()) {
        return;
    }
    match event {
        WindowEvent::Focused(_) | WindowEvent::ThemeChanged(_) => {
            reconcile_soon(window.app_handle(), window.label(), false)
        }
        WindowEvent::Resized(_) => reconcile_settled(window.app_handle(), window.label()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tauri_plugin_window_effects::{
        status_for, Backend, BoxFuture, Desktop, Environment, SessionType, EXT_BACKGROUND_EFFECT,
    };
    use waypoint_settings::MemoryStorage;

    fn settings(change: impl FnOnce(&mut Settings)) -> Settings {
        let mut s = Settings::default();
        s.transparency.enabled = true;
        s.transparency.blur = BlurLevel::High;
        change(&mut s);
        s
    }

    /// Settings with the master switch on and no solid backing for a window that is not in front:
    /// a mock window is never in front.
    fn lit(change: fn(&mut Settings)) -> Settings {
        settings(|s| {
            s.transparency.solid_when_unfocused = false;
            change(s)
        })
    }

    fn state() -> WindowState {
        WindowState {
            focused: true,
            maximised: false,
            dark: false,
            size: (1100, 720),
        }
    }

    fn kde_wayland() -> PluginStatus {
        status_for(&Environment {
            session_type: SessionType::Wayland,
            desktop: Desktop::Kde,
            has_composite: true,
            wayland_globals: vec![EXT_BACKGROUND_EFFECT.to_string()],
            build_number: None,
        })
    }

    fn gnome_wayland() -> PluginStatus {
        status_for(&Environment {
            session_type: SessionType::Wayland,
            desktop: Desktop::Gnome,
            has_composite: true,
            wayland_globals: Vec::new(),
            build_number: None,
        })
    }

    fn windows(build: u32) -> PluginStatus {
        status_for(&Environment {
            session_type: SessionType::Windows,
            desktop: Desktop::Other,
            has_composite: true,
            wayland_globals: Vec::new(),
            build_number: Some(build),
        })
    }

    #[test]
    fn nothing_is_asked_for_by_default() {
        let wanted = plan(&Settings::default(), &kde_wayland(), &state());
        assert_eq!(wanted.effect, None);
        // The inset is the frame's, whatever the settings say.
        assert_eq!(wanted.inset.map(|i| i.left), Some(SHADOW_MARGIN));
    }

    #[test]
    fn blur_is_asked_for_where_the_compositor_offers_it_and_fits_the_visible_window() {
        let wanted = plan(&settings(|_| {}), &kde_wayland(), &state());
        let effect = wanted.effect.expect("blur on KDE");
        assert_eq!(effect.kind, EffectKind::Blur);
        assert_eq!(
            effect.region,
            Some(vec![Rect {
                x: 8,
                y: 8,
                width: 1084,
                height: 704
            }])
        );
    }

    #[test]
    fn blur_covers_the_whole_window_when_it_is_maximised_and_the_inset_goes() {
        let maximised = WindowState {
            maximised: true,
            ..state()
        };
        let wanted = plan(&settings(|_| {}), &kde_wayland(), &maximised);
        assert_eq!(wanted.effect.expect("blur").region, None);
        assert_eq!(
            wanted.inset.map(|i| (i.top, i.right, i.bottom, i.left)),
            Some((0, 0, 0, 0))
        );
    }

    #[test]
    fn gnome_has_no_blur_to_ask_for_but_keeps_the_inset() {
        let wanted = plan(&settings(|_| {}), &gnome_wayland(), &state());
        assert_eq!(wanted.effect, None);
        assert!(wanted.inset.is_some());
    }

    #[test]
    fn every_guardrail_asks_for_nothing() {
        let kde = kde_wayland();
        let off = |change: fn(&mut Settings)| plan(&settings(change), &kde, &state()).effect;
        assert!(off(|_| {}).is_some());
        assert_eq!(off(|s| s.transparency.enabled = false), None);
        assert_eq!(off(|s| s.transparency.blur = BlurLevel::Off), None);
        assert_eq!(
            off(|s| s.accessibility.high_contrast = OsPreference::On),
            None
        );
        assert_eq!(
            off(|s| s.accessibility.reduced_transparency = OsPreference::On),
            None
        );
        // Following the OS is the page's to read: it draws solid, and the blur behind it is unseen.
        assert!(off(|s| s.accessibility.high_contrast = OsPreference::Follow).is_some());
        let unfocused = WindowState {
            focused: false,
            ..state()
        };
        // The blur is there to be seen, so a window that is not in front keeps it whatever the
        // setting that draws such a window solid says.
        assert!(plan(&settings(|_| {}), &kde, &unfocused).effect.is_some());
        let solid = settings(|s| s.transparency.solid_when_unfocused = true);
        assert!(plan(&solid, &kde, &unfocused).effect.is_some());
    }

    #[test]
    fn a_system_that_cannot_be_transparent_gets_nothing() {
        let mut status = kde_wayland();
        status
            .features
            .iter_mut()
            .find(|f| f.name == FEATURE_OPACITY)
            .unwrap()
            .available = false;
        assert_eq!(plan(&settings(|_| {}), &status, &state()).effect, None);
    }

    #[test]
    fn windows_blurs_with_acrylic_and_covers_the_whole_window() {
        let both = windows(22621);
        let blur = plan(&settings(|_| {}), &both, &state()).effect.unwrap();
        assert_eq!((blur.kind, blur.region), (EffectKind::Acrylic, None));
        // Windows 10 has no Mica and blurs the same way.
        let ten = windows(19045);
        assert_eq!(
            plan(&settings(|_| {}), &ten, &state()).effect.unwrap().kind,
            EffectKind::Acrylic
        );
        // Off asks for nothing, on Windows too.
        let off = settings(|s| s.transparency.blur = BlurLevel::Off);
        assert_eq!(plan(&off, &both, &state()).effect, None);
        // And no inset: that is a GTK feature.
        assert_eq!(plan(&settings(|_| {}), &both, &state()).inset, None);
    }

    #[test]
    fn the_window_state_picks_the_dark_material() {
        let dark = WindowState {
            dark: true,
            ..state()
        };
        assert!(
            plan(&settings(|_| {}), &windows(22621), &dark)
                .effect
                .unwrap()
                .dark
        );
    }

    #[test]
    fn the_tear_off_ghost_has_no_effects_and_every_framed_window_does() {
        for label in ["main-1", "main-12", "settings", "ops", "properties-3"] {
            assert!(applies_to(label), "{label}");
        }
        for label in ["tear-ghost", "other"] {
            assert!(!applies_to(label), "{label}");
        }
    }

    // The window-effects plugin over a backend that records what it is asked, on a mock app.

    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<String>>,
        env: Mutex<Option<Environment>>,
    }

    struct Fake(Arc<Recorder>);

    impl<R: Runtime> Backend<R> for Fake {
        fn environment(&self, _app: &AppHandle<R>) -> BoxFuture<'_, Environment> {
            let env = locked(&self.0.env).clone().unwrap();
            Box::pin(async move { env })
        }
        fn logical_size(
            &self,
            _window: &Window<R>,
        ) -> tauri_plugin_window_effects::Result<(i64, i64)> {
            Ok((1100, 720))
        }
        fn apply(
            &self,
            _window: Window<R>,
            _env: Environment,
            effects: Effects,
        ) -> BoxFuture<'_, tauri_plugin_window_effects::Result<()>> {
            locked(&self.0.calls).push(format!("apply {:?} dark={}", effects.kind, effects.dark));
            Box::pin(async { Ok(()) })
        }
        fn clear(
            &self,
            _window: Window<R>,
            _env: Environment,
        ) -> BoxFuture<'_, tauri_plugin_window_effects::Result<()>> {
            locked(&self.0.calls).push("clear".into());
            Box::pin(async { Ok(()) })
        }
        fn set_shadow_inset(
            &self,
            _window: Window<R>,
            insets: Insets,
        ) -> BoxFuture<'_, tauri_plugin_window_effects::Result<()>> {
            locked(&self.0.calls).push(format!("inset {}", insets.left));
            Box::pin(async { Ok(()) })
        }
    }

    fn app_with(recorder: &Arc<Recorder>) -> tauri::App<tauri::test::MockRuntime> {
        use tauri::{WebviewUrl, WebviewWindowBuilder};
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_window_effects::init_with(Arc::new(Fake(
                Arc::clone(recorder),
            ))))
            .plugin(tauri_plugin_waypoint_settings::init(Arc::new(
                MemoryStorage::default(),
            )))
            .manage(Driver::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app builds");
        WebviewWindowBuilder::new(&app, "main-1", WebviewUrl::App("index.html".into()))
            .build()
            .expect("the mock window opens");
        app
    }

    fn calls(recorder: &Recorder) -> Vec<String> {
        std::mem::take(&mut *locked(&recorder.calls))
    }

    #[test]
    fn a_window_follows_the_settings_and_is_asked_only_for_what_changed() {
        let recorder = Arc::new(Recorder::default());
        *locked(&recorder.env) = Some(Environment {
            session_type: SessionType::Wayland,
            desktop: Desktop::Kde,
            has_composite: true,
            wayland_globals: vec![EXT_BACKGROUND_EFFECT.to_string()],
            build_number: None,
        });
        let app = app_with(&recorder);
        let handle = app.handle().clone();
        let store = handle.state::<SettingsStore<tauri::test::MockRuntime>>();
        let run = |refresh| {
            tauri::async_runtime::block_on(reconcile(handle.clone(), "main-1".into(), refresh))
        };

        // Off by default: only the frame's inset is set, once.
        run(true);
        run(false);
        assert_eq!(calls(&recorder), ["inset 8"]);

        // Switched on: blur goes behind the window.
        store.set(lit(|_| {})).unwrap();
        run(false);
        run(false);
        assert_eq!(calls(&recorder), ["apply Blur dark=false"]);

        // The person forces high contrast: the effect is taken away.
        store
            .set(settings(|s| {
                s.accessibility.high_contrast = OsPreference::On
            }))
            .unwrap();
        run(false);
        assert_eq!(calls(&recorder), ["clear"]);

        // Back on, and a dark mode chosen: the effect is asked for again, dark.
        store
            .set(lit(|s| s.appearance.mode = ColourMode::Dark))
            .unwrap();
        run(false);
        assert_eq!(calls(&recorder), ["apply Blur dark=true"]);

        // Blur turned Off is the same as off.
        store
            .set(lit(|s| s.transparency.blur = BlurLevel::Off))
            .unwrap();
        run(false);
        assert_eq!(calls(&recorder), ["clear"]);
    }

    #[test]
    fn a_window_that_is_gone_is_forgotten_and_a_system_without_blur_is_never_asked() {
        let recorder = Arc::new(Recorder::default());
        *locked(&recorder.env) = Some(Environment {
            session_type: SessionType::Wayland,
            desktop: Desktop::Gnome,
            has_composite: true,
            wayland_globals: Vec::new(),
            build_number: None,
        });
        let app = app_with(&recorder);
        let handle = app.handle().clone();
        let store = handle.state::<SettingsStore<tauri::test::MockRuntime>>();
        store.set(lit(|_| {})).unwrap();
        tauri::async_runtime::block_on(reconcile(handle.clone(), "main-1".into(), true));
        assert_eq!(calls(&recorder), ["inset 8"]);
        tauri::async_runtime::block_on(reconcile(handle.clone(), "main-9".into(), false));
        assert!(calls(&recorder).is_empty());
    }
}

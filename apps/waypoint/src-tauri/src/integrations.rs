// Carries Waypoint's work out to the desktop: notifications, progress on the launcher, a sleep inhibitor, the file manager name and a global shortcut
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// This is the one place that consumes the shared `xdg-portal` and `desktop-integration` plugins, from
// Rust only (A65): nothing here is reachable from a page except the two status commands. Every
// integration is off until the person enables it under Settings → Integrations (D118), and each one
// is only attempted where the plugins' own statuses say it works.
//
// The shape is one `reconcile`: it reads the settings, the queue and the plugins' statuses, works
// out what the world should look like (`integration_policy`, all pure), and makes the difference.
// Anything that could change the answer calls it — an operations event, a settings change, a tick
// while a job runs — and one lock lets a late call do nothing harmful, because it reads the
// latest state rather than acting on the event that woke it. Progress ticks are not events (the
// operations plugin sends them to the windows that subscribed), so while a job is in flight and
// the launcher shows progress, a half-second tick calls `reconcile` too.
//
// - Notifications (D121): a job that finished or failed, or stopped to ask a question, notifies
//   under the policy in `integration_policy::notice`. Clicking the notification raises a window.
// - Progress: one combined value over every job in flight, on the launcher entry (Linux) or the main
//   window's taskbar button (Windows).
// - Sleep inhibitor: held from the first job to start until none is working, and released on exit.
// - File manager: while on, the process owns `org.freedesktop.FileManager1`; each call opens its
//   folders in new tabs, and `ShowItems` selects the item. The tabs are opened through the session
//   store, the same commands the page's tab strip sends, so every window hears them as usual.
// - Global shortcut: raises the most recently focused main window, or opens one.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use tauri::async_runtime::{spawn, Mutex};
use tauri::{AppHandle, Emitter, Listener, Manager, Runtime, WindowEvent};
use tauri_plugin_desktop_integration::models::{
    ActionButton as DesktopButton, FileManagerCall, LauncherProgress, LauncherRequest,
    NotifyRequest, PluginStatus as DesktopStatus, SleepInhibitRequest, SleepKind,
};
use tauri_plugin_desktop_integration::DesktopServicesExt;
use tauri_plugin_waypoint_ops::Ops;
use tauri_plugin_waypoint_session::Sessions;
use tauri_plugin_waypoint_settings::SettingsStore;
use tauri_plugin_xdg_portal::models::{
    ActionButton as PortalButton, InhibitKind, InhibitRequest, NotificationRequest, PortalStatus,
};
use tauri_plugin_xdg_portal::PortalExt;
use waypoint_ops::{JobId, Resolution};
use waypoint_protocol::{
    Availability, IntegrationAvailability, Location, PluginStatus, WindowKind,
};
use waypoint_session::{Command, SessionEvent, TabHints};
use waypoint_settings::{Settings, DEFAULT_ACCELERATOR};

use crate::integration_policy::{
    actions_work, availability, combined_progress, command_for, desktop_summary,
    drive_inhibitor as policy_drive_inhibitor, facts, inhibit_route, keeps_awake, notify_route,
    open_requests, portal_summary, request_is_gone, transition_buttons, with_refused_inhibit,
    ActionCommand, InhibitAction, InhibitBackend, Inhibitor, JobFacts, Notice, OpenRequest,
    Platform, Route, Shown, Tracker, RAISE_ACTION,
};
use crate::ops_window;

/// What the shell tells a window when a notification's "Show" is pressed: the job's id. The
/// window that holds the job's question brings it back (`OpsResolverHost`).
pub const SHOW_JOB_EVENT: &str = "waypoint://show-job";

/// How often the progress on the launcher is refreshed while a job is in flight.
const TICK: Duration = Duration::from_millis(500);

/// What the inhibitor is told the app is doing.
const INHIBIT_REASON: &str = "Waypoint is working on files";

/// The id the Windows hotkey thread reports the shortcut by.
#[cfg(target_os = "windows")]
const SHORTCUT_ID: &str = "waypoint.raise";

/// The Wayland portal session that holds the shortcut, and what the compositor shows for it. A
/// reverse-DNS id, as the plugin asks, so it cannot collide with another app's.
#[cfg(target_os = "linux")]
const SHORTCUT_SESSION: &str = "ca.liminalhq.waypoint.raise";
#[cfg(target_os = "linux")]
const SHORTCUT_DESCRIPTION: &str = "Show Waypoint";

/// A held inhibitor: which plugin made it, and its handle.
type Handle = (Route, u32);

/// What the plugins reported when they were first needed. Probing talks to D-Bus, so it waits until
/// an integration is on, and the answer is kept for the run.
struct Probes {
    portal: PortalStatus,
    desktop: DesktopStatus,
    availability: IntegrationAvailability,
}

#[derive(Default)]
struct State {
    tracker: Tracker,
    inhibitor: Inhibitor<Handle>,
    /// What the launcher shows now; never sent is the same as cleared.
    shown: Option<Shown>,
    /// A tick task is running.
    ticking: bool,
    probes: Option<Probes>,
    /// Whether the file manager name is owned (as far as the last attempt knows).
    owns_file_manager: bool,
    /// The accelerator registered now.
    shortcut: Option<String>,
    /// A shortcut has been registered this run, so a later one is an update (Linux).
    #[cfg(target_os = "linux")]
    shortcut_started: bool,
}

/// The integrations' state: one lock around everything `reconcile` touches, and the window that was
/// in front last, which a window event writes without waiting for the lock.
pub struct Integrations {
    gate: Mutex<State>,
    recent: StdMutex<Option<String>>,
    /// The backend refused the sleep inhibitor this run. Read without the gate, which is held
    /// across D-Bus calls, so the availability command never waits on them.
    inhibit_refused: AtomicBool,
}

impl Integrations {
    fn new() -> Self {
        Self {
            gate: Mutex::new(State::default()),
            recent: StdMutex::new(None),
            inhibit_refused: AtomicBool::new(false),
        }
    }

    fn recent(&self) -> Option<String> {
        self.recent
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

// ---------------------------------------------------------------------------------------------
// Reading the world
// ---------------------------------------------------------------------------------------------

fn settings_now<R: Runtime>(app: &AppHandle<R>) -> Settings {
    app.try_state::<SettingsStore<R>>()
        .map(|store| store.get())
        .unwrap_or_default()
}

fn jobs_now<R: Runtime>(app: &AppHandle<R>) -> Vec<JobFacts> {
    app.try_state::<Ops<R>>()
        .map(|ops| ops.snapshot().jobs.iter().map(facts).collect())
        .unwrap_or_default()
}

/// Whether any Waypoint window has focus: a main window, the Shelf, Settings, a Properties window.
fn any_window_focused<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.webview_windows()
        .values()
        .any(|window| window.is_focused().unwrap_or(false))
}

/// Whether a keyring can keep a volume's passphrase, from the `secrets` plugin's own status: the sentence it gives when it cannot (no keyring running, or locked) is the reason.
async fn keyring_availability<R: Runtime>(app: &AppHandle<R>) -> Availability {
    let Some(secrets) = app.try_state::<tauri_plugin_secrets::Secrets>() else {
        return Availability::no("The keyring is not set up.");
    };
    let status = secrets.get_status().await;
    if status.has(tauri_plugin_secrets::FEATURE_STORE) {
        Availability::yes()
    } else {
        Availability::no(
            status
                .message
                .unwrap_or_else(|| "No keyring is available.".to_string()),
        )
    }
}

async fn probe<R: Runtime>(app: &AppHandle<R>) -> Probes {
    let portal = app.portal().status().await;
    let desktop = app.desktop_services().status().await;
    let keyring = keyring_availability(app).await;
    let availability = availability(Platform::current(), &portal, &desktop, keyring);
    Probes {
        portal,
        desktop,
        availability,
    }
}

// ---------------------------------------------------------------------------------------------
// Making the difference
// ---------------------------------------------------------------------------------------------

/// Shows a notification through the plugin the policy chose.
async fn show_notice<R: Runtime>(app: &AppHandle<R>, route: Route, notice: Notice) {
    // Buttons are the plugins' `actions`; none is the same as the field being absent.
    let has = !notice.actions.is_empty();
    let portal_buttons = has.then(|| {
        notice
            .actions
            .iter()
            .map(|b| PortalButton {
                id: b.id.clone(),
                label: b.label.clone(),
            })
            .collect::<Vec<_>>()
    });
    let desktop_buttons = has.then(|| {
        notice
            .actions
            .iter()
            .map(|b| DesktopButton {
                id: b.id.clone(),
                label: b.label.clone(),
            })
            .collect::<Vec<_>>()
    });
    let result = match route {
        Route::Portal => app
            .portal()
            .send_notification(NotificationRequest {
                id: notice.id,
                title: notice.title,
                body: Some(notice.body),
                default_action: Some(RAISE_ACTION.to_owned()),
                urgency: None,
                actions: portal_buttons,
            })
            .await
            .map_err(|e| e.to_string()),
        Route::Desktop => app
            .desktop_services()
            .notify(NotifyRequest {
                id: notice.id,
                title: notice.title,
                body: Some(notice.body),
                default_action: Some(RAISE_ACTION.to_owned()),
                urgency: None,
                app_name: None,
                desktop_id: None,
                actions: desktop_buttons,
            })
            .await
            .map_err(|e| e.to_string()),
    };
    if let Err(e) = result {
        log::warn!("could not show a notification: {e}");
    }
}

async fn show_progress<R: Runtime>(app: &AppHandle<R>, shown: Shown) {
    let progress = match shown {
        Shown::Cleared => LauncherProgress::Cleared,
        Shown::Indeterminate => LauncherProgress::Indeterminate,
        Shown::Percent(percent) => LauncherProgress::Value {
            value: f64::from(percent) / 100.0,
        },
    };
    let request = LauncherRequest {
        progress,
        count: None,
        // The bundle identifier, which is also the `.desktop` file's id.
        desktop_id: None,
        window_label: None,
    };
    if let Err(e) = app.desktop_services().set_launcher_progress(request).await {
        log::warn!("could not set the launcher progress: {e}");
    }
}

async fn acquire<R: Runtime>(app: &AppHandle<R>, route: Route) -> Result<Handle, String> {
    match route {
        Route::Portal => app
            .portal()
            .inhibit(InhibitRequest {
                reason: INHIBIT_REASON.to_owned(),
                kinds: vec![InhibitKind::Suspend],
            })
            .await
            .map(|h| (route, h.handle))
            .map_err(|e| e.to_string()),
        Route::Desktop => app
            .desktop_services()
            .inhibit_sleep(SleepInhibitRequest {
                reason: INHIBIT_REASON.to_owned(),
                kinds: vec![SleepKind::Sleep],
            })
            .await
            .map(|h| (route, h.handle))
            .map_err(|e| e.to_string()),
    }
}

/// The portal and `desktop-integration`, as the inhibitor's policy sees them.
struct AppBackend<'a, R: Runtime>(&'a AppHandle<R>);

impl<R: Runtime> InhibitBackend for AppBackend<'_, R> {
    type Handle = Handle;

    fn route_of((route, _): Handle) -> Route {
        route
    }

    async fn acquire(&self, route: Route) -> Result<Handle, String> {
        acquire(self.0, route).await
    }

    async fn release(&self, (route, handle): Handle) -> Result<(), String> {
        match route {
            Route::Portal => self
                .0
                .portal()
                .release_inhibit(handle)
                .await
                .map_err(|e| e.to_string()),
            Route::Desktop => self
                .0
                .desktop_services()
                .release_sleep_inhibit(handle)
                .await
                .map_err(|e| e.to_string()),
        }
    }
}

/// Gives a held inhibitor back at exit, where nothing is asked for again. A request the backend
/// already dropped (a refused inhibitor) is not worth a warning.
async fn release_at_exit<R: Runtime>(app: &AppHandle<R>, held: Handle) {
    if let Err(e) = AppBackend(app).release(held).await {
        if held.0 != Route::Portal || !request_is_gone(&e) {
            log::warn!("could not release the sleep inhibitor: {e}");
        }
    }
}

/// Follows the wish to keep the machine awake (see `integration_policy::drive_inhibitor`), and
/// tells the Services panel once the backend has refused.
async fn drive_inhibitor<R: Runtime>(
    app: &AppHandle<R>,
    st: &mut State,
    wanted: bool,
    route: Option<Route>,
) {
    policy_drive_inhibitor(&AppBackend(app), &mut st.inhibitor, wanted, route).await;
    if st.inhibitor.is_refused() {
        if let Some(integrations) = app.try_state::<Integrations>() {
            integrations.inhibit_refused.store(true, Ordering::Relaxed);
        }
    }
}

/// Takes or gives up the file manager name to match the setting.
async fn drive_file_manager<R: Runtime>(app: &AppHandle<R>, st: &mut State, wanted: bool) {
    if wanted == st.owns_file_manager {
        return;
    }
    let result = if wanted {
        app.desktop_services().own_file_manager().await
    } else {
        app.desktop_services().disown_file_manager().await
    };
    match result {
        // Whatever happened, do not try again until the setting changes: a name another file
        // manager holds would otherwise be asked for on every job event.
        Ok(ownership) => {
            if wanted && !ownership.owned {
                log::warn!(
                    "Waypoint could not take the file manager name: {}",
                    ownership.reason.unwrap_or_default()
                );
            }
            st.owns_file_manager = wanted;
        }
        Err(e) => {
            log::warn!("could not change the file manager name: {e}");
            st.owns_file_manager = wanted;
        }
    }
}

/// Registers, rebinds or removes the global shortcut to match the setting.
#[cfg(target_os = "linux")]
async fn drive_shortcut<R: Runtime>(app: &AppHandle<R>, st: &mut State, wish: Option<String>) {
    use tauri_plugin_desktop_integration::DesktopIntegrationExt;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    if wish == st.shortcut {
        return;
    }
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    match &wish {
        Some(accelerator) => {
            let handle = app.clone();
            let on_pressed = move || raise_if_enabled(&handle);
            if st.shortcut_started {
                app.update_shortcut(accelerator, on_pressed);
            } else {
                app.register_shortcut(
                    SHORTCUT_SESSION,
                    SHORTCUT_DESCRIPTION,
                    accelerator,
                    on_pressed,
                );
                st.shortcut_started = true;
                if wayland {
                    // The portal's dialog needs a window to belong to.
                    if let Some(window) = recent_or_first_window(app) {
                        app.set_shortcut_window(&window);
                    }
                }
            }
        }
        // The Wayland portal session cannot be closed from here; the key press is ignored while
        // the setting is off (`raise_if_enabled`) and the binding is gone at the next start.
        None if !wayland => {
            if let Err(e) = app.global_shortcut().unregister_all() {
                log::warn!("could not remove the global shortcut: {e}");
            }
        }
        None => {}
    }
    st.shortcut = wish;
}

#[cfg(target_os = "windows")]
async fn drive_shortcut<R: Runtime>(app: &AppHandle<R>, st: &mut State, wish: Option<String>) {
    use tauri_plugin_desktop_integration::models::GlobalShortcutRequest;

    if wish == st.shortcut {
        return;
    }
    let result = match &wish {
        Some(accelerator) => {
            app.desktop_services()
                .register_global_shortcut(GlobalShortcutRequest {
                    id: SHORTCUT_ID.to_owned(),
                    accelerator: accelerator.clone(),
                })
                .await
        }
        None => {
            app.desktop_services()
                .unregister_global_shortcut(SHORTCUT_ID.to_owned())
                .await
        }
    };
    match result {
        Ok(()) => {}
        // Not asked again until the setting changes: a hotkey another program holds would
        // otherwise be retried on every job event.
        Err(e) => log::warn!("could not change the global shortcut: {e}"),
    }
    st.shortcut = wish;
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
async fn drive_shortcut<R: Runtime>(_app: &AppHandle<R>, _st: &mut State, _wish: Option<String>) {}

/// Works out what the world should look like and makes it so. Returns whether a job is in flight
/// with progress to show, which is when a tick is worth scheduling.
async fn reconcile_locked<R: Runtime>(app: &AppHandle<R>, st: &mut State) -> bool {
    let settings = settings_now(app);
    let wanted = settings.integrations;
    let jobs = jobs_now(app);
    let transitions = st.tracker.observe(&jobs);

    let any_on = wanted.notifications
        || wanted.launcher_progress
        || wanted.prevent_sleep
        || wanted.default_file_manager
        || wanted.global_shortcut_enabled;
    if any_on && st.probes.is_none() {
        st.probes = Some(probe(app).await);
    }
    let (notify_via, inhibit_via, works, buttons_work) = match &st.probes {
        Some(p) => {
            let route = notify_route(&p.portal, &p.desktop);
            (
                route,
                inhibit_route(&p.portal, &p.desktop),
                Some(p.availability.clone()),
                route.is_some_and(|route| actions_work(route, &p.portal, &p.desktop)),
            )
        }
        None => (None, None, None, false),
    };
    let can = |pick: fn(&IntegrationAvailability) -> bool| works.as_ref().is_some_and(pick);

    if wanted.notifications {
        if let Some(route) = notify_via {
            let focused = any_window_focused(app);
            // Buttons only where the person wants them and the route accepts them; the click on
            // the notification does the default either way.
            let with_buttons = wanted.notification_actions && buttons_work;
            let undo_top = if with_buttons {
                undo_top_job(app)
            } else {
                None
            };
            for transition in &transitions {
                if let Some(mut notice) = crate::integration_policy::notice(transition, focused) {
                    if with_buttons {
                        notice.actions = transition_buttons(transition, undo_top);
                    }
                    show_notice(app, route, notice).await;
                }
            }
        }
    }

    let progress = if wanted.launcher_progress && can(|a| a.launcher_progress.available) {
        combined_progress(&jobs)
    } else {
        Shown::Cleared
    };
    if st.shown.unwrap_or(Shown::Cleared) != progress {
        show_progress(app, progress).await;
        st.shown = Some(progress);
    }

    let keep_awake =
        wanted.prevent_sleep && can(|a| a.prevent_sleep.available) && keeps_awake(&jobs);
    drive_inhibitor(app, st, keep_awake, inhibit_via).await;

    let file_manager = wanted.default_file_manager && can(|a| a.file_manager_service.available);
    drive_file_manager(app, st, file_manager).await;

    let shortcut =
        (wanted.global_shortcut_enabled && can(|a| a.global_shortcut.available)).then(|| {
            wanted
                .global_shortcut
                .clone()
                .unwrap_or_else(|| DEFAULT_ACCELERATOR.to_owned())
        });
    drive_shortcut(app, st, shortcut).await;

    progress != Shown::Cleared
}

/// Reconciles now, and keeps ticking while there is progress to show.
async fn reconcile<R: Runtime>(app: AppHandle<R>) {
    let Some(integrations) = app.try_state::<Integrations>() else {
        return;
    };
    let mut st = integrations.gate.lock().await;
    let busy = reconcile_locked(&app, &mut st).await;
    if busy && !st.ticking {
        st.ticking = true;
        let handle = app.clone();
        spawn(async move { tick(handle).await });
    }
}

/// Refreshes the launcher's progress until nothing is in flight. Deciding to stop and clearing the
/// flag happen under the same lock `reconcile` takes, so a job that starts just as the loop ends
/// starts a new one.
async fn tick<R: Runtime>(app: AppHandle<R>) {
    loop {
        tokio::time::sleep(TICK).await;
        let Some(integrations) = app.try_state::<Integrations>() else {
            return;
        };
        let mut st = integrations.gate.lock().await;
        if !reconcile_locked(&app, &mut st).await {
            st.ticking = false;
            return;
        }
    }
}

fn reconcile_soon<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    spawn(async move { reconcile(handle).await });
}

// ---------------------------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------------------------

fn is_main(label: &str) -> bool {
    WindowKind::from_label(label) == Some(WindowKind::Main)
}

/// The main window that was in front most recently, else the first main window by label.
fn recent_or_first_window<R: Runtime>(app: &AppHandle<R>) -> Option<tauri::WebviewWindow<R>> {
    if let Some(window) = app
        .try_state::<Integrations>()
        .and_then(|i| i.recent())
        .and_then(|label| app.get_webview_window(&label))
    {
        return Some(window);
    }
    let mut mains: Vec<_> = app
        .webview_windows()
        .into_iter()
        .filter(|(label, _)| is_main(label))
        .collect();
    mains.sort_by(|a, b| a.0.cmp(&b.0));
    mains.into_iter().next().map(|(_, window)| window)
}

/// Brings a window to the front, from wherever it is: shown, unminimised, then focused. On X11 the
/// window manager is given a fresh user time first, as it asks of a window that wants focus.
fn bring_forward<R: Runtime>(app: &AppHandle<R>, window: &tauri::WebviewWindow<R>) {
    use tauri_plugin_desktop_integration::DesktopIntegrationExt;
    let _ = window.show();
    let _ = window.unminimize();
    app.request_desktop_activation_assist(window, "integrations", window.label());
    let _ = window.set_focus();
}

/// Raises the most recent Waypoint window, or opens a new one when there is none.
pub fn raise_or_open<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = recent_or_first_window(app) {
        bring_forward(app, &window);
        return;
    }
    let Some(sessions) = app.try_state::<Sessions<R>>() else {
        return;
    };
    // No main window is showing: ask the session for a new one from the first it knows, if any.
    let from = sessions.with_store(|s| s.windows().first().map(|w| w.label.clone()));
    let Some(from) = from else {
        log::warn!("no window to raise and none to open one from");
        return;
    };
    if let Err(e) = sessions.run(
        app,
        &from,
        Command::OpenWindow {
            location: None,
            geometry: None,
        },
    ) {
        log::warn!("could not open a window: {e}");
    }
}

/// The shortcut's action, unless the setting was turned off since it was registered (the Wayland
/// portal session cannot be dropped while the app runs).
fn raise_if_enabled<R: Runtime>(app: &AppHandle<R>) {
    if settings_now(app).integrations.global_shortcut_enabled {
        raise_or_open(app);
    }
}

/// Records which main window was in front last, for the shortcut and the notification click. Call
/// from the app's window event handler.
pub fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    if let WindowEvent::Focused(true) = event {
        if is_main(window.label()) {
            if let Some(integrations) = window.app_handle().try_state::<Integrations>() {
                *integrations
                    .recent
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some(window.label().to_owned());
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// FileManager1
// ---------------------------------------------------------------------------------------------

/// Opens what a `FileManager1` call asked for, each folder in a new tab of the window in front.
fn open_in_tabs<R: Runtime>(app: &AppHandle<R>, requests: &[OpenRequest]) {
    let Some(sessions) = app.try_state::<Sessions<R>>() else {
        return;
    };
    let Some(window) = recent_or_first_window(app) else {
        // Nothing to open into; the call came while no window was showing. Open a window at the
        // first folder, from the first window the session knows.
        let from = sessions.with_store(|s| s.windows().first().map(|w| w.label.clone()));
        if let (Some(from), Some(first)) = (from, requests.first()) {
            let outcome = sessions.run(
                app,
                &from,
                Command::OpenWindow {
                    location: Some(first.folder.to_location()),
                    geometry: None,
                },
            );
            if let Err(e) = outcome {
                log::warn!("could not open a window for a file manager call: {e}");
            }
        }
        return;
    };
    let label = window.label().to_owned();
    for request in requests {
        let outcome = sessions.run(
            app,
            &label,
            Command::Open {
                location: request.folder.to_location(),
                after: None,
                activate: true,
            },
        );
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(e) => {
                log::warn!("could not open {}: {e}", request.folder.display());
                continue;
            }
        };
        let Some(name) = &request.select else {
            continue;
        };
        // The same hint the Shelf's Reveal sets: the tab opens with this entry focused.
        let tab = outcome.events_for(&label).find_map(|event| match event {
            SessionEvent::TabOpened { tab, .. } => Some(tab.id),
            _ => None,
        });
        if let Some(tab) = tab {
            let hints = TabHints {
                scroll_top: 0,
                focused: Some(name.clone()),
            };
            if let Err(e) = sessions.run(app, &label, Command::SetHints { tab, hints }) {
                log::warn!("could not select {name}: {e}");
            }
        }
    }
    bring_forward(app, &window);
}

fn on_file_manager_call<R: Runtime>(app: &AppHandle<R>, payload: &str) {
    // A call only matters while the setting is on; an event that outlives the setting is dropped.
    if !settings_now(app).integrations.default_file_manager {
        return;
    }
    let call: FileManagerCall = match serde_json::from_str(payload) {
        Ok(call) => call,
        Err(e) => {
            log::warn!("could not read a file manager call: {e}");
            return;
        }
    };
    let requests = open_requests(&call);
    if requests.is_empty() {
        log::info!("a file manager call named nothing Waypoint can open");
        return;
    }
    open_in_tabs(app, &requests);
}

// ---------------------------------------------------------------------------------------------
// Notification buttons
// ---------------------------------------------------------------------------------------------

/// The job whose journal entry the undo journal would undo now: the newest applied entry, when a
/// job on the queue made it. A move or a trash is only offered "Undo" while it is still this one.
fn undo_top_job<R: Runtime>(app: &AppHandle<R>) -> Option<u64> {
    let ops = app.try_state::<Ops<R>>()?;
    let snapshot = ops.snapshot();
    let top = snapshot.journal.undo?.id;
    snapshot
        .jobs
        .iter()
        .find(|job| ops.journal_entry_of(job.id) == Some(top))
        .map(|job| job.id.0)
}

/// An open main window by label, if it is one.
fn open_main<R: Runtime>(app: &AppHandle<R>, label: &str) -> Option<tauri::WebviewWindow<R>> {
    if !is_main(label) {
        return None;
    }
    app.get_webview_window(label)
}

/// Opens a folder as a tab in the window that started the job, or in a new window when that one
/// has gone.
fn open_location<R: Runtime>(app: &AppHandle<R>, origin: &str, location: Location) {
    let Some(sessions) = app.try_state::<Sessions<R>>() else {
        return;
    };
    let Some(window) = open_main(app, origin) else {
        let from = sessions.with_store(|s| s.windows().first().map(|w| w.label.clone()));
        let Some(from) = from else {
            log::warn!("no window to open a folder from");
            return;
        };
        let opened = sessions.run(
            app,
            &from,
            Command::OpenWindow {
                location: Some(location),
                geometry: None,
            },
        );
        if let Err(e) = opened {
            log::warn!("could not open a window for a notification button: {e}");
        }
        return;
    };
    let opened = sessions.run(
        app,
        window.label(),
        Command::Open {
            location,
            after: None,
            activate: true,
        },
    );
    if let Err(e) = opened {
        log::warn!("could not open a tab for a notification button: {e}");
    }
    bring_forward(app, &window);
}

/// Opens the Operations window; where that fails, raises a window so the press is never silent.
fn open_operations<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = ops_window::open(app) {
        log::warn!("could not open the Operations window: {e}");
        raise_or_open(app);
    }
}

/// Brings the question a job waits on in front: the window that started the job when it is open
/// (its dialog is there), else the Operations window, which lists it with "Resolve…".
fn show_question<R: Runtime>(app: &AppHandle<R>, job: u64, origin: &str) {
    let target = match open_main(app, origin) {
        Some(window) => {
            bring_forward(app, &window);
            window.label().to_owned()
        }
        None => {
            open_operations(app);
            ops_window::OPS_LABEL.to_owned()
        }
    };
    if let Err(e) = app.emit_to(target.as_str(), SHOW_JOB_EVENT, job) {
        log::warn!("could not ask a window to show job {job}: {e}");
    }
}

/// Does what a pressed button asks. Every failure ends in the Operations window, so a press that
/// cannot do its work still shows the person where the job is.
fn run_command<R: Runtime>(app: &AppHandle<R>, command: ActionCommand) {
    match command {
        ActionCommand::OpenFolder { window, location } => open_location(app, &window, location),
        ActionCommand::OpenOperations => open_operations(app),
        ActionCommand::ShowQuestion { job, window } => show_question(app, job, &window),
        ActionCommand::Resolve {
            job,
            source,
            policy,
        } => {
            let Some(ops) = app.try_state::<Ops<R>>() else {
                return;
            };
            let answer = Resolution {
                source: Some(source),
                policy,
            };
            if let Err(e) = ops.resolve(JobId(job), vec![answer], None) {
                log::warn!("could not answer job {job} from a notification: {e}");
                open_operations(app);
            }
        }
        ActionCommand::Undo { job, window } => {
            let Some(ops) = app.try_state::<Ops<R>>() else {
                return;
            };
            let label = open_main(app, &window)
                .map(|w| w.label().to_owned())
                .or_else(|| recent_or_first_window(app).map(|w| w.label().to_owned()))
                .unwrap_or(window);
            let entry = ops.journal_entry_of(JobId(job));
            if let Err(e) = ops.undo(&label, entry) {
                log::warn!("could not undo job {job} from a notification: {e}");
                open_operations(app);
            }
        }
    }
}

/// A click on a notification (the default action) or one of its buttons. The click raises a window;
/// a button is read against the queue as it is now, so a press that arrives late is still safe.
fn on_notification_action<R: Runtime>(app: &AppHandle<R>, action: &str) {
    if action == RAISE_ACTION {
        raise_or_open(app);
        return;
    }
    let jobs = jobs_now(app);
    let undo_top = undo_top_job(app);
    let Some(command) = command_for(action, &jobs, undo_top) else {
        // Not one of ours: another application's id, or a malformed one.
        log::debug!("ignored a notification action Waypoint did not send");
        return;
    };
    let handle = app.clone();
    // Opening a window is not done from the event thread (Windows requires it, and the Operations
    // window builds one).
    spawn(async move { run_command(&handle, command) });
}

// ---------------------------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------------------------

/// Starts listening. Call once from `setup`, after the plugins this reaches.
pub fn wire<R: Runtime>(app: &AppHandle<R>) {
    let integrations = Integrations::new();
    app.manage(integrations);

    // Toasts need an AppUserModelID, and it must be set before the first one.
    #[cfg(target_os = "windows")]
    if let Err(e) = app
        .desktop_services()
        .set_app_user_model_id(&app.config().identifier)
    {
        log::warn!("could not set the AppUserModelID: {e}");
    }

    // Jobs that are already in the queue are not news.
    let seeded = jobs_now(app);
    if let Some(integrations) = app.try_state::<Integrations>() {
        // Nothing else has the lock yet.
        if let Ok(mut st) = integrations.gate.try_lock() {
            st.tracker.seed(&seeded);
        }
    }

    let handle = app.clone();
    app.listen(tauri_plugin_waypoint_ops::EVENT, move |_| {
        reconcile_soon(&handle)
    });

    if let Some(store) = app.try_state::<SettingsStore<R>>() {
        let handle = app.clone();
        store.on_change(move |_| reconcile_soon(&handle));
    }

    // A click on a notification or one of its buttons, through either plugin.
    for event in [
        tauri_plugin_xdg_portal::notification::ACTION_EVENT,
        tauri_plugin_desktop_integration::notify::ACTION_EVENT,
    ] {
        let handle = app.clone();
        app.listen(event, move |event| {
            #[derive(serde::Deserialize)]
            struct Action {
                action: String,
            }
            if let Ok(action) = serde_json::from_str::<Action>(event.payload()) {
                on_notification_action(&handle, &action.action);
            }
        });
    }

    let handle = app.clone();
    app.listen(
        tauri_plugin_desktop_integration::file_manager::CALL_EVENT,
        move |event| on_file_manager_call(&handle, event.payload()),
    );

    #[cfg(target_os = "windows")]
    {
        let handle = app.clone();
        app.listen(
            tauri_plugin_desktop_integration::shortcuts::PRESSED_EVENT,
            move |_| raise_if_enabled(&handle),
        );
    }

    // What the settings had when the app started.
    reconcile_soon(app);
}

/// The app is exiting: gives the sleep inhibitor back at once. The plugins do the same for
/// anything left, so this only makes the shell stop showing it sooner.
pub fn on_exit<R: Runtime>(app: &AppHandle<R>) {
    let Some(integrations) = app.try_state::<Integrations>() else {
        return;
    };
    tauri::async_runtime::block_on(async {
        let work = async {
            let mut st = integrations.gate.lock().await;
            if let InhibitAction::Release(handle) = st.inhibitor.release_all() {
                release_at_exit(app, handle).await;
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(1), work).await;
    });
}

// ---------------------------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------------------------

/// The shared plugins' statuses, for the Services panel (A66): plugins used only from Rust have no
/// guest-js `getStatus` of their own. Probed fresh, so the panel shows the system as it is now.
#[tauri::command]
pub async fn get_integration_statuses<R: Runtime>(
    app: AppHandle<R>,
) -> HashMap<String, PluginStatus> {
    let probes = probe(&app).await;
    HashMap::from([
        ("xdg-portal".to_owned(), portal_summary(&probes.portal)),
        (
            "desktop-integration".to_owned(),
            desktop_summary(&probes.desktop),
        ),
    ])
}

/// What each switch on the Integrations page can do here, with the reason where it cannot.
#[tauri::command]
pub async fn get_integration_availability<R: Runtime>(
    app: AppHandle<R>,
) -> IntegrationAvailability {
    let availability = probe(&app).await.availability;
    let refused = app
        .try_state::<Integrations>()
        .is_some_and(|i| i.inhibit_refused.load(Ordering::Relaxed));
    with_refused_inhibit(availability, refused)
}

#[cfg(test)]
mod live {
    //! Talks to the real session bus: run by hand with
    //! `cargo test -p waypoint live_ -- --ignored --nocapture`. It shows one notification and holds
    //! a sleep inhibitor for a few seconds (`systemd-inhibit --list` shows it meanwhile).

    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};

    fn app() -> tauri::App<MockRuntime> {
        mock_builder()
            .plugin(tauri_plugin_xdg_portal::init())
            .plugin(tauri_plugin_desktop_integration::init())
            .build(mock_context(noop_assets()))
            .expect("a mock app")
    }

    #[test]
    #[ignore = "talks to the real session bus and shows a notification"]
    fn live_statuses_notification_and_inhibitor() {
        let app = app();
        let handle = app.handle().clone();
        tauri::async_runtime::block_on(async {
            let probes = probe(&handle).await;
            println!("portal: {:#?}", probes.portal);
            println!("desktop: {:#?}", probes.desktop);
            println!("availability: {:#?}", probes.availability);
            let route = notify_route(&probes.portal, &probes.desktop);
            println!("notification route: {route:?}");
            if let Some(route) = route {
                show_notice(
                    &handle,
                    route,
                    Notice {
                        id: "waypoint-job-live".into(),
                        title: "Copy 3 items".into(),
                        body: "Finished.".into(),
                        // Whether the desktop draws them is what to look at; the click raises.
                        actions: vec![
                            crate::integration_policy::Button {
                                id: crate::integration_policy::button_id(
                                    0,
                                    crate::integration_policy::Act::ShowInFolder,
                                ),
                                label: "Show in folder".into(),
                            },
                            crate::integration_policy::Button {
                                id: crate::integration_policy::button_id(
                                    0,
                                    crate::integration_policy::Act::Undo,
                                ),
                                label: "Undo".into(),
                            },
                        ],
                    },
                )
                .await;
            }
            let route = inhibit_route(&probes.portal, &probes.desktop);
            println!("inhibit route: {route:?}");
            if let Some(route) = route {
                let held = acquire(&handle, route).await;
                println!("acquired: {held:?}");
                if let Ok(held) = held {
                    tokio::time::sleep(Duration::from_secs(12)).await;
                    release_at_exit(&handle, held).await;
                    println!("released");
                }
            }
        });
    }
}

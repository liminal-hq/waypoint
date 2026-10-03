// What the OS integrations decide, as pure functions: when to notify, the combined progress, the sleep inhibitor, and which request a file manager call becomes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `integrations.rs` is the glue that reads the settings, hears the operations plugin's events and
// calls the shared plugins; every decision it makes is here, over plain values, so each can be
// tested without an app, a bus or a window (A65).
//
// - Which service does a thing: the portal first where its status says it works, the zbus
//   services of `desktop-integration` as the fallback (`notify_route`, `inhibit_route`).
// - What each switch on the Integrations page can do here and why not (`availability`).
// - When a finished job notifies (D121): only while no Waypoint window has focus, when it ran over
//   ten seconds, or when a question is waiting; never when it was cancelled (`Tracker`, `notice`).
// - The one progress value over every job in flight (`combined_progress`).
// - Whether a job is working, so the machine is kept awake (`keeps_awake`), and the inhibitor's
//   lifecycle as a state machine (`Inhibitor`).
// - What a `FileManager1` call asks Waypoint to open (`open_requests`).

use std::collections::HashMap;

use tauri_plugin_desktop_integration::models::{
    Feature as DesktopFeature, FileManagerCall, FileManagerMethod, PluginStatus as DesktopStatus,
    UnavailableReason as DesktopReason,
};
use tauri_plugin_xdg_portal::models::{
    PortalFeature, PortalStatus, UnavailableReason as PortalReason,
};
use waypoint_ops::{JobSnapshot, JobState};
use waypoint_path::FilePath;
use waypoint_protocol::{Availability, IntegrationAvailability, PluginStatus};

/// A job that ran longer than this notifies even while a Waypoint window has focus (D121).
pub const LONG_JOB_MS: u64 = 10_000;

/// The most folders one `FileManager1` call opens: a stray call naming hundreds of items must not
/// open hundreds of tabs.
pub const MAX_OPEN_FOLDERS: usize = 8;

/// The action id a notification carries and the services report back when it is clicked.
pub const RAISE_ACTION: &str = "raise";

// ---------------------------------------------------------------------------------------------
// Which service does a thing, and what works here
// ---------------------------------------------------------------------------------------------

/// The operating system family the services are chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    /// macOS and anything else: no integration is offered.
    Other,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "linux") {
            Platform::Linux
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Other
        }
    }
}

/// Which plugin carries a call out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// `xdg-portal`: works the same inside a Flatpak.
    Portal,
    /// `desktop-integration`: zbus on Linux, the Windows APIs on Windows.
    Desktop,
}

fn portal_works(status: &PortalStatus, feature: PortalFeature) -> bool {
    status.feature(feature).is_some_and(|f| f.available)
}

fn desktop_works(status: &DesktopStatus, feature: DesktopFeature) -> bool {
    status.feature(feature).is_some_and(|f| f.available)
}

/// Notifications go through the portal where it offers them, else the notification server.
pub fn notify_route(portal: &PortalStatus, desktop: &DesktopStatus) -> Option<Route> {
    if portal_works(portal, PortalFeature::Notification) {
        Some(Route::Portal)
    } else if desktop_works(desktop, DesktopFeature::Notify) {
        Some(Route::Desktop)
    } else {
        None
    }
}

/// The sleep inhibitor goes through the portal where it offers one, else logind or Windows.
pub fn inhibit_route(portal: &PortalStatus, desktop: &DesktopStatus) -> Option<Route> {
    if portal_works(portal, PortalFeature::Inhibit) {
        Some(Route::Portal)
    } else if desktop_works(desktop, DesktopFeature::InhibitSleep) {
        Some(Route::Desktop)
    } else {
        None
    }
}

/// The sentence for why a `desktop-integration` feature does not work here.
fn desktop_reason(reason: Option<DesktopReason>) -> &'static str {
    match reason {
        Some(DesktopReason::PlatformUnsupported) | None => {
            "This operating system does not offer it."
        }
        Some(DesktopReason::NoSessionBus) => "There is no session bus to reach it on.",
        Some(DesktopReason::NoNotificationServer) => "No notification server is running.",
        Some(DesktopReason::NoLogind) => "systemd-logind is not running.",
        Some(DesktopReason::NeedsAppId) => {
            "Windows needs an application identity before it shows notifications."
        }
        Some(DesktopReason::NoDisplayServer) => "No display server is running.",
    }
}

/// The sentence for why the portal does not offer an interface.
fn portal_reason(reason: Option<PortalReason>) -> &'static str {
    match reason {
        Some(PortalReason::PlatformUnsupported) | None => "This operating system has no portal.",
        Some(PortalReason::NoPortal) => "No desktop portal is running.",
        Some(PortalReason::InterfaceMissing) => "The desktop portal does not offer it.",
        Some(PortalReason::NoResponse) => "The desktop portal did not answer.",
        Some(PortalReason::NotSandboxed) => {
            "The desktop portal answers only sandboxed apps, so Waypoint uses the system's own services."
        }
    }
}

fn desktop_availability(status: &DesktopStatus, feature: DesktopFeature) -> Availability {
    match status.feature(feature) {
        Some(f) if f.available => Availability::yes(),
        Some(f) => Availability::no(desktop_reason(f.reason)),
        None => Availability::no(desktop_reason(None)),
    }
}

/// A switch that works through either plugin: available when one does, and when neither does, the
/// fallback's reason.
fn either_availability(
    portal: &PortalStatus,
    portal_feature: PortalFeature,
    desktop: &DesktopStatus,
    desktop_feature: DesktopFeature,
) -> Availability {
    if portal_works(portal, portal_feature) {
        return Availability::yes();
    }
    // Without the portal, the fallback's answer stands, reason included: the portal's own reason is
    // usually "not sandboxed", which would mislead.
    desktop_availability(desktop, desktop_feature)
}

/// The portal's status in the shape the Services panel lists every plugin in (A66): the interfaces
/// that work, and the reason of the first that does not.
pub fn portal_summary(status: &PortalStatus) -> PluginStatus {
    let names = [
        (PortalFeature::Notification, "notification"),
        (PortalFeature::Inhibit, "inhibit"),
        (PortalFeature::OpenUri, "openUri"),
    ];
    let features = names
        .iter()
        .filter(|(feature, _)| portal_works(status, *feature))
        .map(|(_, name)| (*name).to_owned())
        .collect();
    let reason = status
        .features
        .iter()
        .find(|f| !f.available)
        .map(|f| portal_reason(f.reason).to_owned());
    PluginStatus {
        available: status.available,
        reason,
        features,
    }
}

/// `desktop-integration`'s status in the same shape.
pub fn desktop_summary(status: &DesktopStatus) -> PluginStatus {
    let names = [
        (DesktopFeature::Notify, "notify"),
        (DesktopFeature::InhibitSleep, "inhibitSleep"),
        (DesktopFeature::LauncherProgress, "launcherProgress"),
        (DesktopFeature::FileManager, "fileManager"),
        (DesktopFeature::GlobalShortcuts, "globalShortcuts"),
    ];
    let features = names
        .iter()
        .filter(|(feature, _)| desktop_works(status, *feature))
        .map(|(_, name)| (*name).to_owned())
        .collect();
    let reason = status
        .features
        .iter()
        .find(|f| !f.available)
        .map(|f| desktop_reason(f.reason).to_owned());
    PluginStatus {
        available: status.available,
        reason,
        features,
    }
}

/// What each switch on the Integrations page can do on this system.
pub fn availability(
    platform: Platform,
    portal: &PortalStatus,
    desktop: &DesktopStatus,
) -> IntegrationAvailability {
    if platform == Platform::Other {
        let none = || Availability::no("This operating system does not offer it.");
        return IntegrationAvailability {
            notifications: none(),
            launcher_progress: none(),
            prevent_sleep: none(),
            file_manager_service: none(),
            global_shortcut: none(),
        };
    }
    IntegrationAvailability {
        notifications: either_availability(
            portal,
            PortalFeature::Notification,
            desktop,
            DesktopFeature::Notify,
        ),
        launcher_progress: desktop_availability(desktop, DesktopFeature::LauncherProgress),
        prevent_sleep: either_availability(
            portal,
            PortalFeature::Inhibit,
            desktop,
            DesktopFeature::InhibitSleep,
        ),
        file_manager_service: desktop_availability(desktop, DesktopFeature::FileManager),
        global_shortcut: desktop_availability(desktop, DesktopFeature::GlobalShortcuts),
    }
}

// ---------------------------------------------------------------------------------------------
// Jobs, as far as the integrations care
// ---------------------------------------------------------------------------------------------

/// Where a job is in its life, without the detail the integrations never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Planning,
    Queued,
    Running,
    Paused,
    /// Stopped for an answer from the person.
    Waiting,
    Cancelling,
    Cancelled,
    Done,
    Failed,
}

impl Phase {
    /// Nothing more happens to the job except being dismissed.
    pub fn is_finished(self) -> bool {
        matches!(self, Phase::Cancelled | Phase::Done | Phase::Failed)
    }

    /// The job is in flight: it counts toward the progress shown on the icon.
    fn in_flight(self) -> bool {
        !self.is_finished()
    }
}

/// What the policies read of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobFacts {
    pub id: u64,
    pub phase: Phase,
    /// "Copy 3 items": the job's own plain description, used as a notification's title.
    pub title: String,
    pub items_done: u64,
    pub items_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    /// How long the job ran, once it has started and finished.
    pub ran_ms: Option<u64>,
    pub skipped: u64,
    pub failed: u64,
    /// The sentence a failed job ended with.
    pub error: Option<String>,
}

/// The facts of a job snapshot.
pub fn facts(job: &JobSnapshot) -> JobFacts {
    let (phase, error) = match &job.state {
        JobState::Planning => (Phase::Planning, None),
        JobState::Queued => (Phase::Queued, None),
        JobState::Running => (Phase::Running, None),
        JobState::Paused => (Phase::Paused, None),
        JobState::Waiting { .. } => (Phase::Waiting, None),
        JobState::Cancelling => (Phase::Cancelling, None),
        JobState::Cancelled => (Phase::Cancelled, None),
        JobState::Done => (Phase::Done, None),
        JobState::Failed { error, .. } => (Phase::Failed, Some(error.to_string())),
    };
    let ran_ms = match (job.started_ms, job.finished_ms) {
        (Some(start), Some(end)) => Some(end.saturating_sub(start).max(0) as u64),
        _ => None,
    };
    JobFacts {
        id: job.id.0,
        phase,
        title: job.title.clone(),
        items_done: job.progress.items_done,
        items_total: job.progress.items_total,
        bytes_done: job.progress.bytes_done,
        bytes_total: job.progress.bytes_total,
        ran_ms,
        skipped: job.counts.skipped,
        failed: job.counts.failed,
        error,
    }
}

// ---------------------------------------------------------------------------------------------
// Progress on the launcher entry or the taskbar button
// ---------------------------------------------------------------------------------------------

/// What the dock or taskbar icon shows. A percentage, so a tick that changes nothing visible is
/// the same value and is not sent again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Cleared,
    /// Busy, with no known end.
    Indeterminate,
    /// Whole percent, 0 to 100.
    Percent(u8),
}

/// One value over every job in flight: the bytes done over the bytes to do (the items where a job
/// has no bytes). Indeterminate while nothing in flight has a total yet (every job is planning),
/// and cleared when nothing is in flight.
pub fn combined_progress(jobs: &[JobFacts]) -> Shown {
    let mut any = false;
    let (mut done, mut total) = (0u128, 0u128);
    for job in jobs.iter().filter(|job| job.phase.in_flight()) {
        any = true;
        let (d, t) = if job.bytes_total > 0 {
            (job.bytes_done.min(job.bytes_total), job.bytes_total)
        } else {
            (job.items_done.min(job.items_total), job.items_total)
        };
        done += u128::from(d);
        total += u128::from(t);
    }
    if !any {
        return Shown::Cleared;
    }
    if total == 0 {
        return Shown::Indeterminate;
    }
    // Rounded down, so the bar never reads 100 before the last job has finished.
    Shown::Percent(((done * 100) / total).min(100) as u8)
}

// ---------------------------------------------------------------------------------------------
// The sleep inhibitor
// ---------------------------------------------------------------------------------------------

/// Whether the machine should be kept awake: a job is planning, running or unwinding. A job that is
/// queued, paused or waiting for an answer is not working, so it does not keep the machine up.
pub fn keeps_awake(jobs: &[JobFacts]) -> bool {
    jobs.iter().any(|job| {
        matches!(
            job.phase,
            Phase::Planning | Phase::Running | Phase::Cancelling
        )
    })
}

/// What the inhibitor needs done now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InhibitAction<H> {
    None,
    /// Ask the service for an inhibitor, then report `acquired` or `failed`.
    Acquire,
    /// Give this one back.
    Release(H),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held<H> {
    Free,
    /// Asked for, not answered yet.
    Acquiring,
    Taken(H),
    /// The last ask failed: asked again only after the wish has gone away and come back, so a
    /// service that cannot do it is not hit on every event.
    Failed,
}

/// The sleep inhibitor's lifecycle: taken when the first job starts, given back when the last
/// finishes, and given back on exit. The service answers later than the wish changes, so an
/// inhibitor that arrives after the wish has gone is given back at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inhibitor<H> {
    held: Held<H>,
    wanted: bool,
}

impl<H: Copy> Default for Inhibitor<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: Copy> Inhibitor<H> {
    pub fn new() -> Self {
        Self {
            held: Held::Free,
            wanted: false,
        }
    }

    /// The machine should (or should no longer) be kept awake.
    pub fn want(&mut self, wanted: bool) -> InhibitAction<H> {
        self.wanted = wanted;
        match (wanted, self.held) {
            (true, Held::Free) => {
                self.held = Held::Acquiring;
                InhibitAction::Acquire
            }
            (false, Held::Taken(handle)) => {
                self.held = Held::Free;
                InhibitAction::Release(handle)
            }
            (false, Held::Failed) => {
                self.held = Held::Free;
                InhibitAction::None
            }
            _ => InhibitAction::None,
        }
    }

    /// The service answered an `Acquire`.
    pub fn acquired(&mut self, handle: H) -> InhibitAction<H> {
        if !matches!(self.held, Held::Acquiring) {
            // Not asked for (the app is exiting): give it straight back.
            return InhibitAction::Release(handle);
        }
        if self.wanted {
            self.held = Held::Taken(handle);
            InhibitAction::None
        } else {
            self.held = Held::Free;
            InhibitAction::Release(handle)
        }
    }

    /// The service refused or did not answer an `Acquire`.
    pub fn failed(&mut self) {
        if matches!(self.held, Held::Acquiring) {
            self.held = if self.wanted {
                Held::Failed
            } else {
                Held::Free
            };
        }
    }

    /// The app is exiting: whatever is held is given back, and nothing is asked for again.
    pub fn release_all(&mut self) -> InhibitAction<H> {
        self.wanted = false;
        match std::mem::replace(&mut self.held, Held::Free) {
            Held::Taken(handle) => InhibitAction::Release(handle),
            _ => InhibitAction::None,
        }
    }

    /// Whether an inhibitor is held now.
    #[cfg(test)]
    pub fn is_held(&self) -> bool {
        matches!(self.held, Held::Taken(_))
    }
}

// ---------------------------------------------------------------------------------------------
// Notifications (D121)
// ---------------------------------------------------------------------------------------------

/// A change in a job that may be worth a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// The job is done or failed.
    Finished(JobFacts),
    /// The job stopped to ask a question.
    NeedsAttention(JobFacts),
}

/// Notices the changes between one look at the queue and the next. A job that is cancelled is
/// remembered and never reported.
#[derive(Debug, Default)]
pub struct Tracker {
    seen: HashMap<u64, Phase>,
}

impl Tracker {
    /// Remembers what is in the queue now without reporting any of it: the jobs that were there
    /// before the integrations started listening are not news.
    pub fn seed(&mut self, jobs: &[JobFacts]) {
        self.seen = jobs.iter().map(|job| (job.id, job.phase)).collect();
    }

    /// The transitions since the last look, in queue order. A job that finished between two looks
    /// (it was never seen running) is still reported finished.
    pub fn observe(&mut self, jobs: &[JobFacts]) -> Vec<Transition> {
        let mut found = Vec::new();
        for job in jobs {
            let before = self.seen.insert(job.id, job.phase);
            match job.phase {
                Phase::Waiting if before != Some(Phase::Waiting) => {
                    found.push(Transition::NeedsAttention(job.clone()));
                }
                Phase::Done | Phase::Failed if !before.is_some_and(Phase::is_finished) => {
                    found.push(Transition::Finished(job.clone()));
                }
                _ => {}
            }
        }
        // A dismissed job is forgotten, so a retry that reuses nothing starts clean.
        let live: std::collections::HashSet<u64> = jobs.iter().map(|job| job.id).collect();
        self.seen.retain(|id, _| live.contains(id));
        found
    }
}

/// A notification to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The id the services replace and withdraw by: one per job, so the "needs your answer" notice
    /// is replaced by the one that says it finished.
    pub id: String,
    pub title: String,
    pub body: String,
}

/// The id of a job's notification.
pub fn notice_id(job: u64) -> String {
    format!("waypoint-job-{job}")
}

/// The notification a transition earns, if any (D121): a finished or failed job notifies only while
/// no Waypoint window has focus or when it ran over ten seconds; a question that is waiting always
/// does. A cancelled job never reaches here.
pub fn notice(transition: &Transition, any_window_focused: bool) -> Option<Notice> {
    match transition {
        Transition::NeedsAttention(job) => Some(Notice {
            id: notice_id(job.id),
            title: job.title.clone(),
            body: "Waypoint is waiting for your answer.".to_owned(),
        }),
        Transition::Finished(job) => {
            let long = job.ran_ms.is_some_and(|ms| ms > LONG_JOB_MS);
            if any_window_focused && !long {
                return None;
            }
            let body = match job.phase {
                Phase::Failed => job
                    .error
                    .clone()
                    .map(|error| format!("Failed: {error}"))
                    .unwrap_or_else(|| "Failed.".to_owned()),
                _ if job.failed > 0 => format!("Finished, but {} could not be done.", job.failed),
                _ if job.skipped > 0 => format!("Finished, with {} skipped.", job.skipped),
                _ => "Finished.".to_owned(),
            };
            Some(Notice {
                id: notice_id(job.id),
                title: job.title.clone(),
                body,
            })
        }
    }
}

// ---------------------------------------------------------------------------------------------
// FileManager1 calls
// ---------------------------------------------------------------------------------------------

/// One folder to open in a new tab, and the entry to select in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub folder: FilePath,
    /// The name of the entry to select in the folder, for `ShowItems` and `ShowItemProperties`.
    pub select: Option<String>,
}

/// A `file:` URI for this machine as a path: the plugin has already decoded and vetted it as local
/// (`path` is set), and `FilePath` makes it absolute and clean. Anything else is refused: another
/// host, another scheme, a relative or unrepresentable path.
fn local_path(uri: &str, path: Option<&str>) -> Option<FilePath> {
    path?;
    FilePath::from_uri(uri).ok()
}

/// What a `FileManager1` call asks Waypoint to open. `ShowFolders` opens each folder; `ShowItems`
/// and `ShowItemProperties` open each item's parent and select the item (the first of the items
/// that share a folder, since one tab selects one name). A target that is not a local `file:` URI
/// is refused and the rest are still honoured; at most `MAX_OPEN_FOLDERS` are opened.
pub fn open_requests(call: &FileManagerCall) -> Vec<OpenRequest> {
    let mut requests: Vec<OpenRequest> = Vec::new();
    for target in &call.targets {
        let Some(path) = local_path(&target.uri, target.path.as_deref()) else {
            continue;
        };
        let request = match call.method {
            FileManagerMethod::ShowFolders => OpenRequest {
                folder: path,
                select: None,
            },
            FileManagerMethod::ShowItems | FileManagerMethod::ShowItemProperties => {
                match (path.parent(), path.file_name()) {
                    (Some(parent), Some(name)) => OpenRequest {
                        folder: parent,
                        select: name.into_string().ok(),
                    },
                    // A root has no parent: show the root itself.
                    _ => OpenRequest {
                        folder: path,
                        select: None,
                    },
                }
            }
        };
        if requests.iter().any(|seen| seen.folder == request.folder) {
            continue;
        }
        requests.push(request);
        if requests.len() >= MAX_OPEN_FOLDERS {
            break;
        }
    }
    requests
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_desktop_integration::models::{
        FeatureStatus as DesktopFeatureStatus, FileManagerTarget,
    };
    use tauri_plugin_xdg_portal::models::FeatureStatus as PortalFeatureStatus;

    fn job(id: u64, phase: Phase) -> JobFacts {
        JobFacts {
            id,
            phase,
            title: format!("Copy {id} items"),
            items_done: 0,
            items_total: 0,
            bytes_done: 0,
            bytes_total: 0,
            ran_ms: None,
            skipped: 0,
            failed: 0,
            error: None,
        }
    }

    fn running(id: u64, done: u64, total: u64) -> JobFacts {
        JobFacts {
            bytes_done: done,
            bytes_total: total,
            ..job(id, Phase::Running)
        }
    }

    // ---- services and availability ----

    fn portal(notify: bool, inhibit: bool) -> PortalStatus {
        let feature = |feature, works| {
            if works {
                PortalFeatureStatus::available(feature, Some(1))
            } else {
                PortalFeatureStatus::unavailable(feature, PortalReason::NotSandboxed, None)
            }
        };
        PortalStatus::new(
            false,
            vec![
                feature(PortalFeature::Notification, notify),
                feature(PortalFeature::Inhibit, inhibit),
                feature(PortalFeature::OpenUri, false),
            ],
        )
    }

    fn desktop(working: &[DesktopFeature]) -> DesktopStatus {
        let all = [
            DesktopFeature::Notify,
            DesktopFeature::InhibitSleep,
            DesktopFeature::LauncherProgress,
            DesktopFeature::FileManager,
            DesktopFeature::GlobalShortcuts,
        ];
        DesktopStatus::new(
            all.into_iter()
                .map(|feature| {
                    if working.contains(&feature) {
                        DesktopFeatureStatus::available(feature)
                    } else {
                        DesktopFeatureStatus::unavailable(
                            feature,
                            match feature {
                                DesktopFeature::Notify => DesktopReason::NoNotificationServer,
                                DesktopFeature::InhibitSleep => DesktopReason::NoLogind,
                                DesktopFeature::GlobalShortcuts => DesktopReason::NoDisplayServer,
                                _ => DesktopReason::PlatformUnsupported,
                            },
                            None,
                        )
                    }
                })
                .collect(),
            false,
        )
    }

    const ALL: [DesktopFeature; 5] = [
        DesktopFeature::Notify,
        DesktopFeature::InhibitSleep,
        DesktopFeature::LauncherProgress,
        DesktopFeature::FileManager,
        DesktopFeature::GlobalShortcuts,
    ];

    #[test]
    fn the_portal_goes_first_where_it_works_and_the_zbus_services_are_the_fallback() {
        let working = desktop(&ALL);
        assert_eq!(
            notify_route(&portal(true, true), &working),
            Some(Route::Portal)
        );
        assert_eq!(
            inhibit_route(&portal(true, true), &working),
            Some(Route::Portal)
        );
        // A host session: the portal answers only sandboxed callers.
        assert_eq!(
            notify_route(&portal(false, false), &working),
            Some(Route::Desktop)
        );
        assert_eq!(
            inhibit_route(&portal(false, false), &working),
            Some(Route::Desktop)
        );
        // Each feature chooses on its own.
        assert_eq!(
            notify_route(&portal(true, false), &working),
            Some(Route::Portal)
        );
        assert_eq!(
            inhibit_route(&portal(true, false), &working),
            Some(Route::Desktop)
        );
        assert_eq!(notify_route(&portal(false, false), &desktop(&[])), None);
        assert_eq!(inhibit_route(&portal(false, false), &desktop(&[])), None);
    }

    #[test]
    fn availability_says_what_works_and_gives_the_fallbacks_reason_when_nothing_does() {
        let fine = availability(Platform::Linux, &portal(false, false), &desktop(&ALL));
        assert!(fine.notifications.available);
        assert!(fine.launcher_progress.available);
        assert!(fine.prevent_sleep.available);
        assert!(fine.file_manager_service.available);
        assert!(fine.global_shortcut.available);

        let bare = availability(Platform::Linux, &portal(false, false), &desktop(&[]));
        assert_eq!(
            bare.notifications.reason.as_deref(),
            Some("No notification server is running.")
        );
        assert_eq!(
            bare.prevent_sleep.reason.as_deref(),
            Some("systemd-logind is not running.")
        );
        assert_eq!(
            bare.global_shortcut.reason.as_deref(),
            Some("No display server is running.")
        );
        assert!(!bare.launcher_progress.available);
        assert!(!bare.file_manager_service.available);

        // The portal alone is enough for notifications and the inhibitor.
        let portal_only = availability(Platform::Linux, &portal(true, true), &desktop(&[]));
        assert!(portal_only.notifications.available);
        assert!(portal_only.prevent_sleep.available);
        assert!(!portal_only.launcher_progress.available);
    }

    #[test]
    fn no_integration_is_offered_on_an_unsupported_platform() {
        let none = availability(Platform::Other, &portal(true, true), &desktop(&ALL));
        for a in [
            none.notifications,
            none.launcher_progress,
            none.prevent_sleep,
            none.file_manager_service,
            none.global_shortcut,
        ] {
            assert!(!a.available);
            assert!(a.reason.is_some());
        }
    }

    #[test]
    fn the_services_panel_lists_what_works_and_the_first_reason_that_does_not() {
        let partial = desktop_summary(&desktop(&[
            DesktopFeature::Notify,
            DesktopFeature::FileManager,
        ]));
        assert!(partial.available);
        assert_eq!(partial.features, vec!["notify", "fileManager"]);
        assert_eq!(
            partial.reason.as_deref(),
            Some("systemd-logind is not running.")
        );
        let none = desktop_summary(&desktop(&[]));
        assert!(!none.available);
        assert!(none.features.is_empty());
        let host = portal_summary(&portal(false, false));
        assert!(!host.available);
        assert!(host.reason.unwrap().contains("sandboxed"));
        let sandboxed = portal_summary(&portal(true, true));
        assert_eq!(sandboxed.features, vec!["notification", "inhibit"]);
        assert!(sandboxed.available);
    }

    // ---- progress ----

    #[test]
    fn progress_is_cleared_when_nothing_is_in_flight() {
        assert_eq!(combined_progress(&[]), Shown::Cleared);
        assert_eq!(
            combined_progress(&[job(1, Phase::Done), job(2, Phase::Cancelled)]),
            Shown::Cleared
        );
    }

    #[test]
    fn progress_is_indeterminate_while_planning() {
        assert_eq!(
            combined_progress(&[job(1, Phase::Planning)]),
            Shown::Indeterminate
        );
        assert_eq!(
            combined_progress(&[job(1, Phase::Planning), job(2, Phase::Queued)]),
            Shown::Indeterminate
        );
    }

    #[test]
    fn progress_combines_every_job_in_flight_by_bytes() {
        let jobs = [
            running(1, 50, 100),
            running(2, 0, 300),
            // Finished jobs are not part of it.
            JobFacts {
                bytes_done: 999,
                bytes_total: 999,
                ..job(3, Phase::Done)
            },
        ];
        assert_eq!(combined_progress(&jobs), Shown::Percent(12));
    }

    #[test]
    fn progress_uses_items_for_a_job_without_bytes_and_never_reads_full_early() {
        let by_items = JobFacts {
            items_done: 1,
            items_total: 4,
            ..job(1, Phase::Running)
        };
        assert_eq!(combined_progress(&[by_items]), Shown::Percent(25));
        assert_eq!(
            combined_progress(&[running(1, 999, 1000)]),
            Shown::Percent(99)
        );
        // A tick past the total (a growing file) is held at the end.
        assert_eq!(
            combined_progress(&[running(1, 120, 100)]),
            Shown::Percent(100)
        );
    }

    #[test]
    fn a_job_planning_beside_a_running_one_leaves_the_running_ones_value() {
        assert_eq!(
            combined_progress(&[job(1, Phase::Planning), running(2, 10, 100)]),
            Shown::Percent(10)
        );
    }

    // ---- keeping awake ----

    #[test]
    fn only_working_jobs_keep_the_machine_awake() {
        assert!(!keeps_awake(&[]));
        assert!(keeps_awake(&[job(1, Phase::Planning)]));
        assert!(keeps_awake(&[job(1, Phase::Running)]));
        assert!(keeps_awake(&[job(1, Phase::Cancelling)]));
        for idle in [
            Phase::Queued,
            Phase::Paused,
            Phase::Waiting,
            Phase::Done,
            Phase::Failed,
            Phase::Cancelled,
        ] {
            assert!(!keeps_awake(&[job(1, idle)]), "{idle:?}");
        }
        assert!(keeps_awake(&[job(1, Phase::Done), job(2, Phase::Running)]));
    }

    // ---- the inhibitor ----

    #[test]
    fn the_inhibitor_is_taken_on_the_first_job_and_released_when_idle() {
        let mut inhibitor = Inhibitor::<u32>::new();
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
        // More events while the service answers do not ask again.
        assert_eq!(inhibitor.want(true), InhibitAction::None);
        assert_eq!(inhibitor.acquired(7), InhibitAction::None);
        assert!(inhibitor.is_held());
        assert_eq!(inhibitor.want(true), InhibitAction::None);
        assert_eq!(inhibitor.want(false), InhibitAction::Release(7));
        assert!(!inhibitor.is_held());
        assert_eq!(inhibitor.want(false), InhibitAction::None);
        // And it can be taken again for the next job.
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
    }

    #[test]
    fn an_inhibitor_that_arrives_after_the_jobs_are_gone_is_given_back() {
        let mut inhibitor = Inhibitor::<u32>::new();
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
        assert_eq!(inhibitor.want(false), InhibitAction::None);
        assert_eq!(inhibitor.acquired(3), InhibitAction::Release(3));
        assert!(!inhibitor.is_held());
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
    }

    #[test]
    fn a_refused_inhibitor_is_asked_for_again_only_after_the_wish_comes_back() {
        let mut inhibitor = Inhibitor::<u32>::new();
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
        inhibitor.failed();
        assert_eq!(inhibitor.want(true), InhibitAction::None);
        assert_eq!(inhibitor.want(false), InhibitAction::None);
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
    }

    #[test]
    fn a_failure_after_the_wish_went_leaves_it_free() {
        let mut inhibitor = Inhibitor::<u32>::new();
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
        assert_eq!(inhibitor.want(false), InhibitAction::None);
        inhibitor.failed();
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
    }

    #[test]
    fn exiting_releases_what_is_held_and_nothing_is_taken_after() {
        let mut inhibitor = Inhibitor::<u32>::new();
        assert_eq!(inhibitor.release_all(), InhibitAction::None);
        inhibitor.want(true);
        inhibitor.acquired(9);
        assert_eq!(inhibitor.release_all(), InhibitAction::Release(9));
        assert!(!inhibitor.is_held());
        // An answer that was still on its way is given straight back.
        let mut late = Inhibitor::<u32>::new();
        late.want(true);
        assert_eq!(late.release_all(), InhibitAction::None);
        assert_eq!(late.acquired(4), InhibitAction::Release(4));
    }

    // ---- notifications ----

    fn finished(id: u64, ran_ms: Option<u64>) -> Transition {
        Transition::Finished(JobFacts {
            ran_ms,
            ..job(id, Phase::Done)
        })
    }

    #[test]
    fn a_finished_job_notifies_when_no_window_has_focus() {
        let notice = notice(&finished(1, Some(500)), false).expect("a notice");
        assert_eq!(notice.id, "waypoint-job-1");
        assert_eq!(notice.title, "Copy 1 items");
        assert_eq!(notice.body, "Finished.");
    }

    #[test]
    fn a_focused_window_keeps_a_short_job_quiet_but_not_a_long_one() {
        assert_eq!(notice(&finished(1, Some(500)), true), None);
        assert_eq!(notice(&finished(1, Some(LONG_JOB_MS)), true), None);
        assert!(notice(&finished(1, Some(LONG_JOB_MS + 1)), true).is_some());
        // A job whose run time is unknown counts as short.
        assert_eq!(notice(&finished(1, None), true), None);
    }

    #[test]
    fn a_failed_job_says_why_and_a_partial_one_says_how_much() {
        let failed = Transition::Finished(JobFacts {
            error: Some("the disk is full".into()),
            ..job(1, Phase::Failed)
        });
        assert_eq!(
            notice(&failed, false).unwrap().body,
            "Failed: the disk is full"
        );
        let no_reason = Transition::Finished(job(1, Phase::Failed));
        assert_eq!(notice(&no_reason, false).unwrap().body, "Failed.");
        let partial = Transition::Finished(JobFacts {
            failed: 2,
            ..job(1, Phase::Done)
        });
        assert_eq!(
            notice(&partial, false).unwrap().body,
            "Finished, but 2 could not be done."
        );
        let skipped = Transition::Finished(JobFacts {
            skipped: 3,
            ..job(1, Phase::Done)
        });
        assert_eq!(
            notice(&skipped, false).unwrap().body,
            "Finished, with 3 skipped."
        );
    }

    #[test]
    fn a_question_that_is_waiting_always_notifies() {
        let waiting = Transition::NeedsAttention(job(1, Phase::Waiting));
        assert!(notice(&waiting, true).is_some());
        assert!(notice(&waiting, false).is_some());
    }

    #[test]
    fn the_tracker_reports_a_job_once_when_it_finishes() {
        let mut tracker = Tracker::default();
        assert!(tracker.observe(&[job(1, Phase::Running)]).is_empty());
        let done = tracker.observe(&[job(1, Phase::Done)]);
        assert_eq!(done.len(), 1);
        assert!(matches!(&done[0], Transition::Finished(j) if j.id == 1));
        // The same state again is not news.
        assert!(tracker.observe(&[job(1, Phase::Done)]).is_empty());
    }

    #[test]
    fn the_tracker_reports_a_job_that_finished_between_two_looks() {
        let mut tracker = Tracker::default();
        let found = tracker.observe(&[job(1, Phase::Done)]);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn the_tracker_reports_a_failure_and_a_wait_but_never_a_cancel() {
        let mut tracker = Tracker::default();
        let found = tracker.observe(&[
            job(1, Phase::Failed),
            job(2, Phase::Waiting),
            job(3, Phase::Cancelled),
        ]);
        assert_eq!(found.len(), 2);
        assert!(matches!(&found[0], Transition::Finished(j) if j.id == 1));
        assert!(matches!(&found[1], Transition::NeedsAttention(j) if j.id == 2));
        // A job that waited, then finished, is reported for both; one that waited, then was cancelled, is not reported again.
        let next = tracker.observe(&[job(2, Phase::Done)]);
        assert_eq!(next.len(), 1);
        let mut other = Tracker::default();
        other.observe(&[job(1, Phase::Waiting)]);
        assert!(other.observe(&[job(1, Phase::Cancelling)]).is_empty());
        assert!(other.observe(&[job(1, Phase::Cancelled)]).is_empty());
    }

    #[test]
    fn a_job_that_waits_again_is_reported_again_and_a_retry_with_a_new_id_is_new() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.observe(&[job(1, Phase::Waiting)]).len(), 1);
        assert!(tracker.observe(&[job(1, Phase::Waiting)]).is_empty());
        assert!(tracker.observe(&[job(1, Phase::Running)]).is_empty());
        assert_eq!(tracker.observe(&[job(1, Phase::Waiting)]).len(), 1);
    }

    #[test]
    fn jobs_already_in_the_queue_when_listening_starts_are_not_news() {
        let mut tracker = Tracker::default();
        tracker.seed(&[job(1, Phase::Done), job(2, Phase::Waiting)]);
        assert!(tracker
            .observe(&[job(1, Phase::Done), job(2, Phase::Waiting)])
            .is_empty());
        assert_eq!(tracker.observe(&[job(3, Phase::Done)]).len(), 1);
    }

    // ---- FileManager1 ----

    fn call(method: FileManagerMethod, uris: &[(&str, Option<&str>)]) -> FileManagerCall {
        FileManagerCall {
            method,
            targets: uris
                .iter()
                .map(|(uri, path)| FileManagerTarget {
                    uri: (*uri).to_owned(),
                    path: path.map(str::to_owned),
                })
                .collect(),
            startup_id: String::new(),
        }
    }

    #[cfg(unix)]
    fn folder(path: &str) -> FilePath {
        FilePath::parse(path).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn show_folders_opens_each_folder() {
        let requests = open_requests(&call(
            FileManagerMethod::ShowFolders,
            &[
                ("file:///home/me/Music", Some("/home/me/Music")),
                ("file:///tmp", Some("/tmp")),
            ],
        ));
        assert_eq!(
            requests,
            vec![
                OpenRequest {
                    folder: folder("/home/me/Music"),
                    select: None
                },
                OpenRequest {
                    folder: folder("/tmp"),
                    select: None
                },
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn show_items_opens_the_parent_and_selects_the_item() {
        for method in [
            FileManagerMethod::ShowItems,
            FileManagerMethod::ShowItemProperties,
        ] {
            let requests = open_requests(&call(
                method,
                &[("file:///home/me/a%20b.txt", Some("/home/me/a b.txt"))],
            ));
            assert_eq!(
                requests,
                vec![OpenRequest {
                    folder: folder("/home/me"),
                    select: Some("a b.txt".into())
                }]
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn items_in_one_folder_open_one_tab_selecting_the_first() {
        let requests = open_requests(&call(
            FileManagerMethod::ShowItems,
            &[
                ("file:///d/one", Some("/d/one")),
                ("file:///d/two", Some("/d/two")),
                ("file:///e/three", Some("/e/three")),
            ],
        ));
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].select.as_deref(), Some("one"));
        assert_eq!(requests[1].folder, folder("/e"));
    }

    #[cfg(unix)]
    #[test]
    fn a_root_is_shown_as_itself() {
        let requests = open_requests(&call(
            FileManagerMethod::ShowItems,
            &[("file:///", Some("/"))],
        ));
        assert_eq!(
            requests,
            vec![OpenRequest {
                folder: folder("/"),
                select: None
            }]
        );
    }

    #[test]
    fn unknown_and_non_local_uris_are_refused() {
        let requests = open_requests(&call(
            FileManagerMethod::ShowFolders,
            &[
                // The plugin found no local path: another host, or not a file URI.
                ("file://other-host/share", None),
                ("sftp://host/dir", None),
                ("smb://host/share", None),
                ("trash:///", None),
                ("not a uri", None),
                ("", None),
                // A path the plugin handed over but the URI does not back up.
                ("https://example.com/x", Some("/x")),
            ],
        ));
        assert!(requests.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_bad_target_does_not_stop_the_good_ones_and_the_count_is_capped() {
        let mut targets: Vec<(String, String)> = (0..20)
            .map(|n| (format!("file:///d{n}"), format!("/d{n}")))
            .collect();
        targets.insert(0, ("sftp://h/x".into(), String::new()));
        let call = FileManagerCall {
            method: FileManagerMethod::ShowFolders,
            targets: targets
                .into_iter()
                .map(|(uri, path)| FileManagerTarget {
                    path: (!path.is_empty()).then_some(path),
                    uri,
                })
                .collect(),
            startup_id: String::new(),
        };
        let requests = open_requests(&call);
        assert_eq!(requests.len(), MAX_OPEN_FOLDERS);
        assert_eq!(requests[0].folder, folder("/d0"));
    }
}

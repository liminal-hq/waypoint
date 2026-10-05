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
// - The buttons a job's notification carries and what a pressed one does (`buttons`, `parse_action`,
//   `command_for`).
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
use waypoint_ops::{ConflictKind, ConflictPolicy, JobKind, JobSnapshot, JobState, WaitReason};
use waypoint_path::FilePath;
use waypoint_protocol::{Availability, IntegrationAvailability, Location, PluginStatus};

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

/// Whether the route notifications take accepts buttons (`notificationActions`). The portal cannot
/// say whether the desktop draws them, so this means they are sent, and a desktop that shows only
/// the click still does the default (D121).
pub fn actions_work(route: Route, portal: &PortalStatus, desktop: &DesktopStatus) -> bool {
    match route {
        Route::Portal => portal_works(portal, PortalFeature::NotificationActions),
        Route::Desktop => desktop_works(desktop, DesktopFeature::NotificationActions),
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
        Some(DesktopReason::ActionsUnsupported) => {
            "The notification server does not draw buttons, so notifications offer only a click."
        }
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
        Some(PortalReason::ActionsUnsupported) => {
            "The notification portal is too old to take buttons, so notifications offer only a click."
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
        (PortalFeature::NotificationActions, "notificationActions"),
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
        (DesktopFeature::NotificationActions, "notificationActions"),
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
    keyring: Availability,
) -> IntegrationAvailability {
    if platform == Platform::Other {
        let none = || Availability::no("This operating system does not offer it.");
        return IntegrationAvailability {
            notifications: none(),
            notification_actions: none(),
            launcher_progress: none(),
            prevent_sleep: none(),
            file_manager_service: none(),
            global_shortcut: none(),
            remember_passphrases: none(),
        };
    }
    IntegrationAvailability {
        notifications: either_availability(
            portal,
            PortalFeature::Notification,
            desktop,
            DesktopFeature::Notify,
        ),
        notification_actions: either_availability(
            portal,
            PortalFeature::NotificationActions,
            desktop,
            DesktopFeature::NotificationActions,
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
        remember_passphrases: keyring,
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
    /// What the job does, as far as its buttons care.
    pub kind: Kind,
    /// Where the job put things, when it has one.
    pub destination: Option<Location>,
    /// The window that started the job.
    pub origin_window: String,
    /// What a waiting job waits on.
    pub wait: Wait,
    /// The job was held by a schedule, so its start is news (D157).
    pub scheduled: bool,
}

/// A job's kind, reduced to what its buttons depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Move,
    Trash,
    Other,
}

/// What a job that has stopped for an answer waits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wait {
    /// Not waiting.
    None,
    /// One file over one file: Replace, Skip and Keep both each settle it. `source` is the entry
    /// being copied or moved, which an answer names.
    PlainConflict { source: Location },
    /// Several clashes, a folder over a file, a clash inside the batch, and the like: the dialog
    /// is the place for it.
    OtherConflict,
    /// An item failed.
    Error,
}

/// The facts of a job snapshot.
pub fn facts(job: &JobSnapshot) -> JobFacts {
    let wait = match &job.state {
        JobState::Waiting {
            reason: WaitReason::Conflicts { conflicts },
        } => match conflicts.as_slice() {
            [one] if one.kind == ConflictKind::FileOverFile && !one.within_batch => {
                Wait::PlainConflict {
                    source: one.source.clone(),
                }
            }
            _ => Wait::OtherConflict,
        },
        JobState::Waiting {
            reason: WaitReason::Error { .. },
        } => Wait::Error,
        _ => Wait::None,
    };
    let kind = match job.kind {
        JobKind::Move => Kind::Move,
        JobKind::Trash => Kind::Trash,
        _ => Kind::Other,
    };
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
        kind,
        destination: job.destination.clone(),
        origin_window: job.origin_window.clone(),
        wait,
        scheduled: job.options.schedule.is_some(),
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
    /// The backend took the request and then dropped it (it will not inhibit sleep), so there is
    /// nothing to release and nothing worth asking for again this run.
    Refused,
}

/// Why prevent sleep is unavailable once the backend has refused the inhibitor: the Services panel
/// and the switch show it.
pub const INHIBIT_REFUSED_REASON: &str =
    "The desktop's portal backend does not allow a sleep inhibitor here.";

/// Whether a failed release means the portal's request object does not exist, which is how a
/// backend that refused the inhibitor looks: the portal hands back a request path at once and the
/// backend drops it afterwards, so `Close` finds nothing at that path.
pub fn request_is_gone(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("unknownmethod")
        || lower.contains("unknownobject")
        || lower.contains("does not exist at path")
}

/// `availability` with prevent sleep switched off, and why, when the inhibitor was refused.
pub fn with_refused_inhibit(
    mut availability: IntegrationAvailability,
    refused: bool,
) -> IntegrationAvailability {
    if refused && availability.prevent_sleep.available {
        availability.prevent_sleep = Availability::no(INHIBIT_REFUSED_REASON);
    }
    availability
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
            // Refused stays refused: no ask, and so no release, for the rest of the run.
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

    /// The backend refused the inhibitor that was just taken (its request is gone). Returns whether
    /// this is news, so the caller logs the reason once; nothing is held and none is asked for again.
    pub fn refused(&mut self) -> bool {
        let first = !matches!(self.held, Held::Refused);
        self.held = Held::Refused;
        first
    }

    /// The backend has refused an inhibitor this run.
    pub fn is_refused(&self) -> bool {
        matches!(self.held, Held::Refused)
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

/// What the inhibitor talks to: the portal or logind in the app, a fake in tests.
pub trait InhibitBackend {
    /// What a taken inhibitor is released by.
    type Handle: Copy;

    /// The route a handle was taken through.
    fn route_of(handle: Self::Handle) -> Route;

    async fn acquire(&self, route: Route) -> Result<Self::Handle, String>;
    async fn release(&self, handle: Self::Handle) -> Result<(), String>;
}

/// Gives an inhibitor back. A failure that says the portal's request is gone means the backend
/// refused the inhibitor (it only says so once the request is dropped), so that is logged once as
/// the reason prevent sleep is unavailable, and never again asked for; any other failure is a
/// warning, because it may be real.
async fn release_inhibitor<B: InhibitBackend>(
    backend: &B,
    inhibitor: &mut Inhibitor<B::Handle>,
    handle: B::Handle,
) {
    match backend.release(handle).await {
        Ok(()) => {}
        // Only the portal has a request object that a backend can drop; on logind a missing object
        // is an ordinary failure.
        Err(e) if B::route_of(handle) == Route::Portal && request_is_gone(&e) => {
            if inhibitor.refused() {
                log::warn!("prevent sleep is unavailable: {INHIBIT_REFUSED_REASON} ({e})");
            }
        }
        Err(e) => log::warn!("could not release the sleep inhibitor: {e}"),
    }
}

/// Follows the wish to keep the machine awake with the inhibitor's state machine.
pub async fn drive_inhibitor<B: InhibitBackend>(
    backend: &B,
    inhibitor: &mut Inhibitor<B::Handle>,
    wanted: bool,
    route: Option<Route>,
) {
    match inhibitor.want(wanted) {
        InhibitAction::None => {}
        InhibitAction::Release(handle) => release_inhibitor(backend, inhibitor, handle).await,
        InhibitAction::Acquire => {
            let Some(route) = route else {
                inhibitor.failed();
                return;
            };
            match backend.acquire(route).await {
                Ok(handle) => {
                    if let InhibitAction::Release(handle) = inhibitor.acquired(handle) {
                        release_inhibitor(backend, inhibitor, handle).await;
                    }
                }
                Err(e) => {
                    log::warn!("could not keep the system awake: {e}");
                    inhibitor.failed();
                }
            }
        }
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
    /// A job that was held by a schedule has started (D157).
    Started(JobFacts),
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
                // A scheduled job that was queued and is running now: someone away from the
                // window learns that it began. A job first seen already running is not news.
                Phase::Running if job.scheduled && before == Some(Phase::Queued) => {
                    found.push(Transition::Started(job.clone()));
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
    /// Buttons, in order. Empty until the caller attaches `buttons` (only where they are wanted
    /// and the route accepts them).
    pub actions: Vec<Button>,
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
        Transition::Started(job) => {
            // Told only while nobody is looking: with a window focused the Operations list shows it.
            if any_window_focused {
                return None;
            }
            Some(Notice {
                id: notice_id(job.id),
                title: job.title.clone(),
                body: "A scheduled job has started.".to_owned(),
                actions: Vec::new(),
            })
        }
        Transition::NeedsAttention(job) => Some(Notice {
            id: notice_id(job.id),
            title: job.title.clone(),
            body: "Waypoint is waiting for your answer.".to_owned(),
            actions: Vec::new(),
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
                actions: Vec::new(),
            })
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Notification buttons (D121, D122)
// ---------------------------------------------------------------------------------------------

/// What a button does. Each is a shortcut for something the app already does; the click on the
/// notification itself raises a window whatever else is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Open the destination as a tab in the window that started the job.
    ShowInFolder,
    /// Open the Operations window.
    ShowDetails,
    /// Open the question's dialog.
    Show,
    /// Answer this one conflict.
    Resolve(ConflictPolicy),
    /// Undo the job's journal entry.
    Undo,
}

impl Act {
    /// The slug in a button's id.
    fn slug(self) -> &'static str {
        match self {
            Act::ShowInFolder => "folder",
            Act::ShowDetails => "details",
            Act::Show => "show",
            Act::Resolve(ConflictPolicy::Replace) => "replace",
            Act::Resolve(ConflictPolicy::Skip) => "skip",
            Act::Resolve(ConflictPolicy::KeepBoth) => "keep-both",
            // Never offered: a notification answers one item with one of the three above.
            Act::Resolve(_) => "other",
            Act::Undo => "undo",
        }
    }

    fn from_slug(slug: &str) -> Option<Self> {
        Some(match slug {
            "folder" => Act::ShowInFolder,
            "details" => Act::ShowDetails,
            "show" => Act::Show,
            "replace" => Act::Resolve(ConflictPolicy::Replace),
            "skip" => Act::Resolve(ConflictPolicy::Skip),
            "keep-both" => Act::Resolve(ConflictPolicy::KeepBoth),
            "undo" => Act::Undo,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Act::ShowInFolder => "Show in folder",
            Act::ShowDetails => "Show details",
            Act::Show => "Show",
            Act::Resolve(ConflictPolicy::Replace) => "Replace",
            Act::Resolve(ConflictPolicy::Skip) => "Skip",
            Act::Resolve(ConflictPolicy::KeepBoth) => "Keep both",
            Act::Resolve(_) => "Resolve",
            Act::Undo => "Undo",
        }
    }
}

/// One button on a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    /// Stable, and encodes the job and the action (`button_id`), so the press can be understood
    /// whenever it arrives.
    pub id: String,
    pub label: String,
}

/// The most buttons a notification carries; the services drop any beyond it.
pub const MAX_BUTTONS: usize = 3;

const ID_PREFIX: &str = "waypoint.job.";

/// A button's id: `waypoint.job.<job>.<action>`. It never starts with `app.` (which the portal
/// routes to the application) and is never `default` (which `desktop-integration` reserves).
pub fn button_id(job: u64, act: Act) -> String {
    format!("{ID_PREFIX}{job}.{}", act.slug())
}

/// The job and action a button id encodes. Anything malformed or from elsewhere (the click's
/// `raise`, another application's ids) is `None`.
pub fn parse_action(id: &str) -> Option<(u64, Act)> {
    let rest = id.strip_prefix(ID_PREFIX)?;
    let (job, slug) = rest.split_once('.')?;
    // Plain digits only: no sign, no space, nothing `parse` would forgive.
    if job.is_empty() || !job.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((job.parse().ok()?, Act::from_slug(slug)?))
}

/// The buttons for a job's notification, in order. `undo_is_this` says the undo journal's newest
/// applied entry is the one this job made, so "Undo" would undo exactly it; it is hidden otherwise,
/// because another operation has since come on top of it.
///
/// - Finished: "Show in folder" where the job has a destination, and "Undo" for a move or a trash
///   while it still can.
/// - Failed: "Show details".
/// - Waiting on a plain conflict: "Replace", "Skip" and "Keep both" for this item only (never "apply
///   to all"); the click on the notification is "Show". Any other wait: "Show".
pub fn buttons(job: &JobFacts, undo_is_this: bool) -> Vec<Button> {
    let acts: Vec<Act> = match job.phase {
        Phase::Done => {
            let mut acts = Vec::new();
            if job.destination.is_some() {
                acts.push(Act::ShowInFolder);
            }
            if matches!(job.kind, Kind::Move | Kind::Trash) && undo_is_this {
                acts.push(Act::Undo);
            }
            acts
        }
        Phase::Failed => vec![Act::ShowDetails],
        Phase::Running if job.scheduled => vec![Act::ShowDetails],
        Phase::Waiting => match job.wait {
            Wait::PlainConflict { .. } => vec![
                Act::Resolve(ConflictPolicy::Replace),
                Act::Resolve(ConflictPolicy::Skip),
                Act::Resolve(ConflictPolicy::KeepBoth),
            ],
            Wait::OtherConflict | Wait::Error => vec![Act::Show],
            Wait::None => Vec::new(),
        },
        _ => Vec::new(),
    };
    acts.into_iter()
        .take(MAX_BUTTONS)
        .map(|act| Button {
            id: button_id(job.id, act),
            label: act.label().to_owned(),
        })
        .collect()
}

/// The buttons a transition's notification carries. `undo_top` is the job whose journal entry the
/// undo journal would undo now, if any.
pub fn transition_buttons(transition: &Transition, undo_top: Option<u64>) -> Vec<Button> {
    match transition {
        Transition::Finished(job) | Transition::NeedsAttention(job) | Transition::Started(job) => {
            buttons(job, undo_top == Some(job.id))
        }
    }
}

/// What a pressed button asks the app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionCommand {
    /// Open `location` as a tab in `window` when that window is open, else in a new window.
    OpenFolder { window: String, location: Location },
    /// Open the Operations window.
    OpenOperations,
    /// Show the question a job waits on: the window that started it, or the Operations window.
    ShowQuestion { job: u64, window: String },
    /// Answer the one conflict a job waits on.
    Resolve {
        job: u64,
        source: Location,
        policy: ConflictPolicy,
    },
    /// Undo the entry the job made.
    Undo { job: u64, window: String },
}

/// What a press of the button `id` does, against the queue as it is now. `undo_is_this` is the job
/// whose entry the undo journal would undo now, if any. `None` is for an id that is not one of
/// Waypoint's buttons (including the click's `raise`). A button for a job that has gone, or whose
/// moment has passed (answered elsewhere, no longer on top of the undo journal), opens the
/// Operations window rather than doing nothing or doing the wrong thing.
pub fn command_for(
    id: &str,
    jobs: &[JobFacts],
    undo_is_this: Option<u64>,
) -> Option<ActionCommand> {
    let (job_id, act) = parse_action(id)?;
    let Some(job) = jobs.iter().find(|job| job.id == job_id) else {
        return Some(ActionCommand::OpenOperations);
    };
    let command = match act {
        Act::ShowInFolder => job
            .destination
            .clone()
            .filter(|_| job.phase == Phase::Done)
            .map(|location| ActionCommand::OpenFolder {
                window: job.origin_window.clone(),
                location,
            }),
        Act::ShowDetails => Some(ActionCommand::OpenOperations),
        Act::Show => (job.phase == Phase::Waiting).then(|| ActionCommand::ShowQuestion {
            job: job.id,
            window: job.origin_window.clone(),
        }),
        Act::Resolve(policy) => match &job.wait {
            Wait::PlainConflict { source }
                if job.phase == Phase::Waiting
                    && matches!(
                        policy,
                        ConflictPolicy::Replace | ConflictPolicy::Skip | ConflictPolicy::KeepBoth
                    ) =>
            {
                Some(ActionCommand::Resolve {
                    job: job.id,
                    source: source.clone(),
                    policy,
                })
            }
            _ => None,
        },
        Act::Undo => (job.phase == Phase::Done
            && matches!(job.kind, Kind::Move | Kind::Trash)
            && undo_is_this == Some(job.id))
        .then(|| ActionCommand::Undo {
            job: job.id,
            window: job.origin_window.clone(),
        }),
    };
    Some(command.unwrap_or(ActionCommand::OpenOperations))
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
            kind: Kind::Other,
            destination: None,
            origin_window: "main-1".to_owned(),
            wait: Wait::None,
            scheduled: false,
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
                feature(PortalFeature::NotificationActions, notify),
                feature(PortalFeature::Inhibit, inhibit),
                feature(PortalFeature::OpenUri, false),
            ],
        )
    }

    fn desktop(working: &[DesktopFeature]) -> DesktopStatus {
        let all = [
            DesktopFeature::Notify,
            DesktopFeature::NotificationActions,
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
                                DesktopFeature::NotificationActions => {
                                    DesktopReason::ActionsUnsupported
                                }
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

    const ALL: [DesktopFeature; 6] = [
        DesktopFeature::Notify,
        DesktopFeature::NotificationActions,
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
        let fine = availability(
            Platform::Linux,
            &portal(false, false),
            &desktop(&ALL),
            Availability::yes(),
        );
        assert!(fine.notifications.available);
        assert!(fine.launcher_progress.available);
        assert!(fine.prevent_sleep.available);
        assert!(fine.file_manager_service.available);
        assert!(fine.global_shortcut.available);

        let bare = availability(
            Platform::Linux,
            &portal(false, false),
            &desktop(&[]),
            Availability::yes(),
        );
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
        let portal_only = availability(
            Platform::Linux,
            &portal(true, true),
            &desktop(&[]),
            Availability::yes(),
        );
        assert!(portal_only.notifications.available);
        assert!(portal_only.prevent_sleep.available);
        assert!(!portal_only.launcher_progress.available);
    }

    #[test]
    fn no_integration_is_offered_on_an_unsupported_platform() {
        let no_keyring = availability(
            Platform::Linux,
            &portal(false, false),
            &desktop(&ALL),
            Availability::no("no keyring is running"),
        );
        assert_eq!(
            no_keyring.remember_passphrases,
            Availability::no("no keyring is running")
        );
        assert!(no_keyring.notifications.available);
        let none = availability(
            Platform::Other,
            &portal(true, true),
            &desktop(&ALL),
            Availability::yes(),
        );
        for a in [
            none.notifications,
            none.notification_actions,
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
            DesktopFeature::NotificationActions,
            DesktopFeature::FileManager,
        ]));
        assert!(partial.available);
        assert_eq!(
            partial.features,
            vec!["notify", "notificationActions", "fileManager"]
        );
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
        assert_eq!(
            sandboxed.features,
            vec!["notification", "notificationActions", "inhibit"]
        );
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

    /// A backend that hands out a request path and then, like a portal backend that refuses, has
    /// nothing at it: `Close` answers `UnknownMethod`. Counts what it is asked.
    #[derive(Default)]
    struct Refusing {
        acquires: std::cell::Cell<u32>,
        releases: std::cell::Cell<u32>,
    }

    impl InhibitBackend for Refusing {
        type Handle = u32;

        fn route_of(_: u32) -> Route {
            Route::Portal
        }

        async fn acquire(&self, _: Route) -> Result<u32, String> {
            self.acquires.set(self.acquires.get() + 1);
            Ok(1)
        }

        async fn release(&self, _: u32) -> Result<(), String> {
            self.releases.set(self.releases.get() + 1);
            Err(
                "Close: org.freedesktop.DBus.Error.UnknownMethod: Object does not exist at path \
                 /org/freedesktop/portal/desktop/request/1_2/t0"
                    .to_owned(),
            )
        }
    }

    #[test]
    fn a_refused_inhibit_is_noted_once_and_never_asked_for_or_released_again() {
        let backend = Refusing::default();
        let mut inhibitor = Inhibitor::<u32>::new();
        let route = Some(Route::Portal);
        tauri::async_runtime::block_on(async {
            // First job: taken, then released at the end, which finds nothing to close.
            drive_inhibitor(&backend, &mut inhibitor, true, route).await;
            assert!(inhibitor.is_held());
            drive_inhibitor(&backend, &mut inhibitor, false, route).await;
            assert!(inhibitor.is_refused());
            assert_eq!((backend.acquires.get(), backend.releases.get()), (1, 1));
            // Later jobs neither ask nor release.
            for _ in 0..3 {
                drive_inhibitor(&backend, &mut inhibitor, true, route).await;
                drive_inhibitor(&backend, &mut inhibitor, false, route).await;
            }
        });
        assert_eq!((backend.acquires.get(), backend.releases.get()), (1, 1));
        assert!(!inhibitor.is_held());
        assert!(!inhibitor.refused(), "only the first refusal is news");
    }

    #[test]
    fn a_release_that_fails_for_another_reason_is_not_taken_for_a_refusal() {
        struct Broken;
        impl InhibitBackend for Broken {
            type Handle = u32;
            fn route_of(_: u32) -> Route {
                Route::Portal
            }
            async fn acquire(&self, _: Route) -> Result<u32, String> {
                Ok(2)
            }
            async fn release(&self, _: u32) -> Result<(), String> {
                Err("the bus went away".to_owned())
            }
        }
        let mut inhibitor = Inhibitor::<u32>::new();
        tauri::async_runtime::block_on(async {
            drive_inhibitor(&Broken, &mut inhibitor, true, Some(Route::Portal)).await;
            drive_inhibitor(&Broken, &mut inhibitor, false, Some(Route::Portal)).await;
        });
        assert!(!inhibitor.is_refused());
        assert_eq!(
            inhibitor.want(true),
            InhibitAction::Acquire,
            "it can be asked for again"
        );
    }

    #[test]
    fn a_missing_object_on_the_logind_route_is_not_a_refusal() {
        struct Logind;
        impl InhibitBackend for Logind {
            type Handle = u32;
            fn route_of(_: u32) -> Route {
                Route::Desktop
            }
            async fn acquire(&self, _: Route) -> Result<u32, String> {
                Ok(3)
            }
            async fn release(&self, _: u32) -> Result<(), String> {
                Err("UnknownObject: Object does not exist at path /x".to_owned())
            }
        }
        let mut inhibitor = Inhibitor::<u32>::new();
        tauri::async_runtime::block_on(async {
            drive_inhibitor(&Logind, &mut inhibitor, true, Some(Route::Desktop)).await;
            drive_inhibitor(&Logind, &mut inhibitor, false, Some(Route::Desktop)).await;
        });
        assert!(!inhibitor.is_refused());
        assert_eq!(inhibitor.want(true), InhibitAction::Acquire);
    }

    #[test]
    fn a_refusal_makes_prevent_sleep_unavailable_with_the_reason() {
        let fine = availability(Platform::Linux, &portal(true, true), &desktop(&[]));
        assert!(
            with_refused_inhibit(fine.clone(), false)
                .prevent_sleep
                .available
        );
        let refused = with_refused_inhibit(fine, true);
        assert!(!refused.prevent_sleep.available);
        assert_eq!(
            refused.prevent_sleep.reason.as_deref(),
            Some(INHIBIT_REFUSED_REASON)
        );
        assert!(refused.notifications.available);
    }

    #[test]
    fn only_a_missing_request_looks_like_a_refusal() {
        assert!(request_is_gone(
            "Close: org.freedesktop.DBus.Error.UnknownMethod: Object does not exist at path /x"
        ));
        assert!(!request_is_gone("Close timed out"));
        assert!(!request_is_gone("the bus went away"));
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
    fn a_scheduled_job_that_starts_is_reported_once_and_an_ordinary_one_is_not() {
        let held = |phase| JobFacts {
            scheduled: true,
            ..job(1, phase)
        };
        let mut tracker = Tracker::default();
        assert!(tracker.observe(&[held(Phase::Queued)]).is_empty());
        let started = tracker.observe(&[held(Phase::Running)]);
        assert_eq!(started.len(), 1);
        assert!(matches!(&started[0], Transition::Started(j) if j.id == 1));
        assert!(tracker.observe(&[held(Phase::Running)]).is_empty());
        // A job that was not scheduled starts without a word, and so does one first seen running.
        let mut plain = Tracker::default();
        plain.observe(&[job(2, Phase::Queued)]);
        assert!(plain.observe(&[job(2, Phase::Running)]).is_empty());
        let mut late = Tracker::default();
        assert!(late.observe(&[held(Phase::Running)]).is_empty());
    }

    #[test]
    fn a_scheduled_start_notifies_only_while_no_window_has_focus_and_offers_details() {
        let started = Transition::Started(JobFacts {
            scheduled: true,
            ..job(1, Phase::Running)
        });
        assert_eq!(notice(&started, true), None);
        let told = notice(&started, false).unwrap();
        assert_eq!(told.id, notice_id(1));
        assert_eq!(told.body, "A scheduled job has started.");
        let buttons = transition_buttons(&started, None);
        assert_eq!(buttons.len(), 1);
        assert_eq!(parse_action(&buttons[0].id), Some((1, Act::ShowDetails)));
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

    // ---- notification buttons ----

    fn at(path: &str) -> Location {
        Location::new(path, format!("file://{path}"))
    }

    fn done(id: u64, kind: Kind, destination: Option<&str>) -> JobFacts {
        JobFacts {
            kind,
            destination: destination.map(at),
            ..job(id, Phase::Done)
        }
    }

    fn waiting(id: u64, wait: Wait) -> JobFacts {
        JobFacts {
            wait,
            ..job(id, Phase::Waiting)
        }
    }

    fn snapshot_of(kind: JobKind) -> waypoint_ops::JobSnapshot {
        waypoint_ops::JobSnapshot {
            id: waypoint_ops::JobId(1),
            kind,
            state: JobState::Running,
            title: "Job".into(),
            sources: Default::default(),
            destination: None,
            options: Default::default(),
            origin_window: "main-1".into(),
            counts: Default::default(),
            progress: Default::default(),
            created_ms: 0,
            started_ms: None,
            finished_ms: None,
            undoable: false,
            verified: None,
        }
    }

    fn labels(buttons: &[Button]) -> Vec<&str> {
        buttons.iter().map(|b| b.label.as_str()).collect()
    }

    #[test]
    fn a_finished_copy_offers_show_in_folder_and_never_undo() {
        let copy = done(4, Kind::Other, Some("/dest"));
        assert_eq!(labels(&buttons(&copy, true)), ["Show in folder"]);
        // A job with no destination (create, rename, delete) has nothing to show.
        assert!(buttons(&done(4, Kind::Other, None), true).is_empty());
    }

    #[test]
    fn a_finished_move_offers_show_in_folder_and_undo_while_it_is_the_top_of_the_journal() {
        let moved = done(5, Kind::Move, Some("/dest"));
        assert_eq!(labels(&buttons(&moved, true)), ["Show in folder", "Undo"]);
        // Another operation has come on top: undo would undo that one, so it is hidden.
        assert_eq!(labels(&buttons(&moved, false)), ["Show in folder"]);
    }

    #[test]
    fn a_trashed_job_offers_only_undo_and_only_while_it_can() {
        let trashed = done(6, Kind::Trash, None);
        assert_eq!(labels(&buttons(&trashed, true)), ["Undo"]);
        assert!(buttons(&trashed, false).is_empty());
    }

    #[test]
    fn a_failed_job_offers_show_details() {
        let failed = JobFacts {
            error: Some("boom".into()),
            destination: Some(at("/dest")),
            ..job(7, Phase::Failed)
        };
        assert_eq!(labels(&buttons(&failed, true)), ["Show details"]);
    }

    #[test]
    fn a_plain_conflict_offers_replace_skip_and_keep_both_and_never_apply_to_all() {
        let plain = waiting(8, Wait::PlainConflict { source: at("/a/x") });
        let offered = buttons(&plain, false);
        assert_eq!(labels(&offered), ["Replace", "Skip", "Keep both"]);
        assert!(offered.len() <= MAX_BUTTONS);
        assert!(offered.iter().all(|b| !b.id.contains("all")));
    }

    #[test]
    fn another_conflict_or_an_error_offers_show_only() {
        assert_eq!(
            labels(&buttons(&waiting(9, Wait::OtherConflict), false)),
            ["Show"]
        );
        assert_eq!(labels(&buttons(&waiting(9, Wait::Error), false)), ["Show"]);
        assert!(buttons(&waiting(9, Wait::None), false).is_empty());
    }

    #[test]
    fn a_job_that_is_neither_finished_nor_waiting_has_no_buttons() {
        for phase in [Phase::Running, Phase::Queued, Phase::Cancelled] {
            assert!(buttons(&job(1, phase), true).is_empty());
        }
    }

    #[test]
    fn a_transition_takes_its_buttons_from_the_job() {
        let moved = Transition::Finished(done(5, Kind::Move, Some("/dest")));
        assert_eq!(transition_buttons(&moved, Some(5)).len(), 2);
        assert_eq!(transition_buttons(&moved, Some(4)).len(), 1);
        assert_eq!(transition_buttons(&moved, None).len(), 1);
        let asks = Transition::NeedsAttention(waiting(2, Wait::Error));
        assert_eq!(labels(&transition_buttons(&asks, None)), ["Show"]);
    }

    #[test]
    fn button_ids_are_stable_unique_and_clear_of_the_reserved_ones() {
        let acts = [
            Act::ShowInFolder,
            Act::ShowDetails,
            Act::Show,
            Act::Resolve(ConflictPolicy::Replace),
            Act::Resolve(ConflictPolicy::Skip),
            Act::Resolve(ConflictPolicy::KeepBoth),
            Act::Undo,
        ];
        let ids: Vec<String> = acts.iter().map(|act| button_id(12, *act)).collect();
        assert_eq!(ids[0], "waypoint.job.12.folder");
        assert_eq!(ids[5], "waypoint.job.12.keep-both");
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
        for id in &ids {
            assert!(id.len() <= 256);
            assert!(!id.starts_with("app."));
            assert_ne!(id, "default");
            assert_ne!(id, RAISE_ACTION);
        }
        // The ids round-trip, whatever the job number.
        for act in acts {
            for job in [0, 1, 12, u64::MAX] {
                assert_eq!(parse_action(&button_id(job, act)), Some((job, act)));
            }
        }
    }

    #[test]
    fn malformed_or_foreign_ids_are_ignored() {
        for id in [
            "",
            "raise",
            "default",
            "app.open",
            "waypoint.job.",
            "waypoint.job.1",
            "waypoint.job.1.",
            "waypoint.job..folder",
            "waypoint.job.x.folder",
            "waypoint.job.-1.folder",
            "waypoint.job.+1.folder",
            "waypoint.job. 1.folder",
            "waypoint.job.99999999999999999999.folder",
            "waypoint.job.1.nothing",
            "waypoint.job.1.folder.extra",
            "waypoint.job.1.other",
            "other.job.1.folder",
            "Waypoint.job.1.folder",
        ] {
            assert_eq!(parse_action(id), None, "{id:?}");
            assert_eq!(
                command_for(id, &[done(1, Kind::Move, Some("/d"))], Some(1)),
                None
            );
        }
    }

    #[test]
    fn each_button_becomes_its_command() {
        let jobs = [
            done(1, Kind::Move, Some("/dest")),
            JobFacts {
                origin_window: "main-3".into(),
                ..job(2, Phase::Failed)
            },
            waiting(3, Wait::PlainConflict { source: at("/a/x") }),
            waiting(4, Wait::Error),
        ];
        assert_eq!(
            command_for(&button_id(1, Act::ShowInFolder), &jobs, None),
            Some(ActionCommand::OpenFolder {
                window: "main-1".into(),
                location: at("/dest")
            })
        );
        assert_eq!(
            command_for(&button_id(2, Act::ShowDetails), &jobs, None),
            Some(ActionCommand::OpenOperations)
        );
        assert_eq!(
            command_for(&button_id(4, Act::Show), &jobs, None),
            Some(ActionCommand::ShowQuestion {
                job: 4,
                window: "main-1".into()
            })
        );
        for policy in [
            ConflictPolicy::Replace,
            ConflictPolicy::Skip,
            ConflictPolicy::KeepBoth,
        ] {
            assert_eq!(
                command_for(&button_id(3, Act::Resolve(policy)), &jobs, None),
                Some(ActionCommand::Resolve {
                    job: 3,
                    source: at("/a/x"),
                    policy
                })
            );
        }
        assert_eq!(
            command_for(&button_id(1, Act::Undo), &jobs, Some(1)),
            Some(ActionCommand::Undo {
                job: 1,
                window: "main-1".into()
            })
        );
    }

    #[test]
    fn a_button_for_a_job_that_has_gone_opens_the_operations_window() {
        let jobs = [done(1, Kind::Move, Some("/dest"))];
        for act in [
            Act::ShowInFolder,
            Act::ShowDetails,
            Act::Show,
            Act::Resolve(ConflictPolicy::Replace),
            Act::Undo,
        ] {
            assert_eq!(
                command_for(&button_id(99, act), &jobs, Some(1)),
                Some(ActionCommand::OpenOperations)
            );
        }
        assert_eq!(
            command_for(&button_id(1, Act::Show), &[], None),
            Some(ActionCommand::OpenOperations)
        );
    }

    #[test]
    fn a_press_whose_moment_has_passed_opens_the_operations_window_instead() {
        // Undo is no longer the top of the journal, or nothing is.
        let moved = [done(1, Kind::Move, Some("/dest"))];
        assert_eq!(
            command_for(&button_id(1, Act::Undo), &moved, Some(2)),
            Some(ActionCommand::OpenOperations)
        );
        assert_eq!(
            command_for(&button_id(1, Act::Undo), &moved, None),
            Some(ActionCommand::OpenOperations)
        );
        // A copy is never undone from a notification, whatever the id says.
        let copy = [done(1, Kind::Other, Some("/dest"))];
        assert_eq!(
            command_for(&button_id(1, Act::Undo), &copy, Some(1)),
            Some(ActionCommand::OpenOperations)
        );
        // The conflict was answered elsewhere and the job runs on; or it now waits on several.
        let running = [job(3, Phase::Running)];
        let replace = button_id(3, Act::Resolve(ConflictPolicy::Replace));
        assert_eq!(
            command_for(&replace, &running, None),
            Some(ActionCommand::OpenOperations)
        );
        let several = [waiting(3, Wait::OtherConflict)];
        assert_eq!(
            command_for(&replace, &several, None),
            Some(ActionCommand::OpenOperations)
        );
        // Show for a question that is gone, and Show in folder for a job that did not finish.
        assert_eq!(
            command_for(&button_id(3, Act::Show), &running, None),
            Some(ActionCommand::OpenOperations)
        );
        let still_running = [JobFacts {
            destination: Some(at("/dest")),
            ..job(3, Phase::Running)
        }];
        assert_eq!(
            command_for(&button_id(3, Act::ShowInFolder), &still_running, None),
            Some(ActionCommand::OpenOperations)
        );
    }

    #[test]
    fn the_click_on_the_notification_is_not_a_button() {
        assert_eq!(command_for(RAISE_ACTION, &[], None), None);
    }

    #[test]
    fn buttons_are_sent_only_where_the_chosen_route_accepts_them() {
        let all = desktop(&ALL);
        // The portal accepts them: the portal route.
        assert!(actions_work(Route::Portal, &portal(true, true), &all));
        // A portal that is not there, a server that lists no `actions`: no buttons, only the click.
        assert!(!actions_work(Route::Portal, &portal(false, false), &all));
        let no_buttons = desktop(&[DesktopFeature::Notify]);
        assert!(!actions_work(
            Route::Desktop,
            &portal(false, false),
            &no_buttons
        ));
        assert!(actions_work(Route::Desktop, &portal(false, false), &all));
        // The page's switch follows the same status, with the reason in the Services panel.
        let bare = availability(
            Platform::Linux,
            &portal(false, false),
            &no_buttons,
            Availability::yes(),
        );
        assert!(bare.notifications.available);
        assert!(!bare.notification_actions.available);
        assert_eq!(
            bare.notification_actions.reason.as_deref(),
            Some("The notification server does not draw buttons, so notifications offer only a click.")
        );
        let fine = availability(
            Platform::Linux,
            &portal(false, false),
            &all,
            Availability::yes(),
        );
        assert!(fine.notification_actions.available);
        let summary = desktop_summary(&no_buttons);
        assert!(!summary.features.contains(&"notificationActions".to_owned()));
    }

    #[test]
    fn the_facts_of_a_waiting_job_say_what_it_waits_on() {
        use waypoint_ops::Conflict;
        let clash = |kind, within_batch| Conflict {
            source: at("/a/x"),
            existing: at("/b/x"),
            name: "x".into(),
            kind,
            within_batch,
            source_size: None,
            existing_size: None,
            source_modified_ms: None,
            existing_modified_ms: None,
        };
        let wait_of = |conflicts: Vec<Conflict>| {
            let snapshot = waypoint_ops::JobSnapshot {
                state: JobState::Waiting {
                    reason: WaitReason::Conflicts { conflicts },
                },
                ..snapshot_of(JobKind::Copy)
            };
            facts(&snapshot).wait
        };
        assert_eq!(
            wait_of(vec![clash(ConflictKind::FileOverFile, false)]),
            Wait::PlainConflict { source: at("/a/x") }
        );
        assert_eq!(
            wait_of(vec![clash(ConflictKind::FolderOverFolder, false)]),
            Wait::OtherConflict
        );
        assert_eq!(
            wait_of(vec![clash(ConflictKind::FileOverFile, true)]),
            Wait::OtherConflict
        );
        assert_eq!(
            wait_of(vec![
                clash(ConflictKind::FileOverFile, false),
                clash(ConflictKind::FileOverFile, false)
            ]),
            Wait::OtherConflict
        );
        assert_eq!(facts(&snapshot_of(JobKind::Move)).kind, Kind::Move,);
        assert_eq!(facts(&snapshot_of(JobKind::Trash)).kind, Kind::Trash);
        assert_eq!(facts(&snapshot_of(JobKind::Copy)).kind, Kind::Other);
    }
}

// The outbound drag state machine: the guards, the one-at-a-time rule, the self-drop record and how a drag's end is classified
//
// The platform code starts the real drag behind the `Driver` trait, so everything here runs headless.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{
    error::{Error, Result},
    models::{DragAction, DragEnded, DragOutcome, StartDragReport, StartDragRequest},
    uri,
};

/// How long after an outbound drag ends a drop of the same files still counts as its own. Windows delivers the drop to the page after the drag's modal loop has returned, and GTK can finish the drag before the drop event reaches the plugin.
pub const SELF_DROP_GRACE: Duration = Duration::from_millis(1500);

/// The actions a drag offers, as flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Actions {
    pub copy: bool,
    pub link: bool,
    pub r#move: bool,
}

impl Actions {
    pub fn from_list(list: &[DragAction]) -> Self {
        let mut actions = Actions::default();
        for action in list {
            match action {
                DragAction::Copy => actions.copy = true,
                DragAction::Move => actions.r#move = true,
                DragAction::Link => actions.link = true,
            }
        }
        actions
    }

    pub fn is_empty(&self) -> bool {
        !(self.copy || self.r#move || self.link)
    }
}

/// A request that has passed validation: normalised URIs and at least one action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragRequest {
    pub uris: Vec<String>,
    pub actions: Actions,
    pub icon: Option<Vec<u8>>,
}

impl DragRequest {
    pub fn validate(request: &StartDragRequest) -> Result<Self> {
        if request.uris.is_empty() {
            return Err(Error::Invalid("there are no files to drag".into()));
        }
        let actions = Actions::from_list(&request.actions);
        if actions.is_empty() {
            return Err(Error::Invalid("the drag offers no action".into()));
        }
        let mut uris = Vec::with_capacity(request.uris.len());
        for given in &request.uris {
            match uri::normalise(given.as_bytes()) {
                Some(uri) => uris.push(uri),
                None => return Err(Error::Invalid(format!("not a file URI: {given}"))),
            }
        }
        Ok(DragRequest {
            uris,
            actions,
            icon: request
                .icon
                .as_ref()
                .map(|icon| icon.png.clone())
                .filter(|png| !png.is_empty()),
        })
    }
}

/// Why a drag the system started did not complete, as the platform reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The user pressed Escape.
    UserCancelled,
    /// Released over something that takes no drop, or cancelled by the system without saying why. GDK on Wayland reports both Escape and a drop on nothing as a plain error, so they cannot be told apart there; both mean nothing was transferred.
    NoTarget,
    /// Anything else, with the platform's description.
    Other(String),
}

/// Classifies how a drag ended from what the target chose (`selected`, empty when it chose nothing), the failure the system reported, and whether the target asked the source to delete the originals (a completed move). Only that request makes a drop a move: a target that chose MOVE but never asked for the delete (it renamed the files itself, or only read them) is a `DroppedCopy`, so a caller never removes originals the target did not hand back.
pub fn classify(
    selected: Actions,
    failure: Option<&Failure>,
    deleted: bool,
) -> (DragOutcome, Option<String>) {
    match failure {
        Some(Failure::UserCancelled) | Some(Failure::NoTarget) => (DragOutcome::Cancelled, None),
        Some(Failure::Other(reason)) => (DragOutcome::Failed, Some(reason.clone())),
        None if deleted => (DragOutcome::DroppedMove, None),
        None if selected.link => (DragOutcome::DroppedLink, None),
        None if selected.copy || selected.r#move => (DragOutcome::DroppedCopy, None),
        None => (DragOutcome::Cancelled, None),
    }
}

/// OLE's `DROPEFFECT_*` bits and the `DoDragDrop` results the plugin reads. Compiled everywhere so the mapping is tested everywhere.
pub const DROPEFFECT_COPY: u32 = 1;
pub const DROPEFFECT_MOVE: u32 = 2;
pub const DROPEFFECT_LINK: u32 = 4;
pub const DRAGDROP_S_DROP: i32 = 0x0004_0100;
pub const DRAGDROP_S_CANCEL: i32 = 0x0004_0101;

/// The `DROPEFFECT_*` flags for the actions a drag offers.
pub fn ole_effects(actions: Actions) -> u32 {
    let mut effects = 0;
    if actions.copy {
        effects |= DROPEFFECT_COPY;
    }
    if actions.r#move {
        effects |= DROPEFFECT_MOVE;
    }
    if actions.link {
        effects |= DROPEFFECT_LINK;
    }
    effects
}

/// Classifies what `DoDragDrop` returned: its `HRESULT` and the effect the target performed.
pub fn classify_ole(hr: i32, effect: u32) -> (DragOutcome, Option<String>) {
    match hr {
        DRAGDROP_S_DROP => {
            let selected = Actions {
                copy: effect & DROPEFFECT_COPY != 0,
                r#move: effect & DROPEFFECT_MOVE != 0,
                link: effect & DROPEFFECT_LINK != 0,
            };
            // OLE reports a performed move as the effect itself: the source is to delete.
            classify(selected, None, selected.r#move)
        }
        DRAGDROP_S_CANCEL => (DragOutcome::Cancelled, None),
        other => (
            DragOutcome::Failed,
            Some(format!(
                "DoDragDrop failed with HRESULT {:#010x}",
                other as u32
            )),
        ),
    }
}

/// What a platform did when asked to start a drag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Begun {
    /// The drag runs on after the call returns; the platform reports its end through the `Finisher`.
    Running,
    /// The drag ran to its end inside the call.
    Done(DragOutcome, Option<String>),
}

/// Reports the end of a drag that was `Running`. Safe to call from the platform's event handlers; calling it a second time does nothing.
pub type Finisher = Arc<dyn Fn(DragOutcome, Option<String>) + Send + Sync>;

/// Receives the end of an outbound drag, to tell the page.
pub type EndSink = Arc<dyn Fn(DragEnded) + Send + Sync>;

/// What the state machine needs from a platform.
pub trait Driver {
    /// Whether outbound drags work here at all, and if not, why.
    fn available(&self) -> Result<()>;
    /// Whether the primary mouse button is down now.
    fn primary_button_down(&self) -> bool;
    /// Starts the drag; runs on the main thread.
    fn begin(&self, id: u32, request: &DragRequest, finisher: Finisher) -> Result<Begun>;
}

struct Active {
    id: u32,
    uris: Vec<String>,
}

struct Recent {
    uris: Vec<String>,
    ended: Instant,
}

#[derive(Default)]
struct Inner {
    next_id: u32,
    active: Option<Active>,
    recent: Option<Recent>,
}

/// The one outbound drag there can be at a time, and the record that tells a drop of it from a drop of someone else's files.
#[derive(Default)]
pub struct Outbound {
    inner: Mutex<Inner>,
}

fn sorted(uris: &[String]) -> Vec<&str> {
    let mut sorted: Vec<&str> = uris.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    sorted
}

impl Outbound {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Records a drag as started and returns its id, or `AlreadyActive`.
    pub fn begin(&self, uris: &[String]) -> Result<u32> {
        let mut inner = self.lock();
        if inner.active.is_some() {
            return Err(Error::AlreadyActive);
        }
        inner.next_id = inner.next_id.wrapping_add(1);
        let id = inner.next_id;
        inner.active = Some(Active {
            id,
            uris: uris.to_vec(),
        });
        Ok(id)
    }

    /// Records the end of drag `id`. `None` if it is not the active drag (it was already finished). The files stay recognisable as this process's own for `SELF_DROP_GRACE`.
    pub fn finish(
        &self,
        id: u32,
        outcome: DragOutcome,
        reason: Option<String>,
        now: Instant,
    ) -> Option<DragEnded> {
        let mut inner = self.lock();
        if inner.active.as_ref().map(|active| active.id) != Some(id) {
            return None;
        }
        let active = inner.active.take()?;
        inner.recent = Some(Recent {
            uris: active.uris.clone(),
            ended: now,
        });
        Some(DragEnded {
            id,
            outcome,
            uris: active.uris,
            reason,
        })
    }

    /// Forgets drag `id` without a trace, because it never started.
    pub fn abort(&self, id: u32) {
        let mut inner = self.lock();
        if inner.active.as_ref().map(|active| active.id) == Some(id) {
            inner.active = None;
        }
    }

    pub fn is_active(&self) -> bool {
        self.lock().active.is_some()
    }

    /// Whether a drop of `dropped` is the end of this process's own drag: the same files as the live drag, or as one that ended less than `SELF_DROP_GRACE` ago. The URIs must be normalised.
    pub fn is_self_drop(&self, dropped: &[String], now: Instant) -> bool {
        let mut inner = self.lock();
        if let Some(active) = &inner.active {
            return sorted(&active.uris) == sorted(dropped);
        }
        let expired = inner
            .recent
            .as_ref()
            .is_some_and(|recent| now.saturating_duration_since(recent.ended) > SELF_DROP_GRACE);
        if expired {
            inner.recent = None;
        }
        inner
            .recent
            .as_ref()
            .is_some_and(|recent| sorted(&recent.uris) == sorted(dropped))
    }

    /// The URIs of the live drag, for the drag's data handlers.
    pub fn active_uris(&self) -> Option<Vec<String>> {
        self.lock()
            .active
            .as_ref()
            .map(|active| active.uris.clone())
    }
}

/// Starts an outbound drag: checks the platform and the request, refuses a second drag and a drag with no button down, then asks the driver. `emit` receives the end of a drag that ends inside the call; a drag that is `Running` reports through its `Finisher`, which calls `emit` too.
pub fn start(
    outbound: &Arc<Outbound>,
    driver: &dyn Driver,
    request: &StartDragRequest,
    emit: EndSink,
) -> Result<StartDragReport> {
    driver.available()?;
    let request = DragRequest::validate(request)?;
    if outbound.is_active() {
        return Err(Error::AlreadyActive);
    }
    if !driver.primary_button_down() {
        return Err(Error::ButtonNotPressed);
    }
    let id = outbound.begin(&request.uris)?;
    let finisher: Finisher = {
        let (outbound, emit) = (outbound.clone(), emit.clone());
        Arc::new(move |outcome, reason| {
            if let Some(ended) = outbound.finish(id, outcome, reason, Instant::now()) {
                emit(ended);
            }
        })
    };
    match driver.begin(id, &request, finisher) {
        Ok(Begun::Running) => Ok(StartDragReport { id, ended: None }),
        Ok(Begun::Done(outcome, reason)) => {
            let ended = outbound.finish(id, outcome, reason, Instant::now());
            if let Some(ended) = &ended {
                emit(ended.clone());
            }
            Ok(StartDragReport { id, ended })
        }
        Err(error) => {
            outbound.abort(id);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;
    use crate::models::DragIcon;

    struct Fake {
        available: Result<()>,
        button: AtomicBool,
        begun: Mutex<Option<Result<Begun>>>,
        calls: AtomicUsize,
        finisher: Mutex<Option<Finisher>>,
    }

    impl Fake {
        fn new(result: Result<Begun>) -> Self {
            Fake {
                available: Ok(()),
                button: AtomicBool::new(true),
                begun: Mutex::new(Some(result)),
                calls: AtomicUsize::new(0),
                finisher: Mutex::new(None),
            }
        }
    }

    impl Driver for Fake {
        fn available(&self) -> Result<()> {
            self.available.clone()
        }
        fn primary_button_down(&self) -> bool {
            self.button.load(Ordering::SeqCst)
        }
        fn begin(&self, _id: u32, _request: &DragRequest, finisher: Finisher) -> Result<Begun> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.finisher.lock().unwrap() = Some(finisher);
            self.begun
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Ok(Begun::Running))
        }
    }

    fn request(uris: &[&str]) -> StartDragRequest {
        StartDragRequest {
            uris: uris.iter().map(|u| u.to_string()).collect(),
            actions: vec![DragAction::Copy, DragAction::Move],
            icon: None,
        }
    }

    fn sink() -> (EndSink, Arc<Mutex<Vec<DragEnded>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_in = seen.clone();
        (
            Arc::new(move |ended| seen_in.lock().unwrap().push(ended)),
            seen,
        )
    }

    #[test]
    fn a_drag_with_no_button_down_is_refused_before_anything_starts() {
        let outbound = Arc::new(Outbound::default());
        let fake = Fake::new(Ok(Begun::Running));
        fake.button.store(false, Ordering::SeqCst);
        let (emit, _) = sink();
        let error = start(&outbound, &fake, &request(&["file:///a"]), emit).unwrap_err();
        assert_eq!(error, Error::ButtonNotPressed);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
        assert!(!outbound.is_active());
    }

    #[test]
    fn an_unavailable_platform_reports_unsupported_first() {
        let outbound = Arc::new(Outbound::default());
        let mut fake = Fake::new(Ok(Begun::Running));
        fake.available = Err(Error::Unsupported("no display".into()));
        let (emit, _) = sink();
        let error = start(&outbound, &fake, &request(&["file:///a"]), emit).unwrap_err();
        assert_eq!(error, Error::Unsupported("no display".into()));
    }

    #[test]
    fn a_bad_request_is_invalid() {
        let outbound = Arc::new(Outbound::default());
        let fake = Fake::new(Ok(Begun::Running));
        let (emit, _) = sink();
        let none = StartDragRequest {
            uris: vec![],
            ..request(&[])
        };
        assert!(matches!(
            start(&outbound, &fake, &none, emit.clone()),
            Err(Error::Invalid(_))
        ));
        let no_action = StartDragRequest {
            actions: vec![],
            ..request(&["file:///a"])
        };
        assert!(matches!(
            start(&outbound, &fake, &no_action, emit.clone()),
            Err(Error::Invalid(_))
        ));
        let not_file = request(&["https://example.com/a"]);
        assert!(matches!(
            start(&outbound, &fake, &not_file, emit),
            Err(Error::Invalid(_))
        ));
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_second_drag_is_refused_while_one_runs_and_allowed_after_it_ends() {
        let outbound = Arc::new(Outbound::default());
        let first = Fake::new(Ok(Begun::Running));
        let (emit, seen) = sink();
        let report = start(&outbound, &first, &request(&["file:///a"]), emit.clone()).unwrap();
        assert_eq!(report.ended, None);
        assert!(outbound.is_active());

        let second = Fake::new(Ok(Begun::Running));
        assert_eq!(
            start(&outbound, &second, &request(&["file:///b"]), emit.clone()).unwrap_err(),
            Error::AlreadyActive
        );
        assert_eq!(second.calls.load(Ordering::SeqCst), 0);

        let finisher = first.finisher.lock().unwrap().clone().unwrap();
        finisher(DragOutcome::DroppedCopy, None);
        assert!(!outbound.is_active());
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            [DragEnded {
                id: report.id,
                outcome: DragOutcome::DroppedCopy,
                uris: vec!["file:///a".into()],
                reason: None
            }]
        );
        // Reporting the end twice does nothing the second time.
        finisher(DragOutcome::Cancelled, None);
        assert_eq!(seen.lock().unwrap().len(), 1);

        assert!(start(&outbound, &second, &request(&["file:///b"]), emit).is_ok());
    }

    #[test]
    fn a_drag_that_ends_inside_the_call_returns_its_end() {
        let outbound = Arc::new(Outbound::default());
        let fake = Fake::new(Ok(Begun::Done(DragOutcome::DroppedMove, None)));
        let (emit, seen) = sink();
        let report = start(&outbound, &fake, &request(&["file:///a"]), emit).unwrap();
        let ended = report.ended.expect("the drag ended inside the call");
        assert_eq!(ended.outcome, DragOutcome::DroppedMove);
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(!outbound.is_active());
    }

    #[test]
    fn a_drag_the_system_refuses_leaves_no_trace() {
        let outbound = Arc::new(Outbound::default());
        let fake = Fake::new(Err(Error::Failed("no context".into())));
        let (emit, seen) = sink();
        assert_eq!(
            start(&outbound, &fake, &request(&["file:///a"]), emit.clone()).unwrap_err(),
            Error::Failed("no context".into())
        );
        assert!(!outbound.is_active());
        assert!(seen.lock().unwrap().is_empty());
        assert!(!outbound.is_self_drop(&["file:///a".to_string()], Instant::now()));
        let again = Fake::new(Ok(Begun::Running));
        assert!(start(&outbound, &again, &request(&["file:///a"]), emit).is_ok());
    }

    #[test]
    fn the_request_is_normalised_and_an_empty_icon_is_none() {
        let mut given = request(&["file://localhost/t/a%20b", "FILE:///t/c"]);
        given.icon = Some(DragIcon { png: vec![] });
        let valid = DragRequest::validate(&given).unwrap();
        assert_eq!(
            valid.uris,
            vec!["file:///t/a%20b".to_string(), "file:///t/c".to_string()]
        );
        assert_eq!(valid.icon, None);
        assert_eq!(
            valid.actions,
            Actions {
                copy: true,
                r#move: true,
                link: false
            }
        );
        given.icon = Some(DragIcon { png: vec![1, 2] });
        assert_eq!(
            DragRequest::validate(&given).unwrap().icon,
            Some(vec![1, 2])
        );
    }

    #[test]
    fn a_drop_of_the_same_files_is_a_self_drop_while_live_and_for_a_moment_after() {
        let outbound = Outbound::default();
        let uris = vec!["file:///t/a".to_string(), "file:///t/b".to_string()];
        let reversed = vec!["file:///t/b".to_string(), "file:///t/a".to_string()];
        let other = vec!["file:///t/a".to_string()];
        let t0 = Instant::now();
        assert!(!outbound.is_self_drop(&uris, t0));

        let id = outbound.begin(&uris).unwrap();
        assert!(outbound.is_self_drop(&reversed, t0));
        assert!(!outbound.is_self_drop(&other, t0));
        assert_eq!(outbound.active_uris(), Some(uris.clone()));

        outbound
            .finish(id, DragOutcome::DroppedCopy, None, t0)
            .unwrap();
        assert!(outbound.is_self_drop(&uris, t0 + Duration::from_millis(100)));
        assert!(!outbound.is_self_drop(&other, t0 + Duration::from_millis(100)));
        assert!(!outbound.is_self_drop(&uris, t0 + SELF_DROP_GRACE + Duration::from_millis(1)));
        // Once expired it stays forgotten.
        assert!(!outbound.is_self_drop(&uris, t0));
        assert_eq!(outbound.active_uris(), None);
    }

    #[test]
    fn drag_ids_count_up() {
        let outbound = Outbound::default();
        let a = outbound.begin(&["file:///a".into()]).unwrap();
        outbound.finish(a, DragOutcome::Cancelled, None, Instant::now());
        let b = outbound.begin(&["file:///a".into()]).unwrap();
        assert_eq!(b, a + 1);
        assert_eq!(
            outbound.finish(a, DragOutcome::Cancelled, None, Instant::now()),
            None
        );
    }

    #[test]
    fn ole_results_map_to_outcomes() {
        assert_eq!(
            classify_ole(DRAGDROP_S_DROP, DROPEFFECT_COPY).0,
            DragOutcome::DroppedCopy
        );
        assert_eq!(
            classify_ole(DRAGDROP_S_DROP, DROPEFFECT_MOVE).0,
            DragOutcome::DroppedMove
        );
        assert_eq!(
            classify_ole(DRAGDROP_S_DROP, DROPEFFECT_LINK).0,
            DragOutcome::DroppedLink
        );
        assert_eq!(classify_ole(DRAGDROP_S_DROP, 0).0, DragOutcome::Cancelled);
        assert_eq!(classify_ole(DRAGDROP_S_CANCEL, 0).0, DragOutcome::Cancelled);
        let (outcome, reason) = classify_ole(0x8000_4005_u32 as i32, 0);
        assert_eq!(outcome, DragOutcome::Failed);
        assert!(reason.unwrap().contains("0x80004005"));
    }

    #[test]
    fn actions_map_to_ole_effects() {
        assert_eq!(
            ole_effects(Actions {
                copy: true,
                r#move: true,
                link: true
            }),
            7
        );
        assert_eq!(
            ole_effects(Actions {
                copy: true,
                ..Actions::default()
            }),
            1
        );
        assert_eq!(
            ole_effects(Actions {
                r#move: true,
                ..Actions::default()
            }),
            2
        );
        assert_eq!(ole_effects(Actions::default()), 0);
    }

    #[test]
    fn ends_are_classified_from_the_action_and_the_failure() {
        let copy = Actions {
            copy: true,
            ..Actions::default()
        };
        let mv = Actions {
            r#move: true,
            ..Actions::default()
        };
        let link = Actions {
            link: true,
            ..Actions::default()
        };
        let none = Actions::default();
        assert_eq!(classify(copy, None, false).0, DragOutcome::DroppedCopy);
        // A chosen MOVE the target never asked the source to delete is not a move.
        assert_eq!(classify(mv, None, false).0, DragOutcome::DroppedCopy);
        assert_eq!(classify(mv, None, true).0, DragOutcome::DroppedMove);
        assert_eq!(classify(link, None, false).0, DragOutcome::DroppedLink);
        assert_eq!(classify(copy, None, true).0, DragOutcome::DroppedMove);
        assert_eq!(classify(none, None, false).0, DragOutcome::Cancelled);
        assert_eq!(
            classify(copy, Some(&Failure::UserCancelled), false).0,
            DragOutcome::Cancelled
        );
        assert_eq!(
            classify(none, Some(&Failure::NoTarget), false).0,
            DragOutcome::Cancelled
        );
        assert_eq!(
            classify(none, Some(&Failure::Other("grab broken".into())), false),
            (DragOutcome::Failed, Some("grab broken".to_string()))
        );
    }
}

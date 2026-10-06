// Windows: finds the window of this application under an outbound drag's cursor and sends it `enter`, `over` and `leave`
//
// `DoDragDrop` is a modal loop started from the event loop's handler, so the webview's own drag events for a window of this process wait until it returns. While it runs, the drop source reports the cursor here; the window under it is told through the plugin's own events, which tauri-runtime-wry delivers at once when sent from the main thread. The decisions are the pure `hover` module's; this is the Win32 glue.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};

use log::warn;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use windows::Win32::Foundation::HWND;
use windows::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::ScreenToClient,
    System::Threading::GetCurrentProcessId,
    UI::WindowsAndMessaging::{
        GetAncestor, GetClientRect, GetCursorPos, GetWindow, GetWindowRect,
        GetWindowThreadProcessId, IsWindowVisible, WindowFromPoint, GA_ROOT, GW_HWNDNEXT,
    },
};

use crate::{
    hover::{Hover, Out, Sample, SETTLE},
    inbound::{normalise_position, PositionUnit},
    models::{EnterEvent, LeaveEvent, OverEvent, ENTER_EVENT, LEAVE_EVENT, OVER_EVENT},
    platform::modifiers_now,
    uri, NativeDnd,
};

enum Step {
    Sample,
    Ended { cancelled: bool },
}

/// What the drop source calls as the drag runs and ends.
#[derive(Clone)]
pub struct Tracker(Rc<dyn Fn(Step)>);

impl Tracker {
    /// A tracker for a drag of `uris`, or `None` if the plugin's state is not there.
    pub fn new<R: Runtime>(app: &AppHandle<R>, uris: &[String]) -> Option<Tracker> {
        let hover = app.try_state::<NativeDnd<R>>()?.hover();
        hover.lock().ok()?.begin();
        let paths: Vec<String> = uris
            .iter()
            .map(|u| {
                uri::uri_to_path(u).map_or_else(|| u.clone(), |p| p.to_string_lossy().into_owned())
            })
            .collect();
        let uris = uris.to_vec();
        let app = app.clone();
        Some(Tracker(Rc::new(move |step| {
            let (outs, expire) = match step {
                Step::Sample => {
                    let at = under_cursor(&app);
                    (apply(&hover, |h| h.sample(at)), false)
                }
                Step::Ended { cancelled } => {
                    let outs = apply(&hover, |h| h.end(cancelled, Instant::now()));
                    (outs, !cancelled)
                }
            };
            send(&app, &uris, &paths, outs);
            if expire {
                schedule_expiry(app.clone(), hover.clone(), uris.clone(), paths.clone());
            }
        })))
    }

    /// The cursor moved, or time passed.
    pub fn sample(&self) {
        (self.0)(Step::Sample);
    }

    /// The drag ended: abandoned (`cancelled`) or released.
    pub fn ended(&self, cancelled: bool) {
        (self.0)(Step::Ended { cancelled });
    }
}

fn apply<T>(hover: &Mutex<Hover>, step: impl FnOnce(&mut Hover) -> T) -> T
where
    T: Default,
{
    hover.lock().map(|mut h| step(&mut h)).unwrap_or_default()
}

/// After a release, tells a window still holding the files that they left if the platform's own drop never came.
fn schedule_expiry<R: Runtime>(
    app: AppHandle<R>,
    hover: Arc<Mutex<Hover>>,
    uris: Vec<String>,
    paths: Vec<String>,
) {
    std::thread::spawn(move || {
        std::thread::sleep(SETTLE);
        let outs = apply(&hover, |h| h.expire(Instant::now()));
        send(&app, &uris, &paths, outs);
    });
}

fn send<R: Runtime>(app: &AppHandle<R>, uris: &[String], paths: &[String], outs: Vec<Out>) {
    for out in outs {
        let result = match &out {
            Out::Enter(sample) => app.emit_to(
                sample.window.as_str(),
                ENTER_EVENT,
                EnterEvent {
                    window: sample.window.clone(),
                    paths: paths.to_vec(),
                    uris: uris.to_vec(),
                    position: position(app, sample),
                    modifiers: sample.modifiers,
                    // Windows' drop target does not negotiate an action while the loop is ours.
                    action: None,
                },
            ),
            Out::Over(sample) => app.emit_to(
                sample.window.as_str(),
                OVER_EVENT,
                OverEvent {
                    window: sample.window.clone(),
                    position: position(app, sample),
                    modifiers: sample.modifiers,
                    action: None,
                },
            ),
            Out::Leave(window) => app.emit_to(
                window.as_str(),
                LEAVE_EVENT,
                LeaveEvent {
                    window: window.clone(),
                },
            ),
        };
        if let Err(error) = result {
            warn!("native-dnd: cannot report the drag to a window: {error}");
        }
    }
}

fn position<R: Runtime>(app: &AppHandle<R>, sample: &Sample) -> crate::models::Position {
    let scale = app
        .get_webview_window(&sample.window)
        .and_then(|window| window.scale_factor().ok())
        .unwrap_or(1.0);
    normalise_position(sample.position, PositionUnit::Physical, scale)
}

fn contains(rect: &RECT, at: POINT) -> bool {
    at.x >= rect.left && at.x < rect.right && at.y >= rect.top && at.y < rect.bottom
}

/// The window of this application under the cursor, with the cursor in its physical client pixels; `None` over anything else or over a window's frame.
fn under_cursor<R: Runtime>(app: &AppHandle<R>) -> Option<Sample> {
    let mut at = POINT::default();
    // SAFETY: `at` is a valid out pointer.
    unsafe { GetCursorPos(&mut at) }.ok()?;
    let ours: Vec<(String, HWND)> = app
        .webview_windows()
        .into_iter()
        .filter_map(|(label, window)| {
            let hwnd = window.hwnd().ok()?;
            Some((label, HWND(hwnd.0)))
        })
        .collect();
    let (window, hwnd) = top_level_under(at, &ours)?;
    let mut client = RECT::default();
    let mut local = at;
    // SAFETY: `hwnd` is a live window of this process and both pointers are valid.
    unsafe {
        GetClientRect(hwnd, &mut client).ok()?;
        if !ScreenToClient(hwnd, &mut local).as_bool() {
            return None;
        }
    }
    if !contains(&client, local) {
        return None;
    }
    Some(Sample {
        window,
        position: (f64::from(local.x), f64::from(local.y)),
        modifiers: modifiers_now(),
    })
}

/// Which of `ours` is under `at`. The top-level window the system finds there is used; when it is another window of this process (the drag image the shell draws is one) the windows beneath it are tried in turn, and a window of another process ends the search.
fn top_level_under(at: POINT, ours: &[(String, HWND)]) -> Option<(String, HWND)> {
    let find = |hwnd: HWND| ours.iter().find(|(_, h)| *h == hwnd).cloned();
    // SAFETY: plain window queries by handle; a window that has gone just fails them.
    unsafe {
        let root = GetAncestor(WindowFromPoint(at), GA_ROOT);
        if root.0.is_null() {
            return None;
        }
        if let Some(found) = find(root) {
            return Some(found);
        }
        let ours_pid = GetCurrentProcessId();
        let mut current = root;
        // Bounded: the z-order is short where the cursor is, and a stale list must not loop.
        for _ in 0..64 {
            let mut pid = 0u32;
            GetWindowThreadProcessId(current, Some(&mut pid));
            let mut rect = RECT::default();
            let here = IsWindowVisible(current).as_bool()
                && GetWindowRect(current, &mut rect).is_ok()
                && contains(&rect, at);
            if here {
                if pid != ours_pid {
                    return None;
                }
                if let Some(found) = find(current) {
                    return Some(found);
                }
            }
            current = GetWindow(current, GW_HWNDNEXT).ok()?;
        }
        None
    }
}

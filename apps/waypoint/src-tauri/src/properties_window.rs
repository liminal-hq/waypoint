// The Properties windows: standalone windows about one entry or folder each, made by a factory (at most four, the same subject reuses its window)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{
    AppHandle, Manager, Runtime, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window,
};
use waypoint_protocol::Location;

/// The most Properties windows open at once (D122).
pub const MAX_WINDOWS: usize = 4;

/// The start of every label `WindowKind::Properties` routes to `PropertiesScreen`.
pub const LABEL_PREFIX: &str = "properties-";

const SIZE: (f64, f64) = (460.0, 640.0);
const MIN_SIZE: (f64, f64) = (380.0, 420.0);

/// What asking for a Properties window did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// A new window was made.
    Opened,
    /// The subject already had a window, which was brought to the front.
    Focused,
    /// Four windows are open already and none is about this subject; nothing was made.
    Limit,
}

/// What to do about a request, decided from the windows that are open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Bring this window to the front.
    Focus(String),
    /// Make this window.
    Create(String),
    /// Refuse: the limit is reached.
    Refuse,
}

/// Names the subject of a window so the same folder or file is recognised however its location was
/// written: a trailing slash does not make a different subject.
pub fn subject_key(uri: &str) -> &str {
    let trimmed = uri.trim_end_matches('/');
    // A root (`file:///`) keeps its slash; trimming would leave a bare scheme.
    if trimmed.is_empty() || trimmed.ends_with(':') {
        uri
    } else {
        trimmed
    }
}

/// Decides what a request for `subject` does, given the `(label, subject)` of every window that is
/// open and the number the next label would carry. The same subject reuses its window (D122), and
/// a fifth window is refused. Pure, so the limit and the reuse are tested without an app.
pub fn plan(open: &[(String, String)], subject: &str, next: u32) -> Plan {
    let key = subject_key(subject);
    if let Some((label, _)) = open.iter().find(|(_, s)| subject_key(s) == key) {
        return Plan::Focus(label.clone());
    }
    if open.len() >= MAX_WINDOWS {
        return Plan::Refuse;
    }
    Plan::Create(format!("{LABEL_PREFIX}{next}"))
}

#[derive(Default)]
struct Inner {
    subjects: BTreeMap<String, Location>,
    next: u32,
}

/// Which Properties window is about which location. The page of a window asks for its own, and
/// tells Rust when it follows the entry to a new name.
#[derive(Default)]
pub struct PropertiesWindows(Mutex<Inner>);

impl PropertiesWindows {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Forgets a window that has closed.
    pub fn forget(&self, label: &str) {
        self.lock().subjects.remove(label);
    }

    fn open_list(inner: &Inner) -> Vec<(String, String)> {
        inner
            .subjects
            .iter()
            .map(|(label, location)| (label.clone(), location.uri.clone()))
            .collect()
    }
}

fn bring_to_front<R: Runtime>(window: &WebviewWindow<R>) -> Result<(), String> {
    // A minimised or hidden window would otherwise stay out of sight after the request.
    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

/// Makes the window `label`: the same frameless, transparent window the main windows are, so the
/// shared chrome draws it.
fn build<R: Runtime>(app: &AppHandle<R>, label: &str) -> Result<WebviewWindow<R>, String> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Properties")
        .inner_size(SIZE.0, SIZE.1)
        .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(window)
}

/// Opens the Properties window for `location`, or brings its window to the front.
pub fn open<R: Runtime>(
    app: &AppHandle<R>,
    windows: &PropertiesWindows,
    location: Location,
) -> Result<Outcome, String> {
    let label = {
        let mut inner = windows.lock();
        // A window that is gone without having said so (it failed to build) must not hold a place.
        inner
            .subjects
            .retain(|label, _| app.get_webview_window(label).is_some());
        inner.next += 1;
        match plan(
            &PropertiesWindows::open_list(&inner),
            &location.uri,
            inner.next,
        ) {
            Plan::Refuse => return Ok(Outcome::Limit),
            Plan::Focus(label) => {
                drop(inner);
                if let Some(window) = app.get_webview_window(&label) {
                    bring_to_front(&window)?;
                }
                return Ok(Outcome::Focused);
            }
            Plan::Create(label) => {
                inner.subjects.insert(label.clone(), location);
                label
            }
        }
    };
    match build(app, &label) {
        Ok(_) => Ok(Outcome::Opened),
        Err(error) => {
            windows.forget(&label);
            Err(error)
        }
    }
}

/// Opens (or focuses) the Properties window for a location. Async so the window is made off the
/// main thread, which `WebviewWindowBuilder` requires on Windows.
#[tauri::command]
pub async fn open_properties_window(
    app: AppHandle,
    windows: State<'_, PropertiesWindows>,
    location: Location,
) -> Result<Outcome, String> {
    open(&app, &windows, location)
}

/// What the calling Properties window is about.
#[tauri::command]
pub fn properties_subject<R: Runtime>(
    window: Window<R>,
    windows: State<'_, PropertiesWindows>,
) -> Result<Location, String> {
    windows
        .lock()
        .subjects
        .get(window.label())
        .cloned()
        .ok_or_else(|| format!("`{}` is not a Properties window", window.label()))
}

/// Records that the calling window now follows `location` (the entry was renamed), so a request for
/// the new name finds this window.
#[tauri::command]
pub fn properties_set_subject<R: Runtime>(
    window: Window<R>,
    windows: State<'_, PropertiesWindows>,
    location: Location,
) -> Result<(), String> {
    let mut inner = windows.lock();
    match inner.subjects.get_mut(window.label()) {
        Some(subject) => {
            *subject = location;
            Ok(())
        }
        None => Err(format!("`{}` is not a Properties window", window.label())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    fn open_set(subjects: &[&str]) -> Vec<(String, String)> {
        subjects
            .iter()
            .enumerate()
            .map(|(i, s)| (format!("properties-{}", i + 1), (*s).to_owned()))
            .collect()
    }

    #[test]
    fn the_label_is_the_one_the_front_end_routes_to_the_properties_screen() {
        assert_eq!(
            waypoint_protocol::WindowKind::from_label(&format!("{LABEL_PREFIX}7")),
            Some(waypoint_protocol::WindowKind::Properties)
        );
    }

    #[test]
    fn a_new_subject_gets_the_next_numbered_label() {
        let open = open_set(&["file:///a"]);
        assert_eq!(
            plan(&open, "file:///b", 2),
            Plan::Create("properties-2".into())
        );
    }

    #[test]
    fn the_same_subject_reuses_its_window() {
        let open = open_set(&["file:///a", "file:///b"]);
        assert_eq!(
            plan(&open, "file:///b", 3),
            Plan::Focus("properties-2".into())
        );
    }

    #[test]
    fn a_trailing_slash_is_the_same_subject() {
        let open = open_set(&["file:///home/me/docs"]);
        assert_eq!(
            plan(&open, "file:///home/me/docs/", 2),
            Plan::Focus("properties-1".into())
        );
    }

    #[test]
    fn the_root_keeps_its_slash() {
        assert_eq!(subject_key("file:///"), "file:///");
        assert_eq!(subject_key("file:///a/"), "file:///a");
    }

    #[test]
    fn a_fifth_window_is_refused() {
        let open = open_set(&["file:///a", "file:///b", "file:///c", "file:///d"]);
        assert_eq!(plan(&open, "file:///e", 5), Plan::Refuse);
    }

    #[test]
    fn at_the_limit_the_same_subject_still_reuses_its_window() {
        let open = open_set(&["file:///a", "file:///b", "file:///c", "file:///d"]);
        assert_eq!(
            plan(&open, "file:///c", 5),
            Plan::Focus("properties-3".into())
        );
    }

    #[test]
    fn closing_a_window_frees_its_place() {
        let windows = PropertiesWindows::default();
        {
            let mut inner = windows.lock();
            for n in 1..=MAX_WINDOWS {
                inner.subjects.insert(
                    format!("properties-{n}"),
                    Location {
                        display: format!("/{n}"),
                        uri: format!("file:///{n}"),
                    },
                );
            }
            let list = PropertiesWindows::open_list(&inner);
            assert_eq!(plan(&list, "file:///x", 9), Plan::Refuse);
        }
        windows.forget("properties-2");
        let list = PropertiesWindows::open_list(&windows.lock());
        assert_eq!(
            plan(&list, "file:///x", 9),
            Plan::Create("properties-9".into())
        );
    }

    #[test]
    fn a_request_makes_a_window_and_the_same_request_focuses_it() {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("the mock app builds");
        let windows = PropertiesWindows::default();
        let location = Location {
            display: "/home/me".into(),
            uri: "file:///home/me".into(),
        };
        assert_eq!(
            open(app.handle(), &windows, location.clone()),
            Ok(Outcome::Opened)
        );
        assert_eq!(open(app.handle(), &windows, location), Ok(Outcome::Focused));
        let made = app
            .webview_windows()
            .into_keys()
            .filter(|label| label.starts_with(LABEL_PREFIX))
            .count();
        assert_eq!(made, 1);
    }
}

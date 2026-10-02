// Plugin-level tests: the commands run against a mock Tauri app with real windows and a recording
// window factory
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{AppHandle, Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use waypoint_protocol::Location;
use waypoint_session::{
    Command, Geometry, MoveTo, MoveWhat, PairLayout, SessionEvent, StorePolicy, TabId,
};

use crate::commands;
use crate::{
    init, Error, MemoryStorage, SessionDeps, Sessions, WindowError, WindowFactory, EVENT,
    MAX_WINDOWS, WARN_WINDOWS,
};

type App = tauri::App<MockRuntime>;

/// Records the windows it is asked for and builds them as mock windows, unless told to fail.
#[derive(Default)]
struct Factory {
    created: Mutex<Vec<(String, Option<Geometry>)>>,
    fail: Mutex<Option<WindowError>>,
}

impl WindowFactory<MockRuntime> for Factory {
    fn create(
        &self,
        app: &AppHandle<MockRuntime>,
        label: &str,
        geometry: Option<&Geometry>,
        _opener: Option<&str>,
    ) -> Result<(), WindowError> {
        if let Some(e) = self.fail.lock().unwrap().clone() {
            return Err(e);
        }
        WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
            .build()
            .map_err(|e| WindowError::Failed(e.to_string()))?;
        self.created
            .lock()
            .unwrap()
            .push((label.to_string(), geometry.copied()));
        Ok(())
    }
}

struct Setup {
    app: App,
    factory: Arc<Factory>,
}

fn setup_with(
    policy: StorePolicy,
    tweak: impl FnOnce(&mut SessionDeps<MockRuntime>),
    labels: &[&str],
) -> Setup {
    let factory = Arc::new(Factory::default());
    let mut deps = SessionDeps::new(
        factory.clone() as Arc<dyn WindowFactory<MockRuntime>>,
        Arc::new(MemoryStorage::default()),
        policy,
    );
    tweak(&mut deps);
    let app = mock_builder()
        .plugin(init(deps))
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    for label in labels {
        WebviewWindowBuilder::new(&app, *label, WebviewUrl::App("index.html".into()))
            .build()
            .expect("the mock window opens");
    }
    Setup { app, factory }
}

fn m2_policy() -> StorePolicy {
    StorePolicy {
        close_window_on_last_tab: false,
    }
}

fn setup(labels: &[&str]) -> Setup {
    setup_with(m2_policy(), |_| {}, labels)
}

fn window(app: &App, label: &str) -> tauri::WebviewWindow<MockRuntime> {
    app.get_webview_window(label).expect("the window exists")
}

fn sessions(app: &App) -> tauri::State<'_, Sessions<MockRuntime>> {
    app.state::<Sessions<MockRuntime>>()
}

fn events(app: &App, label: &str) -> Receiver<SessionEvent> {
    let (sender, receiver) = channel();
    window(app, label).listen(EVENT, move |event| {
        let _ = sender.send(serde_json::from_str(event.payload()).expect("a session event"));
    });
    receiver
}

fn drain(receiver: &Receiver<SessionEvent>) -> Vec<SessionEvent> {
    std::iter::from_fn(|| receiver.try_recv().ok()).collect()
}

fn loc(name: &str) -> Location {
    Location::new(format!("/{name}"), format!("file:///{name}"))
}

fn open_tab(app: &App, label: &str, name: &str) -> Result<TabId, Error> {
    tauri::async_runtime::block_on(commands::open_tab(
        window(app, label),
        sessions(app),
        loc(name),
        None,
        true,
    ))
}

fn wait_until(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "timed out: {what}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The mock runtime does not run the event loop, so it never reports a window as destroyed; this
/// stands in for the run event a real app gets.
fn destroyed(app: &App, label: &str) {
    crate::on_window_event(app.handle(), label, &tauri::WindowEvent::Destroyed);
}

fn tab_count(app: &App, label: &str) -> usize {
    sessions(app).with_store(|s| s.window(label).map_or(0, |w| w.tabs.len()))
}

#[test]
fn a_main_window_is_registered_on_its_first_snapshot_and_other_windows_are_refused() {
    let t = setup(&["main-7", "settings"]);
    let snapshot = tauri::async_runtime::block_on(commands::get_snapshot(
        window(&t.app, "main-7"),
        sessions(&t.app),
    ))
    .unwrap();
    assert!(snapshot.tabs.is_empty() && snapshot.active.is_none());
    assert!(sessions(&t.app).with_store(|s| s.window("main-7").is_some()));

    // The labels the store allocates afterwards stay above the one it was given.
    let label = tauri::async_runtime::block_on(commands::open_window(
        window(&t.app, "main-7"),
        sessions(&t.app),
        Some(loc("a")),
        None,
    ))
    .unwrap();
    assert_eq!(label, "main-8");

    let refused = tauri::async_runtime::block_on(commands::get_snapshot(
        window(&t.app, "settings"),
        sessions(&t.app),
    ));
    assert!(refused.is_err());
}

#[test]
fn a_window_made_after_the_new_window_view_is_set_starts_with_it_and_a_restore_keeps_it() {
    use waypoint_session::{ViewMode, ViewPrefs};
    let t = setup(&["main-1", "main-2", "main-3"]);
    let grid = ViewPrefs {
        mode: ViewMode::Grid,
        show_hidden: true,
        icon_size: 64,
        ..ViewPrefs::default()
    };
    let view_of = |label: &str| {
        tauri::async_runtime::block_on(commands::get_snapshot(
            window(&t.app, label),
            sessions(&t.app),
        ))
        .unwrap()
        .view
    };
    assert_eq!(view_of("main-1"), ViewPrefs::default());
    sessions(&t.app).set_new_window_view(grid);
    assert_eq!(
        view_of("main-1"),
        ViewPrefs::default(),
        "an existing window keeps its view"
    );
    assert_eq!(view_of("main-2"), grid);
    // A store restored from a document gets the choice too.
    sessions(&t.app).restore(waypoint_session::Store::new());
    assert_eq!(view_of("main-3"), grid);
}

#[test]
fn commands_act_on_the_calling_window_and_events_reach_only_that_window() {
    let t = setup(&["main-1", "main-2"]);
    let one = events(&t.app, "main-1");
    let two = events(&t.app, "main-2");
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    let b = open_tab(&t.app, "main-2", "b").unwrap();
    assert_ne!(a, b, "tab ids are global");

    let got_one = drain(&one);
    let got_two = drain(&two);
    assert!(got_one
        .iter()
        .any(|e| matches!(e, SessionEvent::TabOpened { tab, .. } if tab.id == a)));
    assert!(!got_one
        .iter()
        .any(|e| matches!(e, SessionEvent::TabOpened { tab, .. } if tab.id == b)));
    assert!(got_two
        .iter()
        .any(|e| matches!(e, SessionEvent::TabOpened { tab, .. } if tab.id == b)));
    assert!(!got_two
        .iter()
        .any(|e| matches!(e, SessionEvent::TabOpened { tab, .. } if tab.id == a)));
}

#[test]
fn events_arrive_in_revision_order_under_concurrent_commands() {
    let t = setup(&["main-1", "main-2"]);
    let receivers = [events(&t.app, "main-1"), events(&t.app, "main-2")];
    std::thread::scope(|scope| {
        for n in 0..8 {
            let handle = t.app.handle().clone();
            scope.spawn(move || {
                let label = if n % 2 == 0 { "main-1" } else { "main-2" };
                let sessions = handle.state::<Sessions<MockRuntime>>();
                for i in 0..25 {
                    let command = if i % 5 == 4 {
                        Command::SetView {
                            view: Default::default(),
                        }
                    } else {
                        Command::Open {
                            location: loc(&format!("t{n}-{i}")),
                            after: None,
                            activate: i % 2 == 0,
                        }
                    };
                    sessions.run(&handle, label, command).unwrap();
                    std::thread::yield_now();
                }
            });
        }
    });
    for (receiver, label) in receivers.iter().zip(["main-1", "main-2"]) {
        let got = drain(receiver);
        assert!(!got.is_empty());
        let revisions: Vec<u64> = got.iter().map(SessionEvent::revision).collect();
        assert!(
            revisions.windows(2).all(|w| w[0] < w[1]),
            "{label} out of order"
        );
        // Every tab the window heard about is one it holds now.
        sessions(&t.app).with_store(|s| {
            let w = s.window(label).unwrap();
            for e in &got {
                if let SessionEvent::TabOpened { tab, .. } = e {
                    assert!(
                        w.index_of(tab.id).is_some(),
                        "{label} heard of a foreign tab"
                    );
                }
            }
        });
    }
    assert_eq!(
        tab_count(&t.app, "main-1") + tab_count(&t.app, "main-2"),
        8 * 20
    );
}

#[test]
fn moving_tabs_to_a_new_window_creates_it_through_the_factory() {
    let t = setup(&["main-1"]);
    let source = events(&t.app, "main-1");
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    let b = open_tab(&t.app, "main-1", "b").unwrap();
    drain(&source);
    let closed = Arc::new(Mutex::new(Vec::new()));
    let sink = closed.clone();
    sessions(&t.app).on_tab_closed(move |w, tab| sink.lock().unwrap().push((w.to_string(), tab)));

    let label = tauri::async_runtime::block_on(commands::move_tabs(
        window(&t.app, "main-1"),
        sessions(&t.app),
        MoveWhat::Tabs(vec![b]),
        MoveTo::NewWindow {
            label: None,
            geometry: Some(Geometry {
                x: Some(10),
                y: None,
                width: 800,
                height: 600,
                maximised: false,
            }),
        },
    ))
    .unwrap();
    assert_eq!(label, "main-2");
    let created = t.factory.created.lock().unwrap().clone();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].0, "main-2");
    assert_eq!(created[0].1.map(|g| g.width), Some(800));
    assert!(t.app.get_webview_window("main-2").is_some());

    // The new window finds its tab through its snapshot; the source heard the tab leave.
    let snapshot = tauri::async_runtime::block_on(commands::get_snapshot(
        window(&t.app, "main-2"),
        sessions(&t.app),
    ))
    .unwrap();
    assert_eq!(
        snapshot.tabs.iter().map(|x| x.id).collect::<Vec<_>>(),
        vec![b]
    );
    assert!(drain(&source)
        .iter()
        .any(|e| matches!(e, SessionEvent::TabClosed { tab, .. } if *tab == b)));
    assert_eq!(tab_count(&t.app, "main-1"), 1);
    assert!(
        closed.lock().unwrap().is_empty(),
        "a hand-off is not a close ({a:?})"
    );
}

#[test]
fn a_failing_factory_leaves_the_store_and_the_windows_unchanged() {
    let t = setup(&["main-1"]);
    let source = events(&t.app, "main-1");
    open_tab(&t.app, "main-1", "a").unwrap();
    let b = open_tab(&t.app, "main-1", "b").unwrap();
    drain(&source);
    let before = sessions(&t.app).with_store(|s| s.clone());
    *t.factory.fail.lock().unwrap() = Some(WindowError::NotAvailable);

    let moved = tauri::async_runtime::block_on(commands::move_tabs(
        window(&t.app, "main-1"),
        sessions(&t.app),
        MoveWhat::Tabs(vec![b]),
        MoveTo::NewWindow {
            label: None,
            geometry: None,
        },
    ));
    assert!(matches!(
        moved,
        Err(Error::Window(WindowError::NotAvailable))
    ));
    let opened = tauri::async_runtime::block_on(commands::open_window(
        window(&t.app, "main-1"),
        sessions(&t.app),
        Some(loc("c")),
        None,
    ));
    assert!(opened.is_err());

    assert_eq!(sessions(&t.app).with_store(|s| s.clone()), before);
    assert!(
        drain(&source).is_empty(),
        "nothing is announced for a change that was undone"
    );

    // The failed attempts left no trace: the next window gets the label they would have had.
    *t.factory.fail.lock().unwrap() = None;
    let label = tauri::async_runtime::block_on(commands::open_window(
        window(&t.app, "main-1"),
        sessions(&t.app),
        None,
        None,
    ))
    .unwrap();
    assert_eq!(
        label, "main-2",
        "the failed attempts did not use up a label"
    );
}

#[test]
fn a_caller_cannot_choose_a_window_label() {
    let t = setup(&["main-1"]);
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    let moved = tauri::async_runtime::block_on(commands::move_tabs(
        window(&t.app, "main-1"),
        sessions(&t.app),
        MoveWhat::Tabs(vec![a]),
        MoveTo::NewWindow {
            label: Some("main-5".into()),
            geometry: None,
        },
    ));
    assert!(moved.is_err());
    assert!(t.factory.created.lock().unwrap().is_empty());
}

#[test]
fn closing_tabs_and_windows_fires_the_close_hooks() {
    let t = setup(&["main-1"]);
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = heard.clone();
    sessions(&t.app).on_tab_closed(move |w, tab| sink.lock().unwrap().push((w.to_string(), tab)));
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    let b = open_tab(&t.app, "main-1", "b").unwrap();
    tauri::async_runtime::block_on(commands::close_tab(
        window(&t.app, "main-1"),
        sessions(&t.app),
        a,
    ))
    .unwrap();
    // The window going away takes the rest with it.
    destroyed(&t.app, "main-1");
    wait_until("the window's tabs are reported closed", || {
        heard.lock().unwrap().len() == 2
    });
    assert_eq!(
        *heard.lock().unwrap(),
        vec![("main-1".to_string(), a), ("main-1".to_string(), b)]
    );
    assert!(sessions(&t.app).with_store(|s| s.window("main-1").is_none()));
}

#[test]
fn the_last_window_closing_runs_its_hook_once() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let t = setup_with(
        StorePolicy::default(),
        move |deps| {
            deps.on_last_window_closed = Some(Arc::new(move |_app| {
                counter.fetch_add(1, Ordering::SeqCst);
            }));
        },
        &["main-1"],
    );
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    let label = tauri::async_runtime::block_on(commands::open_window(
        window(&t.app, "main-1"),
        sessions(&t.app),
        Some(loc("b")),
        None,
    ))
    .unwrap();
    assert_eq!(label, "main-2");

    // Closing the first window's last tab closes the window under the app policy, not the app.
    tauri::async_runtime::block_on(commands::close_tab(
        window(&t.app, "main-1"),
        sessions(&t.app),
        a,
    ))
    .unwrap();
    assert!(sessions(&t.app).with_store(|s| s.window("main-1").is_none()));
    assert_eq!(hits.load(Ordering::SeqCst), 0);

    // Closing the last window runs the hook (the webview is destroyed too, which the mock
    // runtime does not show).
    tauri::async_runtime::block_on(commands::close_window(
        window(&t.app, "main-2"),
        sessions(&t.app),
        None,
    ))
    .unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // The Destroyed cleanup that follows finds nothing left to close.
    destroyed(&t.app, "main-2");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[test]
fn a_destroyed_last_window_runs_the_hook() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let t = setup_with(
        m2_policy(),
        move |deps| {
            deps.on_last_window_closed = Some(Arc::new(move |_app| {
                counter.fetch_add(1, Ordering::SeqCst);
            }));
        },
        &["main-1"],
    );
    open_tab(&t.app, "main-1", "a").unwrap();
    destroyed(&t.app, "main-1");
    wait_until("the hook runs", || hits.load(Ordering::SeqCst) == 1);
}

#[test]
fn changes_are_reported_once_per_burst() {
    let calls = Arc::new(AtomicUsize::new(0));
    let tabs_seen = Arc::new(Mutex::new(0));
    let (counter, seen) = (calls.clone(), tabs_seen.clone());
    let t = setup_with(
        m2_policy(),
        move |deps| {
            deps.change_delay = Duration::from_millis(300);
            deps.on_change = Some(Arc::new(move |store| {
                counter.fetch_add(1, Ordering::SeqCst);
                *seen.lock().unwrap() = store.windows().iter().map(|w| w.tabs.len()).sum();
            }));
        },
        &["main-1"],
    );
    for name in ["a", "b", "c", "d"] {
        open_tab(&t.app, "main-1", name).unwrap();
    }
    wait_until("the first report", || calls.load(Ordering::SeqCst) == 1);
    assert_eq!(
        *tabs_seen.lock().unwrap(),
        4,
        "the report sees the whole burst"
    );
    open_tab(&t.app, "main-1", "e").unwrap();
    wait_until("the second report", || calls.load(Ordering::SeqCst) == 2);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn closing_the_last_tab_of_the_last_window_runs_the_hook() {
    // The mock runtime does not show the webview being destroyed, so this checks the store and
    // the hook, which run after the destroy call in `after_unlock`.
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let t = setup_with(
        StorePolicy::default(),
        move |deps| {
            deps.on_last_window_closed = Some(Arc::new(move |_app| {
                counter.fetch_add(1, Ordering::SeqCst);
            }));
        },
        &["main-1"],
    );
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    tauri::async_runtime::block_on(commands::close_tab(
        window(&t.app, "main-1"),
        sessions(&t.app),
        a,
    ))
    .unwrap();
    assert!(sessions(&t.app).with_store(|s| s.windows().is_empty()));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[test]
fn a_geometry_change_is_reported_though_it_makes_no_event() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let t = setup_with(
        m2_policy(),
        move |deps| {
            deps.change_delay = Duration::from_millis(50);
            deps.on_change = Some(Arc::new(move |_| {
                counter.fetch_add(1, Ordering::SeqCst);
            }));
        },
        &["main-1"],
    );
    open_tab(&t.app, "main-1", "a").unwrap();
    wait_until("the tab's report", || calls.load(Ordering::SeqCst) == 1);
    let geometry = Geometry {
        x: None,
        y: None,
        width: 900,
        height: 600,
        maximised: false,
    };
    tauri::async_runtime::block_on(commands::set_geometry(
        window(&t.app, "main-1"),
        sessions(&t.app),
        geometry,
    ))
    .unwrap();
    wait_until("the geometry's report", || {
        calls.load(Ordering::SeqCst) == 2
    });
    assert_eq!(
        sessions(&t.app).with_store(|s| s.window("main-1").and_then(|w| w.geometry)),
        Some(geometry)
    );
}

#[test]
fn the_new_commands_run_through_the_store() {
    let t = setup(&["main-1"]);
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    let b = open_tab(&t.app, "main-1", "b").unwrap();
    let c = open_tab(&t.app, "main-1", "c").unwrap();
    let w = || window(&t.app, "main-1");

    let group = tauri::async_runtime::block_on(commands::create_group(
        w(),
        sessions(&t.app),
        vec![a, b],
        Some("Work".into()),
    ))
    .unwrap();
    let pair = tauri::async_runtime::block_on(commands::join_pair(
        w(),
        sessions(&t.app),
        vec![b, c],
        PairLayout::Stacked,
    ));
    // `c` is outside the group, so joining takes it in: the pair stays in one group.
    let pair = pair.unwrap();
    tauri::async_runtime::block_on(commands::pin_tab(w(), sessions(&t.app), a, true)).unwrap();
    tauri::async_runtime::block_on(commands::close_tab(w(), sessions(&t.app), c)).unwrap();
    let reopened =
        tauri::async_runtime::block_on(commands::reopen_tab(w(), sessions(&t.app), None)).unwrap();
    assert_eq!(reopened, Some(c));
    let snapshot =
        tauri::async_runtime::block_on(commands::get_snapshot(w(), sessions(&t.app))).unwrap();
    assert_eq!(snapshot.groups.len(), 1);
    assert_eq!(snapshot.groups[0].id, group);
    assert!(
        snapshot.tabs.iter().all(|t| t.pinned),
        "pinning carries the group"
    );
    assert!(snapshot.pairs.iter().all(|p| p.id == pair) || snapshot.pairs.is_empty());
    assert!(sessions(&t.app).with_store(|s| s.violations().is_empty()));
}

#[test]
fn workspaces_are_saved_switched_and_announced_to_every_window() {
    let t = setup(&["main-1", "main-2"]);
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    open_tab(&t.app, "main-2", "other").unwrap();
    let first = events(&t.app, "main-1");
    let second = events(&t.app, "main-2");
    let w = || window(&t.app, "main-1");
    let group = tauri::async_runtime::block_on(commands::create_group(
        w(),
        sessions(&t.app),
        vec![a],
        Some("Site".into()),
    ))
    .unwrap();
    drain(&first);
    drain(&second);

    let id = tauri::async_runtime::block_on(commands::save_group_as_workspace(
        w(),
        sessions(&t.app),
        group,
        None,
    ))
    .unwrap();
    // Workspaces are global: both windows hear the list.
    for receiver in [&first, &second] {
        assert!(drain(receiver)
            .iter()
            .any(|e| matches!(e, SessionEvent::WorkspacesChanged { workspaces, .. } if workspaces[0].name == "Site")));
    }
    let again = tauri::async_runtime::block_on(commands::save_group_as_workspace(
        w(),
        sessions(&t.app),
        group,
        Some("site".into()),
    ));
    let message = again.unwrap_err().to_string();
    assert!(message.starts_with("a workspace named"), "{message}");

    // Switching is per window.
    tauri::async_runtime::block_on(commands::set_active_workspace(
        w(),
        sessions(&t.app),
        Some(id),
    ))
    .unwrap();
    assert!(drain(&first).iter().any(|e| matches!(
        e,
        SessionEvent::WorkspaceActivated {
            workspace: Some(_),
            ..
        }
    )));
    assert!(drain(&second).is_empty());
    tauri::async_runtime::block_on(commands::delete_workspace(w(), sessions(&t.app), id)).unwrap();
    assert!(drain(&first).iter().any(|e| matches!(
        e,
        SessionEvent::WorkspaceActivated {
            workspace: None,
            ..
        }
    )));
    assert!(sessions(&t.app).with_store(|s| s.violations().is_empty()));
}

#[test]
fn the_shelf_is_shared_by_every_window_and_a_full_one_is_a_typed_refusal() {
    let t = setup(&["main-1", "main-2"]);
    open_tab(&t.app, "main-1", "a").unwrap();
    open_tab(&t.app, "main-2", "b").unwrap();
    let first = events(&t.app, "main-1");
    let second = events(&t.app, "main-2");
    drain(&first);
    drain(&second);
    let w = |label: &str| window(&t.app, label);
    let place = |name: &str| Location::new(format!("/d/{name}"), format!("file:///d/{name}"));

    tauri::async_runtime::block_on(commands::add_to_shelf(
        w("main-1"),
        sessions(&t.app),
        vec![place("x"), place("y")],
    ))
    .unwrap();
    for receiver in [&first, &second] {
        assert!(drain(receiver).iter().any(
            |e| matches!(e, SessionEvent::ShelfChanged { shelf, .. } if shelf.len() == 2
                && shelf[0].name == "x" && shelf[0].origin.display == "/d")
        ));
    }
    let id = sessions(&t.app).with_store(|s| s.shelf()[0].id);
    // Another window takes the first item off.
    tauri::async_runtime::block_on(commands::remove_from_shelf(
        w("main-2"),
        sessions(&t.app),
        vec![id],
    ))
    .unwrap();
    for receiver in [&first, &second] {
        assert!(drain(receiver)
            .iter()
            .any(|e| matches!(e, SessionEvent::ShelfChanged { shelf, .. } if shelf.len() == 1)));
    }
    let second_id = sessions(&t.app).with_store(|s| s.shelf()[0].id);
    tauri::async_runtime::block_on(commands::move_shelf_item(
        w("main-1"),
        sessions(&t.app),
        second_id,
        0,
    ))
    .unwrap();
    tauri::async_runtime::block_on(commands::clear_shelf(w("main-1"), sessions(&t.app))).unwrap();
    assert!(sessions(&t.app).with_store(|s| s.shelf().is_empty()));

    // Past the limit: nothing is added and the page can tell the refusal by its kind.
    let many: Vec<Location> = (0..=waypoint_session::SHELF_LIMIT)
        .map(|i| place(&format!("f{i}")))
        .collect();
    let refused =
        tauri::async_runtime::block_on(commands::add_to_shelf(w("main-1"), sessions(&t.app), many));
    let json = serde_json::to_value(refused.unwrap_err()).unwrap();
    assert_eq!(json["kind"], "shelfFull");
    assert_eq!(json["limit"], waypoint_session::SHELF_LIMIT);
    assert!(sessions(&t.app).with_store(|s| s.shelf().is_empty()));
    assert!(sessions(&t.app).with_store(|s| s.violations().is_empty()));
}

#[test]
fn only_a_main_window_can_change_the_shelf() {
    let t = setup(&["main-1", "settings"]);
    open_tab(&t.app, "main-1", "a").unwrap();
    let refused = tauri::async_runtime::block_on(commands::add_to_shelf(
        window(&t.app, "settings"),
        sessions(&t.app),
        vec![loc("x")],
    ));
    assert!(
        refused.is_err(),
        "a settings window has no session to change"
    );
    assert!(sessions(&t.app).with_store(|s| s.shelf().is_empty()));
    tauri::async_runtime::block_on(commands::add_to_shelf(
        window(&t.app, "main-1"),
        sessions(&t.app),
        vec![loc("x")],
    ))
    .unwrap();
    assert_eq!(sessions(&t.app).with_store(|s| s.shelf().len()), 1);
}

#[test]
fn closing_a_window_by_name_removes_its_session_and_unknown_windows_are_refused() {
    let t = setup(&["main-1", "main-2"]);
    open_tab(&t.app, "main-1", "a").unwrap();
    open_tab(&t.app, "main-2", "b").unwrap();
    tauri::async_runtime::block_on(commands::close_window(
        window(&t.app, "main-1"),
        sessions(&t.app),
        Some("main-2".into()),
    ))
    .unwrap();
    assert!(sessions(&t.app).with_store(|s| s.window("main-2").is_none() && !s.closed().is_empty()));
    let again = tauri::async_runtime::block_on(commands::close_window(
        window(&t.app, "main-1"),
        sessions(&t.app),
        Some("main-2".into()),
    ));
    assert!(again.is_err());
}

#[test]
fn run_existing_does_not_register_a_window_the_store_closed() {
    let t = setup(&["main-1"]);
    open_tab(&t.app, "main-1", "a").unwrap();
    let geometry = Geometry {
        x: None,
        y: None,
        width: 900,
        height: 600,
        maximised: false,
    };
    // The webview still exists (it is being destroyed) but the store already forgot the window.
    destroyed(&t.app, "main-1");
    wait_until("the store forgets the window", || {
        sessions(&t.app).with_store(|s| s.window("main-1").is_none())
    });

    let refused =
        sessions(&t.app).run_existing(t.app.handle(), "main-1", Command::SetGeometry { geometry });
    assert!(refused.is_err());
    assert!(sessions(&t.app).with_store(|s| s.windows().is_empty()));

    // A window the store holds still takes the command.
    let t = setup(&["main-1"]);
    tauri::async_runtime::block_on(commands::get_snapshot(
        window(&t.app, "main-1"),
        sessions(&t.app),
    ))
    .unwrap();
    sessions(&t.app)
        .run_existing(t.app.handle(), "main-1", Command::SetGeometry { geometry })
        .unwrap();
    assert_eq!(
        sessions(&t.app).with_store(|s| s.window("main-1").and_then(|w| w.geometry)),
        Some(geometry)
    );
}

fn open_window(app: &App, from: &str) -> Result<String, Error> {
    tauri::async_runtime::block_on(commands::open_window(
        window(app, from),
        sessions(app),
        Some(loc("w")),
        None,
    ))
}

#[test]
fn moving_the_only_tab_of_a_window_to_a_new_window_is_allowed_at_the_cap() {
    // With the app's policy the emptied window closes, so the count does not grow.
    let t = setup_with(StorePolicy::default(), |_| {}, &["main-1"]);
    let only = open_tab(&t.app, "main-1", "a").unwrap();
    for _ in 1..crate::MAX_WINDOWS {
        open_window(&t.app, "main-1").unwrap();
    }
    assert_eq!(sessions(&t.app).with_store(|s| s.windows().len()), 12);
    let moved = tauri::async_runtime::block_on(commands::move_tabs(
        window(&t.app, "main-1"),
        sessions(&t.app),
        MoveWhat::Tabs(vec![only]),
        MoveTo::NewWindow {
            label: None,
            geometry: None,
        },
    ));
    assert!(moved.is_ok(), "{moved:?}");
    assert_eq!(sessions(&t.app).with_store(|s| s.windows().len()), 12);
    assert!(sessions(&t.app).with_store(|s| s.window("main-1").is_none()));

    // A window that keeps a tab would make a thirteenth, so that is still refused.
    let extra = open_tab(&t.app, "main-2", "y").unwrap();
    let refused = tauri::async_runtime::block_on(commands::move_tabs(
        window(&t.app, "main-2"),
        sessions(&t.app),
        MoveWhat::Tabs(vec![extra]),
        MoveTo::NewWindow {
            label: None,
            geometry: None,
        },
    ));
    assert!(matches!(refused, Err(Error::TooManyWindows { .. })));
}

#[test]
fn the_thirteenth_window_is_refused_with_a_typed_error_and_nothing_changes() {
    let t = setup(&["main-1"]);
    open_tab(&t.app, "main-1", "a").unwrap();
    for _ in 1..crate::MAX_WINDOWS {
        open_window(&t.app, "main-1").unwrap();
    }
    assert_eq!(sessions(&t.app).with_store(|s| s.windows().len()), 12);
    let before = sessions(&t.app).with_store(|s| s.clone());
    let created = t.factory.created.lock().unwrap().len();

    let refused = open_window(&t.app, "main-1");
    assert!(matches!(refused, Err(Error::TooManyWindows { limit: 12 })));
    assert_eq!(sessions(&t.app).with_store(|s| s.clone()), before);
    assert_eq!(t.factory.created.lock().unwrap().len(), created);

    // Moving a tab to yet another new window is refused the same way, and so is nothing else.
    let b = open_tab(&t.app, "main-1", "b").unwrap();
    let moved = tauri::async_runtime::block_on(commands::move_tabs(
        window(&t.app, "main-1"),
        sessions(&t.app),
        MoveWhat::Tabs(vec![b]),
        MoveTo::NewWindow {
            label: None,
            geometry: None,
        },
    ));
    assert!(matches!(moved, Err(Error::TooManyWindows { .. })));
    assert_eq!(tab_count(&t.app, "main-1"), 2);

    // The refusal reaches the page as an object it can tell apart from a failure.
    let json = serde_json::to_value(refused.unwrap_err()).unwrap();
    assert_eq!(json["kind"], "tooManyWindows");
    assert_eq!(json["limit"], 12);

    // Closing a window makes room again, and moving into an existing window is never refused.
    tauri::async_runtime::block_on(commands::close_window(
        window(&t.app, "main-1"),
        sessions(&t.app),
        Some("main-12".into()),
    ))
    .unwrap();
    assert!(open_window(&t.app, "main-1").is_ok());
    const { assert!(WARN_WINDOWS < MAX_WINDOWS) };
}

#[test]
fn list_windows_names_each_window_by_its_active_tab_and_marks_the_caller() {
    let t = setup(&["main-1", "main-2"]);
    open_tab(&t.app, "main-1", "Documents").unwrap();
    open_tab(&t.app, "main-1", "Music").unwrap();
    open_tab(&t.app, "main-2", "Pictures").unwrap();
    let listed = tauri::async_runtime::block_on(commands::list_windows(
        window(&t.app, "main-2"),
        sessions(&t.app),
    ))
    .unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].label, "main-1");
    // `open_tab` activates the new tab, so the window shows its newest.
    assert_eq!(listed[0].title, "Music");
    assert_eq!(listed[0].tab_count, 2);
    assert!(!listed[0].active);
    assert_eq!(listed[1].title, "Pictures");
    assert!(listed[1].active);
}

#[test]
fn an_existing_target_window_is_told_about_the_hand_off() {
    let t = setup(&["main-1", "main-2"]);
    let a = open_tab(&t.app, "main-1", "a").unwrap();
    open_tab(&t.app, "main-1", "b").unwrap();
    open_tab(&t.app, "main-2", "c").unwrap();
    let (sender, receiver) = channel::<serde_json::Value>();
    window(&t.app, "main-2").listen(crate::HANDOFF_EVENT, move |event| {
        let _ = sender.send(serde_json::from_str(event.payload()).unwrap());
    });
    let label = tauri::async_runtime::block_on(commands::move_tabs(
        window(&t.app, "main-1"),
        sessions(&t.app),
        MoveWhat::Tabs(vec![a]),
        MoveTo::ExistingWindow {
            label: "main-2".into(),
            index: 0,
        },
    ))
    .unwrap();
    assert_eq!(label, "main-2");
    wait_until("the hand-off event", || {
        receiver.try_recv().is_ok_and(|p| {
            assert_eq!(p["from"], "main-1");
            assert_eq!(p["tabs"], serde_json::json!([a.0]));
            true
        })
    });
}

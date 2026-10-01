// Headless tests for the persisted document: round trips, history capping, repair and rejection.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Mutex;

use waypoint_session::{
    Command, Document, DocumentError, GroupId, MoveTo, MoveWhat, PairLayout, SessionStorage,
    StorageError, Store, TabColour, TabHints, TabId, WorkspaceId, DOCUMENT_VERSION,
};

mod common;
use common::{assert_ok, loc, run, store_with};

const W: &str = "main-1";

fn busy_store() -> Store {
    let mut s = store_with(&["a", "b", "c", "d"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), TabId(2)],
            name: Some("G".into()),
        },
    );
    run(
        &mut s,
        W,
        Command::JoinPair {
            tabs: vec![TabId(3), TabId(4)],
            layout: PairLayout::Stacked,
        },
    );
    run(
        &mut s,
        W,
        Command::SetColour {
            tab: TabId(1),
            colour: Some(TabColour::Orange),
        },
    );
    run(
        &mut s,
        W,
        Command::SetHints {
            tab: TabId(1),
            hints: TabHints {
                scroll_top: 9,
                focused: Some("x".into()),
            },
        },
    );
    run(
        &mut s,
        W,
        Command::Pin {
            tab: TabId(1),
            pinned: true,
        },
    );
    run(&mut s, W, Command::Activate { tab: TabId(4) });
    let pair = s.window(W).unwrap().pairs[0].id;
    run(
        &mut s,
        W,
        Command::MoveTabs {
            what: MoveWhat::Pair(pair),
            to: MoveTo::NewWindow {
                label: None,
                geometry: None,
            },
        },
    );
    run(&mut s, "main-2", Command::Close { tab: TabId(3) });
    s
}

#[test]
fn a_store_round_trips_through_json_unchanged() {
    let store = busy_store();
    assert_ok(&store);
    let json = serde_json::to_string(&store.to_document()).unwrap();
    let doc: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(doc.version, DOCUMENT_VERSION);
    let (restored, notes) = Store::from_document(doc).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(restored, store);
}

#[test]
fn ids_are_never_reused_after_a_restore() {
    let store = busy_store();
    let (mut restored, _) = Store::from_document(store.to_document()).unwrap();
    let out = run(
        &mut restored,
        W,
        Command::Open {
            location: loc("n"),
            after: None,
            activate: false,
        },
    );
    let used: Vec<u32> = store
        .windows()
        .iter()
        .flat_map(|w| w.tabs.iter().map(|t| t.id.0))
        .chain(store.closed().iter().map(|c| c.tab.id.0))
        .collect();
    let new = out.events.iter().find_map(|e| match &e.event {
        waypoint_session::SessionEvent::TabOpened { tab, .. } => Some(tab.id.0),
        _ => None,
    });
    assert!(!used.contains(&new.unwrap()));
    // The next window label does not collide either.
    let out = run(
        &mut restored,
        "",
        Command::OpenWindow {
            location: None,
            geometry: None,
        },
    );
    assert_eq!(out.windows_opened(), vec!["main-3".to_string()]);
}

#[test]
fn saving_caps_history_at_one_hundred_per_tab() {
    let mut s = store_with(&["a"]);
    for n in 0..150 {
        s.dispatch(
            W,
            Command::Navigate {
                tab: TabId(1),
                location: loc(&format!("n{n}")),
            },
        )
        .unwrap();
    }
    for _ in 0..120 {
        s.dispatch(W, Command::Back { tab: TabId(1) }).unwrap();
    }
    let doc = s.to_document();
    let tab = &doc.body.windows[0].tabs[0];
    assert_eq!(tab.back.len(), 30);
    assert_eq!(tab.forward.len(), 100);
    // The nearest entries survive: going forward from the saved tab reaches the next location.
    assert_eq!(tab.forward.last().unwrap(), &loc("n30"));
    assert_eq!(tab.back.last().unwrap(), &loc("n28"));

    let mut s = store_with(&["a"]);
    for n in 0..150 {
        s.dispatch(
            W,
            Command::Navigate {
                tab: TabId(1),
                location: loc(&format!("n{n}")),
            },
        )
        .unwrap();
    }
    let doc = s.to_document();
    let tab = &doc.body.windows[0].tabs[0];
    assert_eq!(tab.back.len(), 100);
    // Oldest entries go; the latest are kept in order.
    assert_eq!(tab.back.last().unwrap(), &loc("n148"));
    // The live store keeps everything.
    assert_eq!(s.window(W).unwrap().tabs[0].back.len(), 150);
}

#[test]
fn another_version_is_rejected() {
    let mut doc = busy_store().to_document();
    doc.version = 99;
    assert_eq!(
        Store::from_document(doc),
        Err(DocumentError::UnsupportedVersion {
            found: 99,
            supported: DOCUMENT_VERSION
        })
    );
}

#[test]
fn text_that_is_not_a_document_does_not_parse() {
    assert!(serde_json::from_str::<Document>("{ not json").is_err());
    assert!(serde_json::from_str::<Document>(r#"{"version":1}"#).is_err());
    assert!(serde_json::from_str::<Document>(r#"{"version":1,"body":{"windows":"x"}}"#).is_err());
}

#[test]
fn a_document_that_breaks_invariants_is_repaired_and_reports_it() {
    let store = busy_store();
    let mut doc = store.to_document();
    {
        let w = &mut doc.body.windows[0];
        // A dangling group, a repeated id, a bad active tab, a stale mru entry and a bad pair.
        w.tabs[0].group = Some(GroupId(77));
        let dup = w.tabs[0].clone();
        w.tabs.push(dup);
        w.active = Some(TabId(500));
        w.mru = vec![TabId(500), TabId(1), TabId(1)];
        w.groups.push(waypoint_session::Group {
            id: GroupId(88),
            name: "empty".into(),
            colour: None,
            collapsed: false,
        });
    }
    // A second window repeats a tab of the first, and a closed tab is also open.
    let mut other = doc.body.windows[1].clone();
    other.label = "main-9".into();
    doc.body.windows.push(other);
    doc.body.closed.push(waypoint_session::ClosedTab {
        tab: doc.body.windows[0].tabs[1].clone(),
        window: W.into(),
        index: 0,
    });
    doc.body.next_tab = 1;
    let (restored, notes) = Store::from_document(doc).unwrap();
    assert_ok(&restored);
    assert!(!notes.is_empty());
    assert!(
        restored.window("main-9").is_none(),
        "a window of repeated tabs is empty and dropped"
    );
    let w = restored.window(W).unwrap();
    assert!(w.groups.iter().all(|g| g.id != GroupId(88)));
    assert!(w.active.is_some());
    // Counters moved above everything in use.
    let mut restored = restored;
    run(
        &mut restored,
        W,
        Command::Open {
            location: loc("z"),
            after: None,
            activate: false,
        },
    );
    assert_ok(&restored);
}

#[test]
fn a_window_that_is_not_a_main_window_rejects_the_document() {
    let mut doc = busy_store().to_document();
    doc.body.windows[0].label = "settings".into();
    assert!(matches!(
        Store::from_document(doc),
        Err(DocumentError::Corrupt(_))
    ));
}

#[test]
fn empty_windows_are_dropped_and_an_empty_document_is_an_empty_store() {
    let mut doc = busy_store().to_document();
    doc.body.windows.iter_mut().for_each(|w| w.tabs.clear());
    let (restored, notes) = Store::from_document(doc).unwrap();
    assert!(restored.windows().is_empty());
    assert!(!notes.is_empty());
}

struct Memory(Mutex<Option<Document>>);

impl SessionStorage for Memory {
    fn load(&self) -> Result<Option<Document>, StorageError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, document: &Document) -> Result<(), StorageError> {
        *self.0.lock().unwrap() = Some(document.clone());
        Ok(())
    }
}

#[test]
fn the_storage_trait_is_enough_to_save_and_restore() {
    let storage: Box<dyn SessionStorage> = Box::new(Memory(Mutex::new(None)));
    assert_eq!(storage.load().unwrap(), None);
    let store = busy_store();
    storage.save(&store.to_document()).unwrap();
    let (restored, _) = Store::from_document(storage.load().unwrap().unwrap()).unwrap();
    assert_eq!(restored, store);
}

#[test]
fn workspaces_and_the_active_one_survive_a_round_trip() {
    let mut s = store_with(&["a", "b"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1), TabId(2)],
            name: Some("Site".into()),
        },
    );
    run(
        &mut s,
        W,
        Command::SaveGroupAsWorkspace {
            group: GroupId(1),
            name: None,
        },
    );
    run(
        &mut s,
        W,
        Command::SetActiveWorkspace {
            workspace: Some(WorkspaceId(1)),
        },
    );
    let json = serde_json::to_string(&s.to_document()).unwrap();
    let doc: Document = serde_json::from_str(&json).unwrap();
    let (mut restored, notes) = Store::from_document(doc).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(restored, s);
    assert_eq!(restored.window(W).unwrap().workspace, Some(WorkspaceId(1)));
    // The counter is restored, so a new workspace does not reuse the id.
    run(
        &mut restored,
        W,
        Command::SaveGroupAsWorkspace {
            group: GroupId(1),
            name: Some("Other".into()),
        },
    );
    assert_eq!(restored.workspaces()[1].id, WorkspaceId(2));
}

#[test]
fn a_document_written_before_workspaces_still_loads() {
    let store = busy_store();
    let mut json: serde_json::Value = serde_json::to_value(store.to_document()).unwrap();
    let body = json["body"].as_object_mut().unwrap();
    body.remove("workspaces");
    body.remove("nextWorkspace");
    for w in body["windows"].as_array_mut().unwrap() {
        w.as_object_mut().unwrap().remove("workspace");
    }
    assert_eq!(json["version"], DOCUMENT_VERSION);
    let doc: Document = serde_json::from_value(json).unwrap();
    let (restored, notes) = Store::from_document(doc).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(restored, store);
    assert!(restored.workspaces().is_empty());
}

#[test]
fn a_repeated_workspace_name_or_a_missing_active_workspace_is_repaired() {
    let mut s = store_with(&["a"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1)],
            name: Some("G".into()),
        },
    );
    run(
        &mut s,
        W,
        Command::SaveGroupAsWorkspace {
            group: GroupId(1),
            name: None,
        },
    );
    let mut doc = s.to_document();
    let mut twin = doc.body.workspaces[0].clone();
    twin.id = WorkspaceId(7);
    twin.name = " g ".into();
    doc.body.workspaces.push(twin);
    doc.body.windows[0].workspace = Some(WorkspaceId(42));
    let (restored, notes) = Store::from_document(doc).unwrap();
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert_eq!(restored.workspaces().len(), 1);
    assert_eq!(restored.window(W).unwrap().workspace, None);
    assert_ok(&restored);
}

#[test]
fn deleting_the_active_workspace_sends_every_window_back_to_the_bookmarks() {
    let mut s = store_with(&["a"]);
    run(
        &mut s,
        W,
        Command::CreateGroup {
            tabs: vec![TabId(1)],
            name: Some("G".into()),
        },
    );
    run(
        &mut s,
        W,
        Command::SaveGroupAsWorkspace {
            group: GroupId(1),
            name: None,
        },
    );
    run(
        &mut s,
        "",
        Command::OpenWindow {
            location: Some(loc("w")),
            geometry: None,
        },
    );
    for label in [W, "main-2"] {
        run(
            &mut s,
            label,
            Command::SetActiveWorkspace {
                workspace: Some(WorkspaceId(1)),
            },
        );
    }
    let out = run(
        &mut s,
        W,
        Command::DeleteWorkspace {
            workspace: WorkspaceId(1),
        },
    );
    for label in [W, "main-2"] {
        assert!(out.events_for(label).any(|e| matches!(
            e,
            waypoint_session::SessionEvent::WorkspaceActivated {
                workspace: None,
                ..
            }
        )));
        assert_eq!(s.window(label).unwrap().workspace, None);
    }
    assert_ok(&s);
}

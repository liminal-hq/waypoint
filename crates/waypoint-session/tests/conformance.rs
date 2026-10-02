// Runs the scripted conformance scenarios in `tests/conformance/*.json` against the store.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Scenario format (the TypeScript `FakeTabsApi` test runs the same files):
//
//   { "description": "...", "steps": [ { "window"?: "main-1", "do": {...}, "expect": {...}, "error"?: true, "note"?: "..." } ] }
//
// `do` is one command, named by `op`, on the step's `window` (default `main-1`, the one window the
// run starts with). Tabs, groups and pairs are named by their numeric ids, which both
// implementations allocate from 1 in the order they are made (a window label is `main-{n}`), and
// locations by their path, so `"/a"` is `Location { display: "/a", uri: "file:///a" }`. Operations:
//
//   open      { location, after?: id, activate?: bool (default true) }
//   close | activate | back | forward | toggleSplit | removeFromGroup   { tab }
//   move      { tab, index }
//   navigate  { tab, location }
//   pin       { tab, pinned }
//   colour    { tab, colour: "red" | ... | null }
//   reopen    { tab?: id }
//   createGroup { tabs: [id], name?: string }      addToGroup { tab, group }
//   renameGroup { group, name }      collapseGroup { group, collapsed }      collapseOthers { group }
//   sortGroup { group, by: "name" | "location" | "localFirst" }      moveGroup { group, index }
//   duplicateGroup | ungroup | closeGroup   { group }
//   joinPair  { tabs: [id], layout?: "sideBySide" | "stacked" }      separatePair | swapPanes { pair }
//   setPairSizes { pair, sizes: [n] }
//   openWindow { location? }      closeWindow
//   saveGroupAsWorkspace { group, name?: string }      renameWorkspace { workspace, name }
//   deleteWorkspace { workspace }      setActiveWorkspace { workspace: id | null }
//   setWorkspaceLocations { workspace, locations: [path] }
//   addToShelf { locations: [path] }      removeFromShelf { ids: [n] }      clearShelf
//   moveShelfItem { id, index }
//   moveTabs  { what: { kind: "tabs" | "group" | "pair", value }, to: { kind: "existingWindow", label, index }
//                                                                  | { kind: "newWindow" } }
//
// `expect` lists only the facts a step checks; each is compared after the step. The window facts
// are about the step's window, which must still exist when any is given:
//
//   tabs       ids in display order
//   active     the active tab id
//   locations  the current location of each tab, in display order
//   pinned     ids of the pinned tabs, in display order
//   history    { "<id>": { back: [...], forward: [...] } } as paths
//   mru        ids, most recent first
//   groups     [ { id, name, collapsed, tabs: [id] } ] in the window's group order
//   pairs      [ { id, panes: [id], layout, sizes: [n] } ] in the window's pair order
//   workspace  the id of the window's active workspace, or null
//
// and these are about the store and other windows:
//
//   closed     ids of the recently closed tabs, newest first
//   windows    the window labels, in store order
//   workspaces [ { id, name, locations: [path] } ] in creation order (they are global)
//   shelf      [ { id, name, location: path, origin: path } ] in the Shelf's order (it is global)
//   others     { "<label>": { ...the window facts above... } }
//
// A step with `"error": true` must fail and leave the state as it was. Revisions and events are not
// part of the format, because the two implementations number them differently.

use serde_json::{json, Value};
use waypoint_session::{
    Command, GroupId, GroupSort, MoveTo, MoveWhat, PairId, PairLayout, ShelfItemId, Store,
    TabColour, TabId, WindowState, WorkspaceId,
};

mod common;
use common::{assert_ok, loc};

const W: &str = "main-1";

fn id(v: &Value, key: &str) -> TabId {
    TabId(v[key].as_u64().expect(key) as u32)
}

fn tabs_of(op: &Value, key: &str) -> Vec<TabId> {
    op[key]
        .as_array()
        .expect(key)
        .iter()
        .map(|v| TabId(v.as_u64().expect(key) as u32))
        .collect()
}

fn group(op: &Value) -> GroupId {
    GroupId(op["group"].as_u64().expect("group") as u32)
}

fn workspace(op: &Value) -> WorkspaceId {
    WorkspaceId(op["workspace"].as_u64().expect("workspace") as u32)
}

fn pair(op: &Value) -> PairId {
    PairId(op["pair"].as_u64().expect("pair") as u32)
}

fn wire<T: serde::de::DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).expect("a wire value")
}

fn command(op: &Value) -> Command {
    let path = |key: &str| loc(op[key].as_str().expect(key).trim_start_matches('/'));
    match op["op"].as_str().expect("op") {
        "open" => Command::Open {
            location: path("location"),
            after: op
                .get("after")
                .and_then(Value::as_u64)
                .map(|a| TabId(a as u32)),
            activate: op.get("activate").and_then(Value::as_bool).unwrap_or(true),
        },
        "close" => Command::Close { tab: id(op, "tab") },
        "activate" => Command::Activate { tab: id(op, "tab") },
        "back" => Command::Back { tab: id(op, "tab") },
        "forward" => Command::Forward { tab: id(op, "tab") },
        "move" => Command::Move {
            tab: id(op, "tab"),
            index: op["index"].as_u64().expect("index") as usize,
        },
        "navigate" => Command::Navigate {
            tab: id(op, "tab"),
            location: path("location"),
        },
        "pin" => Command::Pin {
            tab: id(op, "tab"),
            pinned: op["pinned"].as_bool().expect("pinned"),
        },
        "colour" => Command::SetColour {
            tab: id(op, "tab"),
            colour: wire::<Option<TabColour>>(&op["colour"]),
        },
        "reopen" => Command::Reopen {
            tab: op
                .get("tab")
                .and_then(Value::as_u64)
                .map(|t| TabId(t as u32)),
        },
        "createGroup" => Command::CreateGroup {
            tabs: tabs_of(op, "tabs"),
            name: op.get("name").and_then(Value::as_str).map(String::from),
        },
        "addToGroup" => Command::AddToGroup {
            tab: id(op, "tab"),
            group: group(op),
        },
        "removeFromGroup" => Command::RemoveFromGroup { tab: id(op, "tab") },
        "renameGroup" => Command::RenameGroup {
            group: group(op),
            name: op["name"].as_str().expect("name").to_string(),
        },
        "collapseGroup" => Command::SetGroupCollapsed {
            group: group(op),
            collapsed: op["collapsed"].as_bool().expect("collapsed"),
        },
        "collapseOthers" => Command::CollapseOthers { group: group(op) },
        "sortGroup" => Command::SortGroup {
            group: group(op),
            by: wire::<GroupSort>(&op["by"]),
        },
        "moveGroup" => Command::MoveGroup {
            group: group(op),
            index: op["index"].as_u64().expect("index") as usize,
        },
        "duplicateGroup" => Command::DuplicateGroup { group: group(op) },
        "ungroup" => Command::Ungroup { group: group(op) },
        "closeGroup" => Command::CloseGroup { group: group(op) },
        "joinPair" => Command::JoinPair {
            tabs: tabs_of(op, "tabs"),
            layout: op
                .get("layout")
                .map_or(PairLayout::SideBySide, wire::<PairLayout>),
        },
        "separatePair" => Command::SeparatePair { pair: pair(op) },
        "swapPanes" => Command::SwapPanes { pair: pair(op) },
        "setPairSizes" => Command::SetPairSizes {
            pair: pair(op),
            sizes: op["sizes"]
                .as_array()
                .expect("sizes")
                .iter()
                .map(|v| v.as_u64().expect("size") as u32)
                .collect(),
        },
        "toggleSplit" => Command::ToggleSplit { tab: id(op, "tab") },
        "openWindow" => Command::OpenWindow {
            location: op
                .get("location")
                .map(|l| loc(l.as_str().expect("location").trim_start_matches('/'))),
            geometry: None,
        },
        "saveGroupAsWorkspace" => Command::SaveGroupAsWorkspace {
            group: group(op),
            name: op.get("name").and_then(Value::as_str).map(String::from),
        },
        "renameWorkspace" => Command::RenameWorkspace {
            workspace: workspace(op),
            name: op["name"].as_str().expect("name").to_string(),
        },
        "deleteWorkspace" => Command::DeleteWorkspace {
            workspace: workspace(op),
        },
        "setActiveWorkspace" => Command::SetActiveWorkspace {
            workspace: op["workspace"].as_u64().map(|w| WorkspaceId(w as u32)),
        },
        "setWorkspaceLocations" => Command::SetWorkspaceLocations {
            workspace: workspace(op),
            locations: op["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .map(|l| loc(l.as_str().expect("location").trim_start_matches('/')))
                .collect(),
        },
        "addToShelf" => Command::AddToShelf {
            locations: op["locations"]
                .as_array()
                .expect("locations")
                .iter()
                .map(|l| loc(l.as_str().expect("location").trim_start_matches('/')))
                .collect(),
            added_ms: 0,
        },
        "removeFromShelf" => Command::RemoveFromShelf {
            ids: op["ids"]
                .as_array()
                .expect("ids")
                .iter()
                .map(|v| ShelfItemId(v.as_u64().expect("id")))
                .collect(),
        },
        "clearShelf" => Command::ClearShelf,
        "moveShelfItem" => Command::MoveShelfItem {
            id: ShelfItemId(op["id"].as_u64().expect("id")),
            to_index: op["index"].as_u64().expect("index") as usize,
        },
        "closeWindow" => Command::CloseWindow,
        "moveTabs" => Command::MoveTabs {
            what: wire::<MoveWhat>(&op["what"]),
            to: wire::<MoveTo>(&op["to"]),
        },
        other => panic!("unknown op {other}"),
    }
}

fn paths(locations: &[waypoint_protocol::Location]) -> Value {
    Value::Array(
        locations
            .iter()
            .map(|l| Value::from(l.display.clone()))
            .collect(),
    )
}

const WINDOW_FACTS: [&str; 9] = [
    "tabs",
    "active",
    "locations",
    "pinned",
    "history",
    "mru",
    "groups",
    "pairs",
    "workspace",
];

fn ids_value(ids: impl Iterator<Item = u32>) -> Value {
    Value::Array(ids.map(Value::from).collect())
}

fn check_window(w: &WindowState, expect: &Value, at: &str) {
    if let Some(want) = expect.get("tabs") {
        assert_eq!(
            &ids_value(w.tabs.iter().map(|t| t.id.0)),
            want,
            "{at}: tabs"
        );
    }
    if let Some(want) = expect.get("active") {
        assert_eq!(&Value::from(w.active.map(|t| t.0)), want, "{at}: active");
    }
    if let Some(want) = expect.get("locations") {
        let got: Vec<Value> = w
            .tabs
            .iter()
            .map(|t| Value::from(t.location.display.clone()))
            .collect();
        assert_eq!(&Value::Array(got), want, "{at}: locations");
    }
    if let Some(want) = expect.get("pinned") {
        let got = ids_value(w.tabs.iter().filter(|t| t.pinned).map(|t| t.id.0));
        assert_eq!(&got, want, "{at}: pinned");
    }
    if let Some(want) = expect.get("history").and_then(Value::as_object) {
        for (tab, h) in want {
            let t = w
                .tab(TabId(tab.parse().unwrap()))
                .expect("history of a live tab");
            assert_eq!(paths(&t.back), h["back"], "{at}: back of {tab}");
            assert_eq!(paths(&t.forward), h["forward"], "{at}: forward of {tab}");
        }
    }
    if let Some(want) = expect.get("mru") {
        assert_eq!(&ids_value(w.mru.iter().map(|t| t.0)), want, "{at}: mru");
    }
    if let Some(want) = expect.get("groups") {
        let got: Vec<Value> = w
            .groups
            .iter()
            .map(|g| {
                json!({
                    "id": g.id.0,
                    "name": g.name,
                    "collapsed": g.collapsed,
                    "tabs": ids_value(w.group_tabs(g.id).iter().map(|t| t.0)),
                })
            })
            .collect();
        assert_eq!(&Value::Array(got), want, "{at}: groups");
    }
    if let Some(want) = expect.get("workspace") {
        assert_eq!(
            &Value::from(w.workspace.map(|x| x.0)),
            want,
            "{at}: workspace"
        );
    }
    if let Some(want) = expect.get("pairs") {
        let got: Vec<Value> = w
            .pairs
            .iter()
            .map(|p| {
                json!({
                    "id": p.id.0,
                    "panes": ids_value(p.panes.iter().map(|t| t.0)),
                    "layout": serde_json::to_value(p.layout).unwrap(),
                    "sizes": p.sizes,
                })
            })
            .collect();
        assert_eq!(&Value::Array(got), want, "{at}: pairs");
    }
}

fn check(store: &Store, window: &str, expect: &Value, at: &str) {
    if WINDOW_FACTS.iter().any(|k| expect.get(*k).is_some()) {
        let w = store
            .window(window)
            .unwrap_or_else(|| panic!("{at}: {window} is open"));
        check_window(w, expect, at);
    }
    if let Some(want) = expect.get("closed") {
        let got = ids_value(store.closed().iter().map(|c| c.tab.id.0));
        assert_eq!(&got, want, "{at}: closed");
    }
    if let Some(want) = expect.get("workspaces") {
        let got: Vec<Value> = store
            .workspaces()
            .iter()
            .map(|w| json!({ "id": w.id.0, "name": w.name, "locations": paths(&w.locations) }))
            .collect();
        assert_eq!(&Value::Array(got), want, "{at}: workspaces");
    }
    if let Some(want) = expect.get("shelf") {
        let got: Vec<Value> = store
            .shelf()
            .iter()
            .map(|i| {
                json!({
                    "id": i.id.0,
                    "name": i.name,
                    "location": i.location.display,
                    "origin": i.origin.display,
                })
            })
            .collect();
        assert_eq!(&Value::Array(got), want, "{at}: shelf");
    }
    if let Some(want) = expect.get("windows") {
        let got: Vec<Value> = store
            .windows()
            .iter()
            .map(|w| Value::from(w.label.clone()))
            .collect();
        assert_eq!(&Value::Array(got), want, "{at}: windows");
    }
    if let Some(others) = expect.get("others").and_then(Value::as_object) {
        for (label, facts) in others {
            let w = store
                .window(label)
                .unwrap_or_else(|| panic!("{at}: {label} is open"));
            check_window(w, facts, &format!("{at} ({label})"));
        }
    }
}

fn run_scenario(name: &str, json: &str) {
    let scenario: Value = serde_json::from_str(json).expect("valid scenario");
    let mut store = Store::new();
    store
        .dispatch(
            W,
            Command::OpenWindow {
                location: None,
                geometry: None,
            },
        )
        .unwrap();
    for (n, step) in scenario["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .enumerate()
    {
        let at = format!("{name} step {}", n + 1);
        let before = store.clone();
        let window = step.get("window").and_then(Value::as_str).unwrap_or(W);
        let result = store.dispatch(window, command(&step["do"]));
        if step.get("error").and_then(Value::as_bool).unwrap_or(false) {
            assert!(result.is_err(), "{at}: expected an error");
            assert_eq!(store, before, "{at}: an error changes nothing");
        } else {
            result.unwrap_or_else(|e| panic!("{at}: {e}"));
        }
        assert_ok(&store);
        check(&store, window, &step["expect"], &at);
    }
}

#[test]
fn basic() {
    run_scenario("basic", include_str!("conformance/basic.json"));
}

#[test]
fn groups_and_pairs() {
    run_scenario(
        "groups_and_pairs",
        include_str!("conformance/groups_and_pairs.json"),
    );
}

#[test]
fn workspaces() {
    run_scenario("workspaces", include_str!("conformance/workspaces.json"));
}

#[test]
fn windows_and_handoff() {
    run_scenario(
        "windows_and_handoff",
        include_str!("conformance/windows_and_handoff.json"),
    );
}

#[test]
fn shelf() {
    run_scenario("shelf", include_str!("conformance/shelf.json"));
}

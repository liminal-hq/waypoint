// Runs the scripted conformance scenarios in `tests/conformance/*.json` against the store.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Scenario format (the TypeScript `FakeTabsApi` test runs the same files):
//
//   { "description": "...", "steps": [ { "do": {...}, "expect": {...}, "error"?: true, "note"?: "..." } ] }
//
// `do` is one command, named by `op`, on one window. Tabs are named by their numeric id, which both
// implementations allocate from 1 in the order tabs are opened, and locations by their path, so
// `"/a"` is `Location { display: "/a", uri: "file:///a" }`. Operations:
//
//   open      { location, after?: id, activate?: bool (default true) }
//   close | activate | back | forward   { tab }
//   move      { tab, index }
//   navigate  { tab, location }
//   pin       { tab, pinned }
//
// `expect` lists only the facts a step checks; each is compared after the step:
//
//   tabs       ids in display order
//   active     the active tab id
//   locations  the current location of each tab, in display order
//   pinned     ids of the pinned tabs, in display order
//   history    { "<id>": { back: [...], forward: [...] } } as paths
//
// A step with `"error": true` must fail and leave the state as it was. Revisions and events are not
// part of the format, because the two implementations number them differently.

use serde_json::Value;
use waypoint_session::{Command, Store, TabId};

mod common;
use common::{assert_ok, loc};

const W: &str = "main-1";

fn id(v: &Value, key: &str) -> TabId {
    TabId(v[key].as_u64().expect(key) as u32)
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

fn check(store: &Store, expect: &Value, at: &str) {
    let w = store.window(W).expect("the window stays open");
    if let Some(want) = expect.get("tabs") {
        let got: Vec<Value> = w.tabs.iter().map(|t| Value::from(t.id.0)).collect();
        assert_eq!(&Value::Array(got), want, "{at}: tabs");
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
        let got: Vec<Value> = w
            .tabs
            .iter()
            .filter(|t| t.pinned)
            .map(|t| Value::from(t.id.0))
            .collect();
        assert_eq!(&Value::Array(got), want, "{at}: pinned");
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
        let result = store.dispatch(W, command(&step["do"]));
        if step.get("error").and_then(Value::as_bool).unwrap_or(false) {
            assert!(result.is_err(), "{at}: expected an error");
            assert_eq!(store, before, "{at}: an error changes nothing");
        } else {
            result.unwrap_or_else(|e| panic!("{at}: {e}"));
        }
        assert_ok(&store);
        check(&store, &step["expect"], &at);
    }
}

#[test]
fn basic() {
    run_scenario("basic", include_str!("conformance/basic.json"));
}

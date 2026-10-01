// Shared helpers for the session tests: location and command shorthands, a frontend-style mirror
// that replays events, and a deterministic generator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![allow(dead_code)]

use std::collections::HashMap;

use waypoint_protocol::Location;
use waypoint_session::{
    Command, Outcome, SessionEvent, SessionSnapshot, Store, TabId, WindowEvent,
};

pub fn loc(name: &str) -> Location {
    Location::new(format!("/{name}"), format!("file:///{name}"))
}

/// A store with one window, `main-1`, holding `names` as tabs (the first active).
pub fn store_with(names: &[&str]) -> Store {
    let mut store = Store::new();
    store
        .dispatch(
            "main-1",
            Command::OpenWindow {
                location: Some(loc(names.first().copied().unwrap_or("home"))),
                geometry: None,
            },
        )
        .unwrap();
    for name in names.iter().skip(1) {
        open(&mut store, "main-1", name);
    }
    store
}

/// Opens a tab at the end of `window` without activating it and returns its id.
pub fn open(store: &mut Store, window: &str, name: &str) -> TabId {
    open_with(store, window, name, None, false)
}

pub fn open_with(
    store: &mut Store,
    window: &str,
    name: &str,
    after: Option<TabId>,
    activate: bool,
) -> TabId {
    let outcome = store
        .dispatch(
            window,
            Command::Open {
                location: loc(name),
                after,
                activate,
            },
        )
        .unwrap();
    let id = outcome.events_for(window).find_map(|e| match e {
        SessionEvent::TabOpened { tab, .. } => Some(tab.id),
        _ => None,
    });
    id.expect("a TabOpened event")
}

pub fn ids(store: &Store, window: &str) -> Vec<u32> {
    store
        .window(window)
        .map(|w| w.tabs.iter().map(|t| t.id.0).collect())
        .unwrap_or_default()
}

pub fn run(store: &mut Store, window: &str, command: Command) -> Outcome {
    store.dispatch(window, command).unwrap()
}

pub fn assert_ok(store: &Store) {
    let problems = store.violations();
    assert!(problems.is_empty(), "invariants broken: {problems:?}");
}

/// Applies one event to a window's snapshot the way a frontend mirror does.
pub fn replay(s: &mut SessionSnapshot, event: &SessionEvent) {
    s.revision = event.revision();
    match event {
        SessionEvent::TabOpened { tab, index, .. }
        | SessionEvent::TabReopened { tab, index, .. } => {
            s.tabs.insert(*index as usize, tab.clone())
        }
        SessionEvent::TabClosed { tab, .. } => {
            s.tabs.retain(|t| t.id != *tab);
            if s.active == Some(*tab) {
                s.active = None;
            }
        }
        SessionEvent::TabActivated { tab, .. } => s.active = Some(*tab),
        SessionEvent::TabMoved { tab, index, .. } => {
            let from = s.tabs.iter().position(|t| t.id == *tab).unwrap();
            let moved = s.tabs.remove(from);
            s.tabs.insert(*index as usize, moved);
        }
        SessionEvent::TabNavigated { tab, .. } | SessionEvent::TabChanged { tab, .. } => {
            let slot = s.tabs.iter_mut().find(|t| t.id == tab.id).unwrap();
            *slot = tab.clone();
        }
        SessionEvent::GroupCreated { group, .. } => s.groups.push(group.clone()),
        SessionEvent::GroupChanged { group, .. } => {
            let slot = s.groups.iter_mut().find(|g| g.id == group.id).unwrap();
            *slot = group.clone();
        }
        SessionEvent::GroupRemoved { group, .. } => s.groups.retain(|g| g.id != *group),
        SessionEvent::PairCreated { pair, .. } => s.pairs.push(pair.clone()),
        SessionEvent::PairChanged { pair, .. } => {
            let slot = s.pairs.iter_mut().find(|p| p.id == pair.id).unwrap();
            *slot = pair.clone();
        }
        SessionEvent::PairRemoved { pair, .. } => s.pairs.retain(|p| p.id != *pair),
        SessionEvent::MruChanged { mru, .. } => s.mru = mru.clone(),
        SessionEvent::ViewChanged { view, .. } => s.view = *view,
        SessionEvent::GeometryChanged { geometry, .. } => s.geometry = Some(*geometry),
        SessionEvent::WindowOpened { .. } | SessionEvent::WindowClosed { .. } => {}
    }
}

/// Every window's snapshot kept current by events alone.
#[derive(Default)]
pub struct Mirror {
    pub windows: HashMap<String, SessionSnapshot>,
}

impl Mirror {
    pub fn apply(&mut self, events: &[WindowEvent]) {
        for WindowEvent { window, event } in events {
            match event {
                SessionEvent::WindowOpened { .. } => {
                    self.windows
                        .insert(window.clone(), SessionSnapshot::empty());
                }
                SessionEvent::WindowClosed { .. } => {
                    self.windows.remove(window);
                }
                _ => replay(self.windows.get_mut(window).expect("window is open"), event),
            }
            if !matches!(event, SessionEvent::WindowClosed { .. }) {
                if let Some(s) = self.windows.get_mut(window) {
                    s.revision = event.revision();
                }
            }
        }
    }

    /// Whether the mirror equals the store, ignoring revision and the closed list (neither is
    /// carried by a window's events).
    pub fn matches(&self, store: &Store) -> Result<(), String> {
        let mut labels: Vec<&String> = self.windows.keys().collect();
        labels.sort();
        let mut want: Vec<&str> = store.windows().iter().map(|w| w.label.as_str()).collect();
        want.sort();
        if labels.iter().map(|l| l.as_str()).collect::<Vec<_>>() != want {
            return Err(format!("windows differ: {labels:?} vs {want:?}"));
        }
        for (label, mirrored) in &self.windows {
            let mut real = store.snapshot(label).unwrap();
            real.revision = 0;
            real.closed.clear();
            let mut mirrored = mirrored.clone();
            mirrored.revision = 0;
            if mirrored != real {
                return Err(format!(
                    "{label} differs:\n mirror {mirrored:?}\n store  {real:?}"
                ));
            }
        }
        Ok(())
    }
}

/// A tiny deterministic generator, so the sequences are reproducible without a dependency.
pub struct Lcg(pub u64);
impl Lcg {
    pub fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % bound.max(1)
    }
}

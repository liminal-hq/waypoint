// The pure reducer: a command applied to a session gives the new session and the events it made.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use thiserror::Error;
use waypoint_protocol::Location;

use crate::model::{SessionEvent, SessionSnapshot, TabId, TabSnapshot};

/// What a caller can ask of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Opens a tab at `location`, after tab `after` or at the end. The first tab of an empty
    /// session is always activated, whatever `activate` says.
    Open {
        location: Location,
        after: Option<TabId>,
        activate: bool,
    },
    /// Closes a tab. Closing the active tab activates the tab that takes its place, or the one
    /// before it when it was last. Closing the last tab leaves an empty session.
    Close {
        tab: TabId,
    },
    Activate {
        tab: TabId,
    },
    /// Moves a tab to `index` in the display order (clamped to the last position).
    Move {
        tab: TabId,
        index: usize,
    },
    /// Goes somewhere new: the current location joins the back stack and the forward stack clears.
    Navigate {
        tab: TabId,
        location: Location,
    },
    Back {
        tab: TabId,
    },
    Forward {
        tab: TabId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SessionError {
    #[error("no such tab: {0}")]
    UnknownTab(u32),
}

/// The state behind a snapshot, plus the counter for the next tab id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) snapshot: SessionSnapshot,
    pub(crate) next_id: u32,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: SessionSnapshot {
                revision: 0,
                tabs: Vec::new(),
                active: None,
            },
            next_id: 1,
        }
    }
}

fn position(state: &SessionSnapshot, tab: TabId) -> Result<usize, SessionError> {
    state
        .tabs
        .iter()
        .position(|t| t.id == tab)
        .ok_or(SessionError::UnknownTab(tab.0))
}

/// Applies a command without mutating the input. A command that changes nothing (activating the
/// active tab, going back with no history) returns the state unchanged, no events and the same
/// revision. An unknown tab is an error and leaves the session as it was.
pub(crate) fn reduce(
    state: &State,
    command: Command,
) -> Result<(State, Vec<SessionEvent>), SessionError> {
    let mut next = state.clone();
    let mut events = Vec::new();
    let s = &mut next.snapshot;
    // Every event takes the next revision.
    let mut revision = s.revision;
    let mut bump = || {
        revision += 1;
        revision
    };

    match command {
        Command::Open {
            location,
            after,
            activate,
        } => {
            let index = match after {
                Some(after) => position(s, after)? + 1,
                None => s.tabs.len(),
            };
            let id = TabId(next.next_id);
            next.next_id += 1;
            let tab = TabSnapshot {
                id,
                location,
                back: Vec::new(),
                forward: Vec::new(),
            };
            s.tabs.insert(index, tab.clone());
            events.push(SessionEvent::TabOpened {
                tab,
                index: index as u32,
                revision: bump(),
            });
            if activate || s.active.is_none() {
                s.active = Some(id);
                events.push(SessionEvent::TabActivated {
                    tab: id,
                    revision: bump(),
                });
            }
        }
        Command::Close { tab } => {
            let index = position(s, tab)?;
            s.tabs.remove(index);
            events.push(SessionEvent::TabClosed {
                tab,
                revision: bump(),
            });
            if s.active == Some(tab) {
                let neighbour = s
                    .tabs
                    .get(index)
                    .or_else(|| index.checked_sub(1).and_then(|i| s.tabs.get(i)))
                    .map(|t| t.id);
                s.active = neighbour;
                if let Some(id) = neighbour {
                    events.push(SessionEvent::TabActivated {
                        tab: id,
                        revision: bump(),
                    });
                }
            }
        }
        Command::Activate { tab } => {
            position(s, tab)?;
            if s.active != Some(tab) {
                s.active = Some(tab);
                events.push(SessionEvent::TabActivated {
                    tab,
                    revision: bump(),
                });
            }
        }
        Command::Move { tab, index } => {
            let from = position(s, tab)?;
            let to = index.min(s.tabs.len() - 1);
            if from != to {
                let moved = s.tabs.remove(from);
                s.tabs.insert(to, moved);
                events.push(SessionEvent::TabMoved {
                    tab,
                    index: to as u32,
                    revision: bump(),
                });
            }
        }
        Command::Navigate { tab, location } => {
            let index = position(s, tab)?;
            let t = &mut s.tabs[index];
            if t.location != location {
                let previous = std::mem::replace(&mut t.location, location);
                t.back.push(previous);
                t.forward.clear();
                events.push(SessionEvent::TabNavigated {
                    tab: t.clone(),
                    revision: bump(),
                });
            }
        }
        Command::Back { tab } => {
            let index = position(s, tab)?;
            let t = &mut s.tabs[index];
            if let Some(previous) = t.back.pop() {
                let current = std::mem::replace(&mut t.location, previous);
                t.forward.push(current);
                events.push(SessionEvent::TabNavigated {
                    tab: t.clone(),
                    revision: bump(),
                });
            }
        }
        Command::Forward { tab } => {
            let index = position(s, tab)?;
            let t = &mut s.tabs[index];
            if let Some(following) = t.forward.pop() {
                let current = std::mem::replace(&mut t.location, following);
                t.back.push(current);
                events.push(SessionEvent::TabNavigated {
                    tab: t.clone(),
                    revision: bump(),
                });
            }
        }
    }

    next.snapshot.revision = revision;
    Ok((next, events))
}

/// The pure form of the reducer over a snapshot: the new snapshot and the events. Tab ids are
/// allocated above every id in the snapshot, so replaying from a snapshot is deterministic.
pub fn apply(
    snapshot: &SessionSnapshot,
    command: Command,
) -> Result<(SessionSnapshot, Vec<SessionEvent>), SessionError> {
    let next_id = snapshot.tabs.iter().map(|t| t.id.0).max().unwrap_or(0) + 1;
    let state = State {
        snapshot: snapshot.clone(),
        next_id,
    };
    let (state, events) = reduce(&state, command)?;
    Ok((state.snapshot, events))
}

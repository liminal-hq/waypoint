// Turns the difference between two stores into granular events. The reducer mutates a copy and
// the events follow from what changed, so a command that changes nothing makes no event.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::model::{SessionEvent, TabId, TabSnapshot, WindowEvent, WindowState};
use crate::store::Store;

/// Events from `before` to `after`, in an order a mirror can apply one by one. Revisions are
/// left at zero for the caller to stamp.
pub(crate) fn store_events(
    before: &Store,
    after: &Store,
    reopened: Option<TabId>,
) -> Vec<WindowEvent> {
    let mut out = Vec::new();
    let mut push = |window: &str, event: SessionEvent| {
        out.push(WindowEvent {
            window: window.to_string(),
            event,
        })
    };
    for old in &before.windows {
        match after.window(&old.label) {
            Some(new) => {
                for event in window_events(old, new, reopened) {
                    push(&old.label, event);
                }
                if before.workspaces != after.workspaces {
                    push(&old.label, workspaces_changed(after));
                }
            }
            None => {
                for tab in &old.tabs {
                    push(
                        &old.label,
                        SessionEvent::TabClosed {
                            tab: tab.id,
                            revision: 0,
                        },
                    );
                }
                push(
                    &old.label,
                    SessionEvent::WindowClosed {
                        window: old.label.clone(),
                        revision: 0,
                    },
                );
            }
        }
    }
    for new in &after.windows {
        if before.window(&new.label).is_some() {
            continue;
        }
        push(
            &new.label,
            SessionEvent::WindowOpened {
                window: new.label.clone(),
                revision: 0,
            },
        );
        for event in window_events(&WindowState::new(new.label.clone()), new, reopened) {
            push(&new.label, event);
        }
        if !after.workspaces.is_empty() {
            push(&new.label, workspaces_changed(after));
        }
    }
    out
}

fn workspaces_changed(after: &Store) -> SessionEvent {
    SessionEvent::WorkspacesChanged {
        workspaces: after.workspaces.clone(),
        revision: 0,
    }
}

fn navigated(a: &TabSnapshot, b: &TabSnapshot) -> bool {
    a.location != b.location || a.back != b.back || a.forward != b.forward
}

fn window_events(
    before: &WindowState,
    after: &WindowState,
    reopened: Option<TabId>,
) -> Vec<SessionEvent> {
    let mut out = Vec::new();
    let r = 0;

    for g in &after.groups {
        if before.group(g.id).is_none() {
            out.push(SessionEvent::GroupCreated {
                group: g.clone(),
                revision: r,
            });
        }
    }
    for p in &after.pairs {
        if before.pair(p.id).is_none() {
            out.push(SessionEvent::PairCreated {
                pair: p.clone(),
                revision: r,
            });
        }
    }

    let mut current: Vec<TabId> = Vec::new();
    for t in &before.tabs {
        if after.index_of(t.id).is_none() {
            out.push(SessionEvent::TabClosed {
                tab: t.id,
                revision: r,
            });
        } else {
            current.push(t.id);
        }
    }
    let target: Vec<TabId> = after.tabs.iter().map(|t| t.id).collect();
    for (index, t) in after.tabs.iter().enumerate() {
        if before.index_of(t.id).is_some() {
            continue;
        }
        let at = index.min(current.len());
        current.insert(at, t.id);
        out.push(if reopened == Some(t.id) {
            SessionEvent::TabReopened {
                tab: t.clone(),
                index: at as u32,
                revision: r,
            }
        } else {
            SessionEvent::TabOpened {
                tab: t.clone(),
                index: at as u32,
                revision: r,
            }
        });
    }
    for (tab, index) in reorder(&current, &target) {
        out.push(SessionEvent::TabMoved {
            tab,
            index: index as u32,
            revision: r,
        });
    }

    for t in &after.tabs {
        let Some(old) = before.tab(t.id) else {
            continue;
        };
        if old == t {
            continue;
        }
        out.push(if navigated(old, t) {
            SessionEvent::TabNavigated {
                tab: t.clone(),
                revision: r,
            }
        } else {
            SessionEvent::TabChanged {
                tab: t.clone(),
                revision: r,
            }
        });
    }
    if after.active != before.active {
        if let Some(tab) = after.active {
            out.push(SessionEvent::TabActivated { tab, revision: r });
        }
    }
    for p in &after.pairs {
        if let Some(old) = before.pair(p.id) {
            if old != p {
                out.push(SessionEvent::PairChanged {
                    pair: p.clone(),
                    revision: r,
                });
            }
        }
    }
    for g in &after.groups {
        if let Some(old) = before.group(g.id) {
            if old != g {
                out.push(SessionEvent::GroupChanged {
                    group: g.clone(),
                    revision: r,
                });
            }
        }
    }
    for p in &before.pairs {
        if after.pair(p.id).is_none() {
            out.push(SessionEvent::PairRemoved {
                pair: p.id,
                revision: r,
            });
        }
    }
    for g in &before.groups {
        if after.group(g.id).is_none() {
            out.push(SessionEvent::GroupRemoved {
                group: g.id,
                revision: r,
            });
        }
    }
    if after.mru != before.mru {
        out.push(SessionEvent::MruChanged {
            mru: after.mru.clone(),
            revision: r,
        });
    }
    if after.view != before.view {
        out.push(SessionEvent::ViewChanged {
            view: after.view,
            revision: r,
        });
    }
    if after.workspace != before.workspace {
        out.push(SessionEvent::WorkspaceActivated {
            workspace: after.workspace,
            revision: r,
        });
    }
    if after.geometry != before.geometry {
        if let Some(geometry) = after.geometry {
            out.push(SessionEvent::GeometryChanged {
                geometry,
                revision: r,
            });
        }
    }
    out
}

/// The `(tab, index)` moves that turn `from` into `to` (the same set of tabs). One move when a
/// single tab is out of place, otherwise a left-to-right pass.
fn reorder(from: &[TabId], to: &[TabId]) -> Vec<(TabId, usize)> {
    if from == to {
        return Vec::new();
    }
    for (i, x) in from.iter().enumerate() {
        let rest_from: Vec<TabId> = from.iter().copied().filter(|t| t != x).collect();
        let rest_to: Vec<TabId> = to.iter().copied().filter(|t| t != x).collect();
        if rest_from == rest_to {
            let at = to.iter().position(|t| t == x).unwrap_or(i);
            return vec![(*x, at)];
        }
    }
    let mut moves = Vec::new();
    let mut cur = from.to_vec();
    for (i, want) in to.iter().enumerate() {
        if cur[i] != *want {
            let at = cur.iter().position(|t| t == want).unwrap_or(i);
            let moved = cur.remove(at);
            cur.insert(i, moved);
            moves.push((*want, i));
        }
    }
    moves
}

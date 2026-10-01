// A live single-window session: the milestone 2 view over the store, with the hook the
// composition root subscribes to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::model::{SessionEvent, SessionSnapshot, TabId, WindowState};
use crate::reducer::{reject_multi_window, Command, SessionError, COMPAT_WINDOW};
use crate::store::{Store, StorePolicy};

type TabClosedHook = Box<dyn Fn(TabId) + Send + Sync>;

/// One window's session. It is a one-window `Store` that keeps an empty window when its last tab
/// closes, so the milestone 2 plugin and frontend behave as before; the app's multi-window rules
/// live in `Store` itself. It owns the state, applies commands and tells subscribers when a tab
/// closes, so the composition root can release what the tab held (for example its listing
/// handle) without this crate knowing about the file system.
pub struct Session {
    store: Store,
    on_tab_closed: Vec<TabClosedHook>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        let mut store = Store::with_policy(StorePolicy {
            close_window_on_last_tab: false,
        });
        store.windows.push(WindowState::new(COMPAT_WINDOW));
        store.next_window = 2;
        Self {
            store,
            on_tab_closed: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        self.store
            .snapshot(COMPAT_WINDOW)
            .expect("a session always has its window")
    }

    /// Subscribes to tab closes: the callback runs synchronously for every tab removed by a close
    /// command or by `shutdown`, so it must be quick and must not call back into this session.
    pub fn on_tab_closed(&mut self, hook: impl Fn(TabId) + Send + Sync + 'static) {
        self.on_tab_closed.push(Box::new(hook));
    }

    /// Applies a command and returns the events it produced for this window. On an error the
    /// session is unchanged. The window commands (`OpenWindow`, `CloseWindow`, `MoveTabs`) are
    /// rejected with `SessionError::Invalid`: this session's one window is never lost.
    pub fn dispatch(&mut self, command: Command) -> Result<Vec<SessionEvent>, SessionError> {
        reject_multi_window(&command)?;
        let outcome = self.store.dispatch(COMPAT_WINDOW, command)?;
        let events: Vec<SessionEvent> = outcome.events_for(COMPAT_WINDOW).cloned().collect();
        for event in &events {
            if let SessionEvent::TabClosed { tab, .. } = event {
                self.notify_closed(*tab);
            }
        }
        Ok(events)
    }

    /// Ends the session (its window closed): every remaining tab counts as closed.
    pub fn shutdown(self) {
        if let Some(w) = self.store.window(COMPAT_WINDOW) {
            for tab in &w.tabs {
                self.notify_closed(tab.id);
            }
        }
    }

    fn notify_closed(&self, tab: TabId) {
        for hook in &self.on_tab_closed {
            hook(tab);
        }
    }
}

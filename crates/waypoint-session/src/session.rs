// A live session: the reducer's state plus the hooks the composition root subscribes to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::model::{SessionEvent, SessionSnapshot, TabId};
use crate::reducer::{reduce, Command, SessionError, State};

type TabClosedHook = Box<dyn Fn(TabId) + Send + Sync>;

/// One window's session. It owns the state, applies commands and tells subscribers when a tab
/// closes, so the composition root can release what the tab held (for example its listing
/// handle) without this crate knowing about the file system.
pub struct Session {
    state: State,
    on_tab_closed: Vec<TabClosedHook>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            state: State::new(),
            on_tab_closed: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        self.state.snapshot.clone()
    }

    /// Subscribes to tab closes: the callback runs synchronously for every tab removed by a close
    /// command or by `shutdown`, so it must be quick and must not call back into this session.
    pub fn on_tab_closed(&mut self, hook: impl Fn(TabId) + Send + Sync + 'static) {
        self.on_tab_closed.push(Box::new(hook));
    }

    /// Applies a command and returns the events it produced. On an error the session is unchanged.
    pub fn dispatch(&mut self, command: Command) -> Result<Vec<SessionEvent>, SessionError> {
        let (state, events) = reduce(&self.state, command)?;
        self.state = state;
        for event in &events {
            if let SessionEvent::TabClosed { tab, .. } = event {
                self.notify_closed(*tab);
            }
        }
        Ok(events)
    }

    /// Ends the session (its window closed): every remaining tab counts as closed.
    pub fn shutdown(self) {
        for tab in &self.state.snapshot.tabs {
            self.notify_closed(tab.id);
        }
    }

    fn notify_closed(&self, tab: TabId) {
        for hook in &self.on_tab_closed {
            hook(tab);
        }
    }
}

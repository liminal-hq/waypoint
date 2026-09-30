// The per-window session registry held in Tauri state.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use waypoint_session::{Command, Session, SessionEvent, SessionSnapshot, TabId};

use crate::error::Error;

type Hook = Arc<dyn Fn(&str, TabId) + Send + Sync>;

/// Every window's session, created on first use. The composition root subscribes to tab closes
/// with `on_tab_closed` (for example to close the tab's listing handle); the plugin itself knows
/// nothing about listings.
#[derive(Default)]
pub struct Sessions {
    windows: Mutex<HashMap<String, Session>>,
    hooks: Arc<Mutex<Vec<Hook>>>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock only means a hook panicked; the state is still consistent.
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Sessions {
    /// Runs `hook(window_label, tab)` whenever a tab closes, including every remaining tab when
    /// its window closes. It runs while the window's session is locked, so it must be quick and
    /// must not call back into the session commands.
    pub fn on_tab_closed(&self, hook: impl Fn(&str, TabId) + Send + Sync + 'static) {
        locked(&self.hooks).push(Arc::new(hook));
    }

    fn session_for<'a>(
        &self,
        windows: &'a mut HashMap<String, Session>,
        label: &str,
    ) -> &'a mut Session {
        windows.entry(label.to_string()).or_insert_with(|| {
            let mut session = Session::new();
            let hooks = Arc::clone(&self.hooks);
            let label = label.to_string();
            session.on_tab_closed(move |tab| {
                for hook in locked(&hooks).iter() {
                    hook(&label, tab);
                }
            });
            session
        })
    }

    pub fn snapshot(&self, label: &str) -> SessionSnapshot {
        let mut windows = locked(&self.windows);
        self.session_for(&mut windows, label).snapshot()
    }

    pub fn dispatch(&self, label: &str, command: Command) -> Result<Vec<SessionEvent>, Error> {
        let mut windows = locked(&self.windows);
        Ok(self.session_for(&mut windows, label).dispatch(command)?)
    }

    /// Drops a window's session, reporting its remaining tabs as closed.
    pub fn end(&self, label: &str) {
        let session = locked(&self.windows).remove(label);
        if let Some(session) = session {
            session.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waypoint_protocol::Location;

    fn open() -> Command {
        Command::Open {
            location: Location::new("/a", "file:///a"),
            after: None,
            activate: true,
        }
    }

    #[test]
    fn windows_have_separate_sessions() {
        let sessions = Sessions::default();
        sessions.dispatch("main-1", open()).unwrap();
        assert_eq!(sessions.snapshot("main-1").tabs.len(), 1);
        assert!(sessions.snapshot("main-2").tabs.is_empty());
    }

    #[test]
    fn hooks_hear_closes_with_the_window_label_and_the_window_ending() {
        let sessions = Sessions::default();
        let heard = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&heard);
        sessions
            .on_tab_closed(move |label, tab| sink.lock().unwrap().push((label.to_string(), tab)));
        sessions.dispatch("main-1", open()).unwrap();
        sessions.dispatch("main-1", open()).unwrap();
        sessions
            .dispatch("main-1", Command::Close { tab: TabId(1) })
            .unwrap();
        sessions.end("main-1");
        assert_eq!(
            *heard.lock().unwrap(),
            vec![
                ("main-1".to_string(), TabId(1)),
                ("main-1".to_string(), TabId(2))
            ]
        );
        assert!(sessions.snapshot("main-1").tabs.is_empty());
    }

    #[test]
    fn an_unknown_tab_is_an_error() {
        let sessions = Sessions::default();
        assert!(sessions
            .dispatch("main-1", Command::Activate { tab: TabId(5) })
            .is_err());
    }
}

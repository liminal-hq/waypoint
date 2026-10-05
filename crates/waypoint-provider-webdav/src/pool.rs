// The sessions of every connection, keyed by `ConnectionKey`, with the state the app shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use waypoint_path::ConnectionKey;
use waypoint_protocol::ConnectionState;

use crate::client::Session;
use crate::tls::TrustState;

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// One connection. HTTP has no session to open, so "connected" means the last request reached the
/// server and was answered; the session itself (client, login) is built by the first call.
pub(crate) struct Slot {
    state: Mutex<ConnectionState>,
    pub(crate) session: Mutex<Option<Arc<Session>>>,
    /// What this connection trusts beyond the system. It outlives its session: an answer to a
    /// certificate question holds until the app closes, not until the connection idles.
    pub(crate) trust: Arc<TrustState>,
    /// A path on the server that was used, which connecting again asks about.
    pub(crate) probe: Mutex<Option<String>>,
}

impl Slot {
    fn new() -> Self {
        Self {
            state: Mutex::new(ConnectionState::Idle),
            session: Mutex::new(None),
            trust: Arc::new(TrustState::default()),
            probe: Mutex::new(None),
        }
    }

    pub(crate) fn set_state(&self, state: ConnectionState) {
        *lock(&self.state) = state;
    }

    pub(crate) fn state(&self) -> ConnectionState {
        lock(&self.state).clone()
    }

    /// Closes the session. The next call builds another.
    pub(crate) fn close(&self) {
        *lock(&self.session) = None;
        self.set_state(ConnectionState::Idle);
    }
}

#[derive(Default)]
pub(crate) struct Pool {
    slots: Mutex<HashMap<ConnectionKey, Arc<Slot>>>,
}

impl Pool {
    pub(crate) fn slot(&self, key: &ConnectionKey) -> Arc<Slot> {
        lock(&self.slots)
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Slot::new()))
            .clone()
    }

    pub(crate) fn existing(&self, key: &ConnectionKey) -> Option<Arc<Slot>> {
        lock(&self.slots).get(key).cloned()
    }
}

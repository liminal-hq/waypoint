// The sessions of every connection, keyed by `ConnectionKey`, with the state the app shows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use waypoint_path::ConnectionKey;
use waypoint_protocol::{ConnectionState, VfsError};

use crate::session::Session;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// One connection: its state, its open session, and the gate that makes one caller connect while
/// the others wait for the result.
pub(crate) struct Slot {
    state: Mutex<ConnectionState>,
    live: Mutex<Option<Arc<Session>>>,
    /// Held while connecting, so one caller connects while the others wait for the result.
    pub(crate) gate: tokio::sync::Mutex<()>,
}

impl Slot {
    fn new() -> Self {
        Self {
            state: Mutex::new(ConnectionState::Idle),
            live: Mutex::new(None),
            gate: tokio::sync::Mutex::new(()),
        }
    }

    /// The open session, unless it has closed (then it is forgotten).
    pub(crate) fn live(&self) -> Option<Arc<Session>> {
        let mut live = lock(&self.live);
        if live.as_ref().is_some_and(|session| session.is_closed()) {
            *live = None;
            *lock(&self.state) = ConnectionState::Idle;
        }
        live.clone()
    }

    pub(crate) fn set_state(&self, state: ConnectionState) {
        *lock(&self.state) = state;
    }

    pub(crate) fn connected(&self, session: Arc<Session>) {
        *lock(&self.live) = Some(session);
        self.set_state(ConnectionState::Connected);
    }

    /// Forgets `session` after it failed with `error`, unless another has replaced it already.
    pub(crate) fn lost(&self, session: &Arc<Session>, error: VfsError) {
        let mut live = lock(&self.live);
        if live
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, session))
        {
            *live = None;
            *lock(&self.state) = ConnectionState::Failed { error };
        }
    }

    /// Takes the open session away, for closing.
    pub(crate) fn take(&self) -> Option<Arc<Session>> {
        let session = lock(&self.live).take();
        self.set_state(ConnectionState::Idle);
        session
    }

    pub(crate) fn state(&self) -> ConnectionState {
        self.live();
        lock(&self.state).clone()
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

// Lock helpers that carry on after a thread panicked while holding the lock.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

/// Locks, recovering the data if another thread panicked while holding the lock: the state behind
/// these locks is always left consistent between statements.
pub(crate) fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn wait<'a, T>(condvar: &Condvar, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
    condvar.wait(guard).unwrap_or_else(|e| e.into_inner())
}

#[cfg(any(test, feature = "testing"))]
pub(crate) fn wait_timeout<'a, T>(
    condvar: &Condvar,
    guard: MutexGuard<'a, T>,
    timeout: Duration,
) -> MutexGuard<'a, T> {
    condvar
        .wait_timeout(guard, timeout)
        .unwrap_or_else(|e| e.into_inner())
        .0
}

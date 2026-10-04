// A `Pacer` on a virtual clock: sleeping moves the clock and takes no time, so a copy throttled to a
// few megabytes a second is measured in microseconds of real time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Mutex;
use std::time::Duration;

use crate::throttle::Pacer;

type SleepHook = Box<dyn FnMut(Duration) + Send>;

#[derive(Default)]
pub struct VirtualPacer {
    now: Mutex<Duration>,
    hook: Mutex<Option<SleepHook>>,
}

impl VirtualPacer {
    /// Calls `hook` with the clock after every sleep, to change something while a copy waits.
    pub fn on_sleep(&self, hook: SleepHook) {
        *self.hook.lock().unwrap_or_else(|e| e.into_inner()) = Some(hook);
    }

    /// Moves the clock on without a sleep.
    pub fn advance(&self, by: Duration) {
        *self.now.lock().unwrap_or_else(|e| e.into_inner()) += by;
    }
}

impl Pacer for VirtualPacer {
    fn now(&self) -> Duration {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn sleep(&self, duration: Duration) {
        self.advance(duration);
        let now = self.now();
        if let Some(hook) = self.hook.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            hook(now);
        }
    }
}

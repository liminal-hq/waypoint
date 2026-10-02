// The worker pool behind the queue: threads that take jobs, run a platform processor and send each result to the requests waiting for it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

use crate::models::{SkipWhy, ThumbEvent, ThumbRequest, Ticket};
use crate::queue::{Queue, Sink};

/// What a processor made of one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Ready {
        url: String,
    },
    Failed {
        reason: String,
    },
    Skipped {
        why: SkipWhy,
    },
    /// The work was cancelled; nobody is told.
    Cancelled,
}

/// Makes the thumbnail for one request. One implementation per platform; the engine does not know which.
pub trait Processor: Send + Sync + 'static {
    /// Makes (or finds) the thumbnail. A long-running step should watch `cancel` and stop early.
    fn process(&self, request: &ThumbRequest, cancel: &AtomicBool) -> Outcome;

    /// Runs once on each worker thread when it starts; what it returns is kept until the thread ends (the Windows processor enters a COM apartment here and leaves it when the guard drops).
    fn on_worker_start(&self) -> Box<dyn Any> {
        Box::new(())
    }
}

/// Limits the app can change while the plugin runs.
pub struct Limits {
    max_file_bytes: AtomicU64,
    external_timeout_ms: AtomicU64,
}

impl Limits {
    pub fn new(max_file_bytes: u64, external_timeout: std::time::Duration) -> Self {
        Limits {
            max_file_bytes: AtomicU64::new(max_file_bytes),
            external_timeout_ms: AtomicU64::new(external_timeout.as_millis() as u64),
        }
    }

    pub fn max_file_bytes(&self) -> u64 {
        self.max_file_bytes.load(Ordering::Relaxed)
    }

    pub fn set_max_file_bytes(&self, bytes: u64) {
        self.max_file_bytes.store(bytes, Ordering::Relaxed);
    }

    pub fn external_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.external_timeout_ms.load(Ordering::Relaxed))
    }
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

struct State {
    queue: Queue,
    shutdown: bool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic never happens while the queue is being changed, but a poisoned lock is still usable.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The queue and its workers. Dropping it stops them: pending work is discarded and running work is told to stop.
pub struct Engine {
    shared: Arc<Shared>,
}

impl Engine {
    /// Starts `workers` threads (at least one) that run `processor`.
    pub fn new(workers: usize, processor: Arc<dyn Processor>) -> Engine {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                queue: Queue::new(),
                shutdown: false,
            }),
            wake: Condvar::new(),
        });
        for n in 0..workers.max(1) {
            let shared = Arc::clone(&shared);
            let processor = Arc::clone(&processor);
            thread::Builder::new()
                .name(format!("thumbnails-{n}"))
                .spawn(move || worker(&shared, processor.as_ref()))
                .expect("a thread can be spawned");
        }
        Engine { shared }
    }

    /// Queues the items as one request; each result goes to `sink` unless the request is cancelled first.
    pub fn request(&self, items: Vec<ThumbRequest>, sink: Sink) -> Ticket {
        let ticket = self.shared.lock().queue.enqueue(items, sink);
        self.shared.wake.notify_all();
        ticket
    }

    pub fn cancel(&self, ticket: Ticket) -> bool {
        self.shared.lock().queue.cancel(ticket)
    }

    pub fn prioritise(&self, ticket: Ticket, keys: &[String]) {
        self.shared.lock().queue.prioritise(ticket, keys);
    }

    /// How many jobs wait and how many run.
    pub fn load(&self) -> (usize, usize) {
        let state = self.shared.lock();
        (state.queue.pending_len(), state.queue.running_len())
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.shutdown = true;
        state.queue.clear();
        drop(state);
        self.shared.wake.notify_all();
    }
}

fn worker(shared: &Shared, processor: &dyn Processor) {
    let _guard = processor.on_worker_start();
    loop {
        let work = {
            let mut state = shared.lock();
            loop {
                if state.shutdown {
                    return;
                }
                if let Some(work) = state.queue.take_next() {
                    break work;
                }
                state = shared
                    .wake
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            processor.process(&work.request, &work.cancel)
        }))
        .unwrap_or_else(|_| Outcome::Failed {
            reason: "the thumbnail generator crashed".to_string(),
        });
        let subscribers = shared.lock().queue.finish(work.id);
        let key = work.request.key;
        let event = match outcome {
            Outcome::Ready { url } => ThumbEvent::Ready { key, url },
            Outcome::Failed { reason } => ThumbEvent::Failed { key, reason },
            Outcome::Skipped { why } => ThumbEvent::Skipped { key, why },
            Outcome::Cancelled => continue,
        };
        for subscriber in subscribers {
            (subscriber.sink)(event.clone());
        }
    }
}

#[cfg(test)]
mod tests;

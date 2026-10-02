// The request queue: last in, first out, with one job per key, cancellation by ticket and reprioritisation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::models::{ThumbEvent, ThumbRequest, Ticket};

/// Where the events of a request go.
pub type Sink = Arc<dyn Fn(ThumbEvent) + Send + Sync>;

/// A request that wants the result of a job.
#[derive(Clone)]
pub struct Subscriber {
    pub ticket: Ticket,
    pub sink: Sink,
}

struct Job {
    id: u64,
    request: ThumbRequest,
    subscribers: Vec<Subscriber>,
}

struct Running {
    request: ThumbRequest,
    subscribers: Vec<Subscriber>,
    cancel: Arc<AtomicBool>,
}

/// What a worker takes from the queue.
pub struct Work {
    pub id: u64,
    pub request: ThumbRequest,
    /// Set when every request that wanted this job has been cancelled; a generator that can stop early should.
    pub cancel: Arc<AtomicBool>,
}

/// Pending and running jobs. The newest request is served first, because what the user scrolled to last is what they are looking at; within one request, the items are served in the order they were given. A key is one job however many requests ask for it: a later request for a pending job moves it to the front and shares it, and one for a running job shares the running one.
#[derive(Default)]
pub struct Queue {
    /// The next job to run is the last.
    pending: Vec<Job>,
    running: HashMap<u64, Running>,
    next_ticket: u64,
    next_job: u64,
}

impl Queue {
    pub fn new() -> Self {
        Queue::default()
    }

    /// Adds the items as one request and returns its ticket.
    pub fn enqueue(&mut self, items: Vec<ThumbRequest>, sink: Sink) -> Ticket {
        self.next_ticket += 1;
        let ticket = Ticket(self.next_ticket);
        let subscriber = Subscriber { ticket, sink };
        // Pushed in reverse so the first item ends up last, where the next job is taken from.
        for request in items.into_iter().rev() {
            if let Some(running) = self.running.values_mut().find(|running| {
                running.request == request && !running.cancel.load(Ordering::Relaxed)
            }) {
                add_subscriber(&mut running.subscribers, &subscriber);
                continue;
            }
            if let Some(at) = self
                .pending
                .iter()
                .position(|job| job.request.key == request.key)
            {
                let mut job = self.pending.remove(at);
                // The newest description of the item wins.
                job.request = request;
                add_subscriber(&mut job.subscribers, &subscriber);
                self.pending.push(job);
                continue;
            }
            self.next_job += 1;
            self.pending.push(Job {
                id: self.next_job,
                request,
                subscribers: vec![subscriber.clone()],
            });
        }
        ticket
    }

    /// Takes the next job to run, marking it running.
    pub fn take_next(&mut self) -> Option<Work> {
        let job = self.pending.pop()?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.running.insert(
            job.id,
            Running {
                request: job.request.clone(),
                subscribers: job.subscribers,
                cancel: Arc::clone(&cancel),
            },
        );
        Some(Work {
            id: job.id,
            request: job.request,
            cancel,
        })
    }

    /// Ends a running job and returns the requests still waiting for it, which is none when they were all cancelled.
    pub fn finish(&mut self, id: u64) -> Vec<Subscriber> {
        self.running
            .remove(&id)
            .map(|running| running.subscribers)
            .unwrap_or_default()
    }

    /// Withdraws a request: its pending jobs that nobody else wants are dropped, and a running job nobody else wants is told to stop. Returns false if the ticket was not known.
    pub fn cancel(&mut self, ticket: Ticket) -> bool {
        let mut known = false;
        let mut drop_ticket = |subscribers: &mut Vec<Subscriber>| {
            let before = subscribers.len();
            subscribers.retain(|subscriber| subscriber.ticket != ticket);
            known |= subscribers.len() != before;
        };
        self.pending.retain_mut(|job| {
            drop_ticket(&mut job.subscribers);
            !job.subscribers.is_empty()
        });
        for running in self.running.values_mut() {
            drop_ticket(&mut running.subscribers);
            if running.subscribers.is_empty() {
                running.cancel.store(true, Ordering::Relaxed);
            }
        }
        known
    }

    /// Moves the request's pending jobs for `keys` to the front, in the order given.
    pub fn prioritise(&mut self, ticket: Ticket, keys: &[String]) {
        for key in keys.iter().rev() {
            let at = self.pending.iter().position(|job| {
                job.request.key == *key && job.subscribers.iter().any(|s| s.ticket == ticket)
            });
            if let Some(at) = at {
                let job = self.pending.remove(at);
                self.pending.push(job);
            }
        }
    }

    /// Stops everything: nothing pending runs, and every running job is told to stop.
    pub fn clear(&mut self) {
        self.pending.clear();
        for running in self.running.values() {
            running.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn running_len(&self) -> usize {
        self.running.len()
    }
}

fn add_subscriber(subscribers: &mut Vec<Subscriber>, subscriber: &Subscriber) {
    if !subscribers.iter().any(|s| s.ticket == subscriber.ticket) {
        subscribers.push(subscriber.clone());
    }
}

#[cfg(test)]
mod tests;

// The helper's side of the stream out: one writer that every thread sends through, and the bounded
// queue a watch's events wait in.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

use waypoint_vfs::{RescanReason, WatchEvent};

use crate::frame::{write_frame, Frame, FrameError};
use crate::sync::{locked, wait};
use crate::wire::{Event, EventBody, Message, Reply, Response};

/// Why a message did not go out.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SendError {
    /// The message is over the frame limit; nothing was written.
    TooLarge,
    /// The stream is gone.
    Broken,
}

/// The writer shared by the workers, the watch emitters and the loop. A frame is written whole
/// under the lock, so frames never interleave; once a write fails every later send fails at once.
pub(crate) struct Outbox {
    writer: Mutex<Box<dyn Write + Send>>,
    broken: AtomicBool,
}

impl Outbox {
    pub(crate) fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Mutex::new(writer),
            broken: AtomicBool::new(false),
        }
    }

    pub(crate) fn is_broken(&self) -> bool {
        self.broken.load(Ordering::Relaxed)
    }

    fn send_frame(&self, frame: &Frame) -> Result<(), SendError> {
        if self.is_broken() {
            return Err(SendError::Broken);
        }
        let mut writer = locked(&self.writer);
        match write_frame(&mut *writer, frame) {
            Ok(()) => Ok(()),
            Err(FrameError::TooLarge) => Err(SendError::TooLarge),
            Err(_) => {
                self.broken.store(true, Ordering::Relaxed);
                Err(SendError::Broken)
            }
        }
    }

    pub(crate) fn event(&self, id: u64, body: EventBody) -> Result<(), SendError> {
        let frame = Message::Event(Event { id, body })
            .to_frame()
            .map_err(|_| SendError::TooLarge)?;
        self.send_frame(&frame)
    }

    pub(crate) fn reply(&self, id: u64, reply: Reply) -> Result<(), SendError> {
        let frame = Message::Response(Response { id, reply })
            .to_frame()
            .map_err(|_| SendError::TooLarge)?;
        match self.send_frame(&frame) {
            Err(SendError::TooLarge) => {
                // The answer would not fit in a frame: the request still gets its one response.
                let failed = Reply::Error {
                    error: waypoint_protocol::VfsError::Io {
                        message: "the answer is too large".to_owned(),
                        location: None,
                    },
                };
                let frame = Message::Response(Response { id, reply: failed })
                    .to_frame()
                    .map_err(|_| SendError::TooLarge)?;
                self.send_frame(&frame)
            }
            other => other,
        }
    }

    pub(crate) fn data(&self, id: u64, bytes: Vec<u8>) -> Result<(), SendError> {
        self.send_frame(&Frame::Data { id, bytes })
    }
}

struct QueueState {
    events: VecDeque<WatchEvent>,
    /// The queue overflowed and holds the one `Rescan` that stands for everything dropped since;
    /// events are dropped until it has been taken.
    overflowed: bool,
    closed: bool,
}

/// What a watch's sink fills and its emitter empties. The watcher's thread must never wait on the
/// stream, so a slow reader costs events, not progress: past `capacity` the queue is emptied and
/// replaced by a single `Rescan`, which tells the client to read the folder again.
pub(crate) struct WatchQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
    capacity: usize,
}

impl WatchQueue {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(QueueState {
                events: VecDeque::new(),
                overflowed: false,
                closed: false,
            }),
            ready: Condvar::new(),
            capacity: capacity.max(1),
        }
    }

    pub(crate) fn push(&self, event: WatchEvent) {
        let mut state = locked(&self.state);
        if state.closed {
            return;
        }
        // A folder that is lost is news the rescan would only rediscover, so it is always kept.
        let lost = matches!(event, WatchEvent::Lost(_));
        if state.overflowed && !lost {
            return;
        }
        if !state.overflowed && state.events.len() >= self.capacity && !lost {
            state.events.clear();
            state
                .events
                .push_back(WatchEvent::Rescan(RescanReason::Overflow));
            state.overflowed = true;
        } else {
            state.events.push_back(event);
        }
        self.ready.notify_one();
    }

    /// Waits for the next event; `None` once the queue is closed.
    pub(crate) fn pop(&self) -> Option<WatchEvent> {
        let mut state = locked(&self.state);
        loop {
            if state.closed {
                return None;
            }
            if let Some(event) = state.events.pop_front() {
                // The first event after an overflow is the rescan itself.
                state.overflowed = false;
                return Some(event);
            }
            state = wait(&self.ready, state);
        }
    }

    pub(crate) fn close(&self) {
        let mut state = locked(&self.state);
        state.closed = true;
        state.events.clear();
        self.ready.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upsert(n: usize) -> WatchEvent {
        WatchEvent::Changes(vec![waypoint_vfs::Change::Remove(format!("{n}").into())])
    }

    #[test]
    fn events_come_out_in_order() {
        let queue = WatchQueue::new(4);
        queue.push(upsert(1));
        queue.push(upsert(2));
        assert_eq!(queue.pop(), Some(upsert(1)));
        assert_eq!(queue.pop(), Some(upsert(2)));
    }

    #[test]
    fn an_overflow_becomes_one_rescan() {
        let queue = WatchQueue::new(3);
        for n in 0..100 {
            queue.push(upsert(n));
        }
        assert_eq!(
            queue.pop(),
            Some(WatchEvent::Rescan(RescanReason::Overflow))
        );
        // Everything after the overflow was covered by the rescan, until it was taken.
        queue.push(upsert(100));
        assert_eq!(queue.pop(), Some(upsert(100)));
    }

    #[test]
    fn a_lost_folder_survives_an_overflow() {
        let queue = WatchQueue::new(1);
        queue.push(upsert(1));
        queue.push(upsert(2));
        queue.push(WatchEvent::Lost(waypoint_protocol::VfsError::StaleHandle));
        assert_eq!(
            queue.pop(),
            Some(WatchEvent::Rescan(RescanReason::Overflow))
        );
        assert!(matches!(queue.pop(), Some(WatchEvent::Lost(_))));
    }

    #[test]
    fn closing_wakes_a_waiting_emitter() {
        let queue = std::sync::Arc::new(WatchQueue::new(2));
        let waiting = {
            let queue = queue.clone();
            std::thread::spawn(move || queue.pop())
        };
        std::thread::sleep(std::time::Duration::from_millis(20));
        queue.close();
        assert_eq!(waiting.join().unwrap(), None);
        queue.push(upsert(1));
        assert_eq!(queue.pop(), None);
    }
}

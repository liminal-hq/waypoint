// One live stream to a helper, as the client sees it: the writer, the callers waiting for answers
// and the watches whose events arrive unasked.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;

use waypoint_path::FilePath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{WatchEvent, WatchSink};

use crate::frame::{read_frame, write_frame, Frame};
use crate::sync::locked;
use crate::wire::{EventBody, Message, Op, Reply, Request, WireWatchEvent};

/// What reaches a caller waiting on a request.
pub(crate) enum Incoming {
    Event(EventBody),
    Data(Vec<u8>),
    Reply(Reply),
    /// The connection ended before an answer came.
    Closed,
}

struct Pending {
    dead: bool,
    slots: HashMap<u64, Sender<Incoming>>,
}

/// A folder being watched: where its events go, and the folder, so an event that names a place
/// outside it can be refused.
struct WatchEntry {
    sink: WatchSink,
    folder: FilePath,
    location: Location,
}

pub(crate) struct Connection {
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    pending: Mutex<Pending>,
    watches: Mutex<HashMap<u64, WatchEntry>>,
    next_id: AtomicU64,
}

impl Connection {
    pub(crate) fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Mutex::new(Some(writer)),
            pending: Mutex::new(Pending {
                dead: false,
                slots: HashMap::new(),
            }),
            watches: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    pub(crate) fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn is_dead(&self) -> bool {
        locked(&self.pending).dead
    }

    /// Makes room for the answer to request `id`; `None` when the connection has ended.
    pub(crate) fn register(&self, id: u64) -> Option<Receiver<Incoming>> {
        let mut pending = locked(&self.pending);
        if pending.dead {
            return None;
        }
        let (sender, receiver) = mpsc::channel();
        pending.slots.insert(id, sender);
        Some(receiver)
    }

    pub(crate) fn forget(&self, id: u64) {
        locked(&self.pending).slots.remove(&id);
    }

    /// Writes the frames one after another under one lock, so a request and the data that goes with
    /// it stay together. A failed write ends the connection.
    fn send(&self, frames: &[Frame]) -> bool {
        let sent = {
            let mut writer = locked(&self.writer);
            match writer.as_mut() {
                Some(writer) => frames
                    .iter()
                    .all(|frame| write_frame(&mut *writer, frame).is_ok()),
                None => false,
            }
        };
        if !sent {
            self.shutdown();
        }
        sent
    }

    pub(crate) fn send_request(&self, id: u64, op: Op) -> bool {
        match Message::Request(Request { id, op }).to_frame() {
            Ok(frame) => self.send(&[frame]),
            Err(_) => false,
        }
    }

    /// A `Write` request and its data.
    pub(crate) fn send_write(&self, id: u64, handle: u64, bytes: Vec<u8>) -> bool {
        match Message::Request(Request {
            id,
            op: Op::Write { handle },
        })
        .to_frame()
        {
            Ok(request) => self.send(&[request, Frame::Data { id, bytes }]),
            Err(_) => false,
        }
    }

    /// Sends a request whose answer nobody waits for (a `Cancel`, an `Unwatch`, a `CloseHandle`);
    /// the answer, when it comes, finds no one and is dropped.
    pub(crate) fn send_and_forget(&self, op: Op) {
        let id = self.next_id();
        self.send_request(id, op);
    }

    pub(crate) fn add_watch(&self, id: u64, sink: WatchSink, folder: FilePath, location: Location) {
        locked(&self.watches).insert(
            id,
            WatchEntry {
                sink,
                folder,
                location,
            },
        );
    }

    /// Stops delivering a watch's events; whether it was still there.
    pub(crate) fn remove_watch(&self, id: u64) -> bool {
        locked(&self.watches).remove(&id).is_some()
    }

    /// Ends the connection: callers still waiting get `Closed`, every watch gets `Lost`, and the
    /// writer is dropped, which ends a helper that reads its input to the end. Only the first call
    /// does anything; it says whether this was it.
    pub(crate) fn shutdown(&self) -> bool {
        let slots = {
            let mut pending = locked(&self.pending);
            if pending.dead {
                return false;
            }
            pending.dead = true;
            std::mem::take(&mut pending.slots)
        };
        for sender in slots.into_values() {
            let _ = sender.send(Incoming::Closed);
        }
        let writer = locked(&self.writer).take();
        drop(writer);
        let watches = std::mem::take(&mut *locked(&self.watches));
        for entry in watches.into_values() {
            (entry.sink)(WatchEvent::Lost(VfsError::Disconnected {
                location: entry.location,
            }));
        }
        true
    }

    /// Reads the stream until it ends or says something it should not, handing each message to
    /// whoever waits for it. Returns when the connection is over.
    pub(crate) fn read_until_end(&self, mut reader: Box<dyn Read + Send>) {
        loop {
            match read_frame(&mut reader) {
                Ok(Some(Frame::Control(body))) => match Message::from_body(&body) {
                    Ok(Message::Response(response)) => {
                        self.deliver(response.id, Incoming::Reply(response.reply))
                    }
                    Ok(Message::Event(event)) => {
                        if !self.on_event(event.id, event.body) {
                            break;
                        }
                    }
                    // A request, or something that does not parse, from the helper: it cannot be
                    // trusted any more.
                    Ok(Message::Request(_)) | Err(_) => break,
                },
                Ok(Some(Frame::Data { id, bytes })) => self.deliver(id, Incoming::Data(bytes)),
                Ok(None) | Err(_) => break,
            }
        }
        self.shutdown();
    }

    /// Delivers a final answer, which frees the request's place.
    fn deliver(&self, id: u64, incoming: Incoming) {
        let sender = locked(&self.pending).slots.remove(&id);
        if let Some(sender) = sender {
            let _ = sender.send(incoming);
        }
    }

    /// Routes an event to the watch or the request it belongs to; `false` when it is one the
    /// client must not accept.
    fn on_event(&self, id: u64, body: EventBody) -> bool {
        match body {
            EventBody::Watch { event } => self.on_watch_event(id, event),
            other => {
                let sender = locked(&self.pending).slots.get(&id).cloned();
                if let Some(sender) = sender {
                    let _ = sender.send(Incoming::Event(other));
                }
                true
            }
        }
    }

    fn on_watch_event(&self, id: u64, event: WireWatchEvent) -> bool {
        let found = locked(&self.watches)
            .get(&id)
            .map(|entry| (entry.sink.clone(), entry.folder.clone()));
        // An event for a watch that has been dropped is expected: it was already on its way.
        let Some((sink, folder)) = found else {
            return true;
        };
        if let WireWatchEvent::Lost { error } = &event {
            if !names_inside(error, &folder) {
                return false;
            }
        }
        let Ok(event) = WatchEvent::try_from(event) else {
            return false;
        };
        let event = match event {
            WatchEvent::Lost(error) => WatchEvent::Lost(crate::paths::rewrite_error(error)),
            other => other,
        };
        sink(event);
        true
    }
}

/// Whether every place an error names is the watched folder or inside it.
fn names_inside(error: &VfsError, folder: &FilePath) -> bool {
    let mut inside = true;
    crate::paths::for_each_location(&mut error.clone(), &mut |location| {
        let named = FilePath::from_uri(&location.uri);
        if !named.is_ok_and(|named| named.as_path().starts_with(folder.as_path())) {
            inside = false;
        }
    });
    inside
}

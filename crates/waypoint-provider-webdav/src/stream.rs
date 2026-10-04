// Handing a response body from the runtime to a caller's thread as `Read`: a task on the provider's
// runtime pumps chunks into a small channel, and the reader blocks on it, so a caller never needs to be
// inside the runtime and a slow reader holds back the network instead of memory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::mpsc;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, InjectedError};

use crate::client::{cancelled, Reply};
use crate::errors::from_transport;

/// Chunks waiting between the network and the reader.
const QUEUE: usize = 8;

pub(crate) type Chunk = Result<Bytes, VfsError>;

/// The reading end. A failure is kept as the typed error it was, for whoever is reading to ask for
/// after the read fails (`take_error`).
pub(crate) struct ChannelReader {
    chunks: mpsc::Receiver<Chunk>,
    current: Bytes,
    failed: Arc<Mutex<Option<VfsError>>>,
    done: bool,
}

/// Asks a reader for the typed error that ended it.
#[derive(Clone)]
pub(crate) struct Failure(Arc<Mutex<Option<VfsError>>>);

impl Failure {
    pub(crate) fn take(&self) -> Option<VfsError> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

impl ChannelReader {
    pub(crate) fn new(chunks: mpsc::Receiver<Chunk>) -> (Self, Failure) {
        let failed = Arc::new(Mutex::new(None));
        (
            Self {
                chunks,
                current: Bytes::new(),
                failed: failed.clone(),
                done: false,
            },
            Failure(failed),
        )
    }

    pub(crate) fn channel() -> (mpsc::Sender<Chunk>, mpsc::Receiver<Chunk>) {
        mpsc::channel(QUEUE)
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.current.is_empty() {
            if self.done || buf.is_empty() {
                return Ok(0);
            }
            // The reader runs on a caller's thread, never on the runtime, so it may block.
            match futures::executor::block_on(self.chunks.recv()) {
                Some(Ok(chunk)) => self.current = chunk,
                Some(Err(error)) => {
                    self.done = true;
                    *self.failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(error.clone());
                    return Err(InjectedError(error).into_io());
                }
                None => {
                    self.done = true;
                    return Ok(0);
                }
            }
        }
        let n = buf.len().min(self.current.len());
        buf[..n].copy_from_slice(&self.current[..n]);
        self.current = self.current.slice(n..);
        Ok(n)
    }
}

/// Reads `reply`'s body to its end into `send`, a chunk at a time. It stops when the reader is
/// dropped, when `cancel` is set and when no chunk arrives within `timeout`. The reply's place in
/// the connection's queue is held until then.
pub(crate) async fn pump(
    reply: Reply,
    cancel: Option<CancelToken>,
    timeout: Duration,
    location: Location,
    send: mpsc::Sender<Chunk>,
) {
    let mut response = reply.response;
    let _permit = reply._permit;
    loop {
        let waiting = tokio::time::timeout(timeout, response.chunk());
        let next = match &cancel {
            Some(cancel) => tokio::select! {
                next = waiting => next,
                () = cancelled(cancel) => {
                    let _ = send.send(Err(VfsError::Cancelled)).await;
                    return;
                }
            },
            None => waiting.await,
        };
        let item = match next {
            Err(_) => Err(VfsError::Timeout {
                location: location.clone(),
            }),
            Ok(Ok(Some(chunk))) => Ok(chunk),
            Ok(Ok(None)) => return,
            Ok(Err(error)) => Err(from_transport(&error, &location)),
        };
        let failed = item.is_err();
        if send.send(item).await.is_err() || failed {
            return;
        }
    }
}

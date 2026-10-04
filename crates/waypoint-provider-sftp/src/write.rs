// Writing a file as a stream, with a window of write requests in flight behind the writer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::{mpsc, oneshot};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{InjectedError, WriteStream};

use crate::errors::from_sftp;
use crate::session::Session;

enum Message {
    Data(u64, Vec<u8>),
    Finish {
        sync: bool,
        done: oneshot::Sender<Result<(), VfsError>>,
    },
}

/// The writer's end: chunks go to a task on the provider's runtime that keeps the session's write
/// window full. The first failure is kept and returned by the next write or by `finish`.
pub(crate) struct SftpWriter {
    send: mpsc::Sender<Message>,
    buffer: Vec<u8>,
    offset: u64,
    chunk: usize,
    failed: Arc<Mutex<Option<VfsError>>>,
    location: Location,
}

impl SftpWriter {
    /// Starts writing the open file `handle` at `offset`.
    pub(crate) fn start(
        runtime: &tokio::runtime::Handle,
        session: Arc<Session>,
        handle: String,
        offset: u64,
        location: Location,
    ) -> Self {
        let window = session.options.write_requests.max(1);
        let chunk = session.options.chunk.max(512) as usize;
        let (send, receive) = mpsc::channel(window);
        let failed = Arc::new(Mutex::new(None));
        runtime.spawn(pump(
            session,
            handle,
            location.clone(),
            receive,
            failed.clone(),
        ));
        Self {
            send,
            buffer: Vec::with_capacity(chunk),
            offset,
            chunk,
            failed,
            location,
        }
    }

    fn failure(&self) -> Option<VfsError> {
        self.failed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Sends the buffered bytes as one write request.
    fn dispatch(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let data = std::mem::replace(&mut self.buffer, Vec::with_capacity(self.chunk));
        let length = data.len() as u64;
        // The writer runs on a caller's thread, never on the runtime, so it may block.
        futures::executor::block_on(self.send.send(Message::Data(self.offset, data)))
            .map_err(|_| self.lost())?;
        self.offset += length;
        Ok(())
    }

    fn lost(&self) -> io::Error {
        let error = self.failure().unwrap_or(VfsError::Io {
            message: "the write stopped".to_owned(),
            location: None,
        });
        InjectedError(error).into_io()
    }
}

impl Write for SftpWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(error) = self.failure() {
            return Err(InjectedError(error).into_io());
        }
        let room = self.chunk - self.buffer.len();
        let n = room.min(buf.len());
        self.buffer.extend_from_slice(&buf[..n]);
        if self.buffer.len() == self.chunk {
            self.dispatch()?;
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.dispatch()
    }
}

impl WriteStream for SftpWriter {
    fn finish(mut self: Box<Self>, sync: bool) -> Result<(), VfsError> {
        self.dispatch()
            .map_err(|error| waypoint_vfs::from_io(&error, &self.location))?;
        let (done, result) = oneshot::channel();
        futures::executor::block_on(self.send.send(Message::Finish { sync, done }))
            .map_err(|_| self.failure().unwrap_or(VfsError::Cancelled))?;
        futures::executor::block_on(result).unwrap_or_else(|_| {
            Err(self.failure().unwrap_or(VfsError::Io {
                message: "the write stopped".to_owned(),
                location: None,
            }))
        })
    }
}

/// Writes what arrives, keeping up to the session's window in flight; on `Finish` waits for every
/// reply, commits the data when asked and the server can, and closes the file. A writer dropped
/// without finishing still has the file closed.
async fn pump(
    session: Arc<Session>,
    handle: String,
    location: Location,
    mut receive: mpsc::Receiver<Message>,
    failed: Arc<Mutex<Option<VfsError>>>,
) {
    let sftp = &session.sftp;
    let window = session.options.write_requests.max(1);
    let record = |error: VfsError| {
        let mut failed = failed.lock().unwrap_or_else(|e| e.into_inner());
        failed.get_or_insert(error);
    };
    let mut in_flight = FuturesUnordered::new();
    let mut finish = None;
    let mut open = true;
    while open || !in_flight.is_empty() {
        if open && in_flight.len() < window {
            tokio::select! {
                message = receive.recv() => match message {
                    Some(Message::Data(offset, data)) => {
                        in_flight.push(sftp.write(handle.clone(), offset, data));
                    }
                    Some(Message::Finish { sync, done }) => {
                        finish = Some((sync, done));
                        open = false;
                    }
                    None => open = false,
                },
                Some(reply) = in_flight.next(), if !in_flight.is_empty() => {
                    if let Err(error) = reply {
                        record(from_sftp(&error, &location));
                    }
                }
            }
        } else if let Some(Err(error)) = in_flight.next().await {
            record(from_sftp(&error, &location));
        }
    }
    let mut result = match failed.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        Some(error) => Err(error),
        None => Ok(()),
    };
    if let Some((true, _)) = &finish {
        if result.is_ok() && session.extensions.fsync {
            if let Err(error) = sftp.fsync(handle.clone()).await {
                result = Err(from_sftp(&error, &location));
            }
        }
    }
    let closed = sftp.close(handle).await;
    if let (Ok(()), Err(error)) = (&result, closed) {
        result = Err(from_sftp(&error, &location));
    }
    if let Some((_, done)) = finish {
        let _ = done.send(result);
    }
}

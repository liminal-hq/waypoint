// Reading a file as a stream, with a window of read requests in flight ahead of the reader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read};
use std::sync::Arc;

use futures::stream::{FuturesOrdered, StreamExt};
use tokio::sync::mpsc;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::InjectedError;

use crate::errors::{from_sftp, is_eof};
use crate::session::Session;

/// The reader's end: chunks arrive in order from a task on the provider's runtime. Dropping it
/// stops the task, which closes the file on the server.
pub(crate) struct SftpReader {
    chunks: mpsc::Receiver<Result<Vec<u8>, VfsError>>,
    current: Vec<u8>,
    at: usize,
    done: bool,
}

impl SftpReader {
    /// Starts reading the open file `handle` from `start`.
    pub(crate) fn start(
        runtime: &tokio::runtime::Handle,
        session: Arc<Session>,
        handle: String,
        start: u64,
        location: Location,
    ) -> Self {
        let in_flight = session.options.read_requests.max(1);
        let (send, chunks) = mpsc::channel(in_flight);
        runtime.spawn(async move {
            pump(&session, &handle, start, &location, &send).await;
            let _ = session.sftp.close(handle).await;
        });
        Self {
            chunks,
            current: Vec::new(),
            at: 0,
            done: false,
        }
    }
}

impl Read for SftpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.at == self.current.len() {
            if self.done || buf.is_empty() {
                return Ok(0);
            }
            // The reader runs on a caller's thread, never on the runtime, so it may block.
            match futures::executor::block_on(self.chunks.recv()) {
                Some(Ok(chunk)) => {
                    self.current = chunk;
                    self.at = 0;
                }
                Some(Err(error)) => {
                    self.done = true;
                    return Err(InjectedError(error).into_io());
                }
                None => self.done = true,
            }
        }
        let n = buf.len().min(self.current.len() - self.at);
        buf[..n].copy_from_slice(&self.current[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// Reads from `start` to the end, keeping the session's read window full, and sends each chunk
/// in order. A server may answer a read with fewer bytes than asked before the end; the rest of
/// that chunk is asked for again before going on.
async fn pump(
    session: &Session,
    handle: &str,
    start: u64,
    location: &Location,
    send: &mpsc::Sender<Result<Vec<u8>, VfsError>>,
) {
    let sftp = &session.sftp;
    let chunk = session.options.chunk.max(512);
    let window = session.options.read_requests.max(1);
    let mut next = start;
    let mut eof = false;
    let mut in_flight = FuturesOrdered::new();
    let request =
        |offset: u64| async move { (offset, sftp.read(handle.to_owned(), offset, chunk).await) };
    while in_flight.len() < window {
        in_flight.push_back(request(next));
        next += u64::from(chunk);
    }
    while let Some((offset, reply)) = in_flight.next().await {
        if eof {
            continue;
        }
        let mut data = match reply {
            Ok(data) => data.data,
            Err(error) if is_eof(&error) => {
                eof = true;
                continue;
            }
            Err(error) => {
                let _ = send.send(Err(from_sftp(&error, location))).await;
                return;
            }
        };
        // Fill a short reply up to the chunk, or find the end.
        while (data.len() as u64) < u64::from(chunk) {
            let at = offset + data.len() as u64;
            let want = chunk - data.len() as u32;
            match sftp.read(handle.to_owned(), at, want).await {
                Ok(more) if more.data.is_empty() => {
                    eof = true;
                    break;
                }
                Ok(more) => data.extend_from_slice(&more.data),
                Err(error) if is_eof(&error) => {
                    eof = true;
                    break;
                }
                Err(error) => {
                    let _ = send.send(Err(from_sftp(&error, location))).await;
                    return;
                }
            }
        }
        if !data.is_empty() && send.send(Ok(data)).await.is_err() {
            // The reader was dropped.
            return;
        }
        if !eof {
            in_flight.push_back(request(next));
            next += u64::from(chunk);
        }
    }
}

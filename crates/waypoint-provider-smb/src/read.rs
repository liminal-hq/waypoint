// Reading a file as a stream, with a window of read requests in flight ahead of the reader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read};
use std::sync::Arc;

use futures::stream::{FuturesOrdered, StreamExt};
use smb2::FileReader;
use tokio::sync::mpsc;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::InjectedError;

use crate::errors::from_smb2;

/// The reader's end: chunks arrive in order from a task on the provider's runtime. Dropping it
/// stops the task, which closes the file on the server.
pub(crate) struct SmbReader {
    chunks: mpsc::Receiver<Result<Vec<u8>, VfsError>>,
    current: Vec<u8>,
    at: usize,
    done: bool,
}

impl SmbReader {
    /// Starts reading the open `file` from `start`, `window` requests of `chunk` bytes at a time.
    pub(crate) fn start(
        runtime: &tokio::runtime::Handle,
        file: FileReader,
        start: u64,
        window: usize,
        chunk: usize,
        location: Location,
    ) -> Self {
        let window = window.max(1);
        let (send, chunks) = mpsc::channel(window);
        runtime.spawn(async move {
            let file = Arc::new(file);
            pump(&file, start, window, chunk.max(4096), &location, &send).await;
            // The handle is released on the server whether the file was read to its end or the
            // reader was dropped.
            if let Some(file) = Arc::into_inner(file) {
                let _ = file.close().await;
            }
        });
        Self {
            chunks,
            current: Vec::new(),
            at: 0,
            done: false,
        }
    }
}

impl Read for SmbReader {
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

/// Reads from `start` to the end of the file as it was when opened, keeping `window` requests in
/// flight, and sends each chunk in order.
async fn pump(
    file: &Arc<FileReader>,
    start: u64,
    window: usize,
    chunk: usize,
    location: &Location,
    send: &mpsc::Sender<Result<Vec<u8>, VfsError>>,
) {
    let size = file.size();
    let mut next = start;
    let mut in_flight = FuturesOrdered::new();
    let request = |offset: u64| {
        let file = file.clone();
        async move { file.read_at(offset, chunk as u64).await }
    };
    while in_flight.len() < window && next < size {
        in_flight.push_back(request(next));
        next += chunk as u64;
    }
    while let Some(reply) = in_flight.next().await {
        match reply {
            Ok(data) if data.is_empty() => return,
            Ok(data) => {
                if send.send(Ok(data)).await.is_err() {
                    // The reader was dropped.
                    return;
                }
            }
            Err(error) => {
                let _ = send.send(Err(from_smb2(&error, location))).await;
                return;
            }
        }
        if next < size {
            in_flight.push_back(request(next));
            next += chunk as u64;
        }
    }
}

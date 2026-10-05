// Writing a file as a stream: the library's writer keeps a window of write requests in flight
// behind the bytes handed to it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Write};

use smb2::FileWriter;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{InjectedError, WriteStream};

use crate::errors::from_smb2;

/// The writer's end. Bytes collect into chunks, and each full chunk is handed to the library's
/// writer, which sends it while the next one fills.
pub(crate) struct SmbWriter {
    runtime: tokio::runtime::Handle,
    writer: Option<FileWriter>,
    buffer: Vec<u8>,
    chunk: usize,
    location: Location,
}

impl SmbWriter {
    pub(crate) fn new(
        runtime: tokio::runtime::Handle,
        writer: FileWriter,
        chunk: usize,
        location: Location,
    ) -> Self {
        let chunk = chunk.max(4096);
        Self {
            runtime,
            writer: Some(writer),
            buffer: Vec::with_capacity(chunk),
            chunk,
            location,
        }
    }

    /// Hands the buffered bytes to the library's writer.
    fn dispatch(&mut self) -> Result<(), VfsError> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let Some(writer) = self.writer.as_mut() else {
            return Err(VfsError::Io {
                message: "the write already ended".to_owned(),
                location: Some(self.location.clone()),
            });
        };
        // The writer runs on a caller's thread, never on the runtime, so it may block.
        let result = self.runtime.block_on(writer.write_chunk(&self.buffer));
        self.buffer.clear();
        result.map_err(|error| from_smb2(&error, &self.location))
    }
}

impl Write for SmbWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let room = self.chunk - self.buffer.len();
        let n = room.min(buf.len());
        self.buffer.extend_from_slice(&buf[..n]);
        if self.buffer.len() >= self.chunk {
            self.dispatch().map_err(|e| InjectedError(e).into_io())?;
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.dispatch().map_err(|e| InjectedError(e).into_io())
    }
}

impl WriteStream for SmbWriter {
    /// The library flushes the file to the server's storage before it closes it, so `sync` changes
    /// nothing here.
    fn finish(mut self: Box<Self>, _sync: bool) -> Result<(), VfsError> {
        self.dispatch()?;
        let Some(writer) = self.writer.take() else {
            return Ok(());
        };
        self.runtime
            .block_on(writer.finish())
            .map(|_| ())
            .map_err(|error| from_smb2(&error, &self.location))
    }
}

impl Drop for SmbWriter {
    fn drop(&mut self) {
        // A writer dropped without `finish` still has its handle closed, without a flush.
        if let Some(writer) = self.writer.take() {
            self.runtime.spawn(async move {
                let _ = writer.abort().await;
            });
        }
    }
}

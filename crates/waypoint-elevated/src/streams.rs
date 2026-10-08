// The streams a file is read and written through: each call is one request to the helper.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read, Write};
use std::sync::Arc;

use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{InjectedError, WriteStream};

use crate::client::{Answer, Inner, Outgoing};
use crate::connection::Connection;
use crate::frame::MAX_CHUNK;
use crate::wire::{Op, Reply};

fn to_io(error: VfsError) -> io::Error {
    // `from_io` hands the typed error back out of this.
    InjectedError(error).into_io()
}

/// A file read through the helper, a chunk at a time.
pub(crate) struct RemoteRead {
    inner: Arc<Inner>,
    conn: Arc<Connection>,
    handle: u64,
    location: Location,
    chunk: Vec<u8>,
    at: usize,
    ended: bool,
}

impl RemoteRead {
    pub(crate) fn new(
        inner: Arc<Inner>,
        conn: Arc<Connection>,
        handle: u64,
        location: Location,
    ) -> Self {
        Self {
            inner,
            conn,
            handle,
            location,
            chunk: Vec::new(),
            at: 0,
            ended: false,
        }
    }

    fn fetch(&mut self) -> Result<(), VfsError> {
        let op = Op::Read {
            handle: self.handle,
            len: MAX_CHUNK as u32,
        };
        match self.inner.exchange(
            &self.conn,
            &self.location,
            Outgoing::Op(op),
            None,
            &mut |_| false,
        )? {
            Answer::Data(bytes) => {
                self.ended = bytes.is_empty();
                self.chunk = bytes;
                self.at = 0;
                Ok(())
            }
            Answer::Reply(_) => Err(self.inner.fault(&self.conn, &self.location)),
        }
    }
}

impl Read for RemoteRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.at >= self.chunk.len() {
            if self.ended {
                return Ok(0);
            }
            self.fetch().map_err(to_io)?;
            if self.chunk.is_empty() {
                return Ok(0);
            }
        }
        let n = buf.len().min(self.chunk.len() - self.at);
        buf[..n].copy_from_slice(&self.chunk[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

impl Drop for RemoteRead {
    fn drop(&mut self) {
        self.conn.send_and_forget(Op::CloseHandle {
            handle: self.handle,
        });
    }
}

/// A file written through the helper. Bytes gather into chunks; each chunk is one request that is
/// answered before the next goes out, so the helper writes them in order.
pub(crate) struct RemoteWrite {
    inner: Arc<Inner>,
    conn: Arc<Connection>,
    handle: u64,
    location: Location,
    buffer: Vec<u8>,
    done: bool,
}

impl RemoteWrite {
    pub(crate) fn new(
        inner: Arc<Inner>,
        conn: Arc<Connection>,
        handle: u64,
        location: Location,
    ) -> Self {
        Self {
            inner,
            conn,
            handle,
            location,
            buffer: Vec::new(),
            done: false,
        }
    }

    fn ask(&self, outgoing: Outgoing) -> Result<(), VfsError> {
        match self
            .inner
            .exchange(&self.conn, &self.location, outgoing, None, &mut |_| false)?
        {
            Answer::Reply(Reply::Unit) => Ok(()),
            _ => Err(self.inner.fault(&self.conn, &self.location)),
        }
    }

    fn send(&mut self, len: usize) -> Result<(), VfsError> {
        let bytes: Vec<u8> = self.buffer.drain(..len).collect();
        self.ask(Outgoing::Write {
            handle: self.handle,
            bytes,
        })
    }

    fn send_all(&mut self) -> Result<(), VfsError> {
        while !self.buffer.is_empty() {
            self.send(self.buffer.len().min(MAX_CHUNK))?;
        }
        Ok(())
    }
}

impl Write for RemoteWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        while self.buffer.len() >= MAX_CHUNK {
            self.send(MAX_CHUNK).map_err(to_io)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.send_all().map_err(to_io)
    }
}

impl WriteStream for RemoteWrite {
    fn finish(mut self: Box<Self>, sync: bool) -> Result<(), VfsError> {
        self.send_all()?;
        self.done = true;
        self.ask(Outgoing::Op(Op::FinishWrite {
            handle: self.handle,
            sync,
        }))
    }
}

impl Drop for RemoteWrite {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        // Dropped without `finish`: what was written reaches the file, then it is closed without
        // reporting, as a local file is.
        let _ = self.send_all();
        self.conn.send_and_forget(Op::CloseHandle {
            handle: self.handle,
        });
    }
}

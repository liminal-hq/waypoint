// An in-memory duplex byte stream that behaves like a pair of OS pipes, on every platform.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::{Arc, Condvar, Mutex};

use crate::sync::{locked, wait};

/// How much a pipe holds before a writer waits for the reader, as an OS pipe does.
const CAPACITY: usize = 64 * 1024;

struct State {
    bytes: VecDeque<u8>,
    /// The writer is gone: the reader drains what is left, then reaches the end.
    writer_gone: bool,
    /// The reader is gone, or the pipe was aborted: writes fail and reads end at once.
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

/// Cuts a pipe from outside, as a process dying does: both ends fail from then on.
#[derive(Clone)]
pub struct PipeCloser(Arc<Shared>);

impl PipeCloser {
    pub fn abort(&self) {
        let mut state = locked(&self.0.state);
        state.closed = true;
        state.bytes.clear();
        self.0.changed.notify_all();
    }
}

pub struct PipeReader(Arc<Shared>);

pub struct PipeWriter(Arc<Shared>);

/// One direction: bytes written to the writer come out of the reader.
pub fn pipe() -> (PipeReader, PipeWriter, PipeCloser) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            bytes: VecDeque::new(),
            writer_gone: false,
            closed: false,
        }),
        changed: Condvar::new(),
    });
    (
        PipeReader(shared.clone()),
        PipeWriter(shared.clone()),
        PipeCloser(shared),
    )
}

impl Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut state = locked(&self.0.state);
        loop {
            if state.closed {
                return Ok(0);
            }
            if !state.bytes.is_empty() {
                let n = buf.len().min(state.bytes.len());
                for slot in buf.iter_mut().take(n) {
                    *slot = state.bytes.pop_front().unwrap_or_default();
                }
                self.0.changed.notify_all();
                return Ok(n);
            }
            if state.writer_gone {
                return Ok(0);
            }
            state = wait(&self.0.changed, state);
        }
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        let mut state = locked(&self.0.state);
        state.closed = true;
        state.bytes.clear();
        self.0.changed.notify_all();
    }
}

impl Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut state = locked(&self.0.state);
        loop {
            if state.closed {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            let room = CAPACITY.saturating_sub(state.bytes.len());
            if room > 0 {
                let n = room.min(buf.len());
                state.bytes.extend(&buf[..n]);
                self.0.changed.notify_all();
                return Ok(n);
            }
            state = wait(&self.0.changed, state);
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        let mut state = locked(&self.0.state);
        state.writer_gone = true;
        self.0.changed.notify_all();
    }
}

/// Both ends of a duplex stream, and what cuts it.
pub struct Duplex {
    pub client_reader: PipeReader,
    pub client_writer: PipeWriter,
    pub server_reader: PipeReader,
    pub server_writer: PipeWriter,
    pub cutter: Cutter,
}

/// Cuts both directions.
#[derive(Clone)]
pub struct Cutter(PipeCloser, PipeCloser);

impl Cutter {
    pub fn cut(&self) {
        self.0.abort();
        self.1.abort();
    }
}

/// A pair of pipes joined to make a connection: the client's writer feeds the server's reader and
/// the other way round.
pub fn duplex() -> Duplex {
    let (server_reader, client_writer, to_server) = pipe();
    let (client_reader, server_writer, to_client) = pipe();
    Duplex {
        client_reader,
        client_writer,
        server_reader,
        server_writer,
        cutter: Cutter(to_server, to_client),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_come_out_in_order_and_the_end_follows_the_writer() {
        let (mut reader, mut writer, _) = pipe();
        writer.write_all(b"hello ").unwrap();
        writer.write_all(b"world").unwrap();
        drop(writer);
        let mut out = String::new();
        reader.read_to_string(&mut out).unwrap();
        assert_eq!(out, "hello world");
    }

    #[test]
    fn a_full_pipe_makes_the_writer_wait_for_the_reader() {
        let (mut reader, mut writer, _) = pipe();
        let big = vec![7u8; CAPACITY * 3];
        let sender = std::thread::spawn(move || writer.write_all(&big).map(|_| ()));
        let mut got = 0;
        let mut buf = [0u8; 4096];
        while got < CAPACITY * 3 {
            got += reader.read(&mut buf).unwrap();
        }
        sender.join().unwrap().unwrap();
    }

    #[test]
    fn writing_to_a_dropped_reader_is_a_broken_pipe() {
        let (reader, mut writer, _) = pipe();
        drop(reader);
        let error = writer.write_all(b"x").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn aborting_ends_a_blocked_reader() {
        let (mut reader, _writer, closer) = pipe();
        let waiting = std::thread::spawn(move || reader.read(&mut [0u8; 8]).unwrap());
        std::thread::sleep(std::time::Duration::from_millis(20));
        closer.abort();
        assert_eq!(waiting.join().unwrap(), 0);
    }
}

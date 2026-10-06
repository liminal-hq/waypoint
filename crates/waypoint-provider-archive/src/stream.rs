// Reading one entry on a thread of its own, so a decoder that borrows its archive can still hand
// out a stream that owns everything it needs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::Duration;

use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, InjectedError, ReadStream};

const CHUNK: usize = 64 * 1024;
/// Chunks in flight between the decoder and the reader.
const WINDOW: usize = 8;

/// What a reading thread uses to report back.
pub(crate) struct Pump {
    /// The archive file, named by the `Corrupt` error a failing decoder becomes.
    location: Location,
    started: Option<SyncSender<Result<(), VfsError>>>,
    data: SyncSender<io::Result<Vec<u8>>>,
}

impl Pump {
    /// What the reader sees for a read failure: a decoder that choked or ran out of data means the
    /// archive is damaged, said with the typed error so it survives the `io::Error` it travels in.
    fn typed(&self, error: io::Error) -> io::Error {
        if crate::errors::is_bad_data(&error) {
            log::debug!("archive: {} does not decode: {error}", self.location.uri);
            InjectedError(crate::errors::corrupt(&self.location)).into_io()
        } else {
            error
        }
    }

    /// The entry is open: `open_read` may return its stream.
    pub fn ready(&mut self) {
        if let Some(started) = self.started.take() {
            let _ = started.send(Ok(()));
        }
    }

    /// The entry cannot be opened: `open_read` returns this error.
    pub fn fail(&mut self, error: VfsError) {
        if let Some(started) = self.started.take() {
            let _ = started.send(Err(error));
        }
    }

    /// Sends what `source` yields, after discarding `skip` bytes and up to `limit` bytes, until the
    /// end or until the reader is dropped. A read error reaches the reader as an error, never as a
    /// short file, and a source that ends before `limit` is an error too.
    pub fn copy_from(
        &mut self,
        source: &mut dyn Read,
        skip: u64,
        limit: Option<u64>,
        cancel: Option<&CancelToken>,
    ) {
        if skip > 0 {
            match io::copy(&mut source.take(skip), &mut io::sink()) {
                Ok(skipped) if skipped < skip => {
                    let error = self.typed(too_short());
                    let _ = self.data.send(Err(error));
                    return;
                }
                Ok(_) => {}
                Err(error) => {
                    let error = self.typed(error);
                    let _ = self.data.send(Err(error));
                    return;
                }
            }
        }
        let mut remaining = limit.unwrap_or(u64::MAX);
        loop {
            if remaining == 0 {
                return;
            }
            if cancel.is_some_and(CancelToken::is_cancelled) {
                let _ = self
                    .data
                    .send(Err(InjectedError(VfsError::Cancelled).into_io()));
                return;
            }
            let want = CHUNK.min(usize::try_from(remaining).unwrap_or(CHUNK));
            let mut chunk = vec![0u8; want];
            let mut filled = 0;
            while filled < want {
                match source.read(&mut chunk[filled..]) {
                    Ok(0) => break,
                    Ok(read) => filled += read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => {
                        if filled > 0 {
                            chunk.truncate(filled);
                            let _ = self.data.send(Ok(chunk));
                        }
                        let error = self.typed(error);
                        let _ = self.data.send(Err(error));
                        return;
                    }
                }
            }
            let ended = filled < want;
            if filled > 0 {
                chunk.truncate(filled);
                remaining -= filled as u64;
                if self.data.send(Ok(chunk)).is_err() {
                    return;
                }
            }
            if ended {
                if limit.is_some() && remaining > 0 {
                    let error = self.typed(too_short());
                    let _ = self.data.send(Err(error));
                }
                return;
            }
        }
    }
}

fn too_short() -> io::Error {
    io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "the archive ends inside this entry",
    )
}

/// The stream `open_read` hands out.
pub(crate) struct ChannelReader {
    rx: Receiver<io::Result<Vec<u8>>>,
    chunk: Vec<u8>,
    at: usize,
    done: bool,
    /// The thread that decodes into `rx`; dropping the stream waits for it, so the archive file it
    /// has open is closed by the time the drop returns (Windows will not replace an open file).
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for ChannelReader {
    fn drop(&mut self) {
        // Hang up first so a decoder waiting to send stops, then wait for it to let go of the file.
        self.rx = sync_channel(0).1;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.at >= self.chunk.len() {
            if self.done {
                return Ok(0);
            }
            match self.rx.recv() {
                Ok(Ok(chunk)) => {
                    self.chunk = chunk;
                    self.at = 0;
                }
                Ok(Err(error)) => {
                    self.done = true;
                    return Err(error);
                }
                Err(_) => {
                    self.done = true;
                    return Ok(0);
                }
            }
        }
        let n = buf.len().min(self.chunk.len() - self.at);
        buf[..n].copy_from_slice(&self.chunk[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// Runs `job` on a new thread and waits for it to say whether the entry opened. The thread ends
/// when the entry has been sent or the returned stream is dropped.
pub(crate) fn spawn(
    location: &Location,
    job: impl FnOnce(&mut Pump) + Send + 'static,
) -> Result<ReadStream, VfsError> {
    let (started_tx, started_rx) = sync_channel(1);
    let (data_tx, data_rx) = sync_channel(WINDOW);
    let thread_location = location.clone();
    let builder = thread::Builder::new().name("waypoint-archive-read".to_owned());
    let handle = builder
        .spawn(move || {
            let mut pump = Pump {
                location: thread_location,
                started: Some(started_tx),
                data: data_tx,
            };
            job(&mut pump);
            // A job that returns without saying anything failed in a way it could not describe.
            pump.fail(VfsError::Io {
                message: "the archive reader stopped before opening the entry".to_owned(),
                location: None,
            });
        })
        .map_err(|error| waypoint_vfs::from_io(&error, location))?;
    // A job may take long to open (a solid archive decodes up to its entry), so wait in slices.
    loop {
        match started_rx.recv_timeout(Duration::from_secs(3600)) {
            Ok(Ok(())) => break,
            Ok(Err(error)) => return Err(error),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err(VfsError::Io {
                    message: "the archive reader stopped before opening the entry".to_owned(),
                    location: Some(location.clone()),
                })
            }
        }
    }
    Ok(Box::new(ChannelReader {
        rx: data_rx,
        chunk: Vec::new(),
        at: 0,
        done: false,
        thread: Some(handle),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location() -> Location {
        Location::new("a.zip", "file:///a.zip")
    }

    #[test]
    fn a_stream_arrives_whole() {
        let bytes: Vec<u8> = (0..300_000u32).map(|n| n as u8).collect();
        let expected = bytes.clone();
        let mut stream = spawn(&location(), move |pump| {
            pump.ready();
            pump.copy_from(&mut io::Cursor::new(bytes), 0, None, None);
        })
        .unwrap();
        let mut out = Vec::new();
        stream.read_to_end(&mut out).unwrap();
        assert_eq!(out, expected);
    }

    #[test]
    fn skipping_discards_the_start() {
        let mut stream = spawn(&location(), |pump| {
            pump.ready();
            pump.copy_from(&mut io::Cursor::new(b"0123456789".to_vec()), 4, None, None);
        })
        .unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        assert_eq!(out, "456789");
    }

    #[test]
    fn an_open_failure_is_the_error_of_the_call() {
        let error = spawn(&location(), |pump| {
            pump.fail(VfsError::Cancelled);
        })
        .err()
        .unwrap();
        assert_eq!(error, VfsError::Cancelled);
    }

    #[test]
    fn a_read_failure_is_an_error_not_a_short_file() {
        struct Breaks(usize);
        impl Read for Breaks {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.0 == 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "bad block"));
                }
                self.0 -= 1;
                buf[0] = 1;
                Ok(1)
            }
        }
        let mut stream = spawn(&location(), |pump| {
            pump.ready();
            pump.copy_from(&mut Breaks(5), 0, None, None);
        })
        .unwrap();
        let mut out = Vec::new();
        let error = stream.read_to_end(&mut out).unwrap_err();
        assert_eq!(
            waypoint_vfs::from_io(&error, &location()),
            VfsError::Corrupt {
                location: location()
            }
        );
    }

    #[test]
    fn a_limit_cuts_the_stream_and_a_short_source_is_an_error() {
        let mut stream = spawn(&location(), |pump| {
            pump.ready();
            pump.copy_from(
                &mut io::Cursor::new(b"0123456789".to_vec()),
                2,
                Some(5),
                None,
            );
        })
        .unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        assert_eq!(out, "23456");
        let mut stream = spawn(&location(), |pump| {
            pump.ready();
            pump.copy_from(&mut io::Cursor::new(b"0123".to_vec()), 0, Some(9), None);
        })
        .unwrap();
        let mut out = Vec::new();
        let error = stream.read_to_end(&mut out).unwrap_err();
        assert_eq!(
            waypoint_vfs::from_io(&error, &location()),
            VfsError::Corrupt {
                location: location()
            }
        );
        assert_eq!(out, b"0123");
    }

    #[test]
    fn dropping_the_stream_stops_the_thread() {
        let (done_tx, done_rx) = sync_channel(1);
        let stream = spawn(&location(), move |pump| {
            pump.ready();
            // Endless data: the thread can only end because the reader went away.
            pump.copy_from(&mut io::repeat(7), 0, None, None);
            let _ = done_tx.send(());
        })
        .unwrap();
        drop(stream);
        done_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    }
}

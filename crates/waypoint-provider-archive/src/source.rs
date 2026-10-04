// Reading the bytes of an archive file from any provider, with the seeks zip and 7z need.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::sync::Arc;

use tempfile::TempPath;
use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{InjectedError, Provider, ReadStream};

/// A forward seek shorter than this is done by reading and discarding, not by opening the file
/// again (a round trip on a server).
const SKIP_LIMIT: u64 = 256 * 1024;
const BUFFER: usize = 64 * 1024;

/// An archive file, wherever it is.
#[derive(Clone)]
pub(crate) struct SourceSpec {
    pub provider: Arc<dyn Provider>,
    pub path: VfsPath,
    pub size: u64,
    /// A scratch copy on disk, when the archive is inside another archive and needs seeks.
    pub spool: Option<Arc<TempPath>>,
}

impl SourceSpec {
    /// A reader that can seek, which a zip's central directory and a 7z header need.
    pub fn open_seek(&self) -> io::Result<SeekSource> {
        if let Some(spool) = &self.spool {
            let file = File::open(spool.as_ref() as &std::path::Path)?;
            return Ok(SeekSource::File(BufReader::with_capacity(BUFFER, file)));
        }
        Ok(SeekSource::Ranged(RangeReader::new(
            self.provider.clone(),
            self.path.clone(),
            self.size,
        )))
    }

    /// The file's bytes from the start, for a compressed tar (which is only ever read through).
    pub fn open_stream(&self) -> Result<ReadStream, VfsError> {
        if let Some(spool) = &self.spool {
            let file = File::open(spool.as_ref() as &std::path::Path)
                .map_err(|error| waypoint_vfs::from_io(&error, &self.path.to_location()))?;
            return Ok(Box::new(BufReader::with_capacity(BUFFER, file)));
        }
        self.provider.open_read(&self.path)
    }
}

pub(crate) enum SeekSource {
    File(BufReader<File>),
    Ranged(RangeReader),
}

impl Read for SeekSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(file) => file.read(buf),
            Self::Ranged(ranged) => ranged.read(buf),
        }
    }
}

impl Seek for SeekSource {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match self {
            Self::File(file) => file.seek(to),
            Self::Ranged(ranged) => ranged.seek(to),
        }
    }
}

struct Open {
    /// Where in the file the next byte of `reader` is.
    at: u64,
    reader: BufReader<ReadStream>,
}

/// Reads a file through `Provider::open_read_at`. A seek only moves the position; the stream is
/// opened (or skipped forward) by the next read, so a header read at the end of the file costs one
/// ranged request.
pub(crate) struct RangeReader {
    provider: Arc<dyn Provider>,
    path: VfsPath,
    size: u64,
    pos: u64,
    open: Option<Open>,
}

impl RangeReader {
    pub fn new(provider: Arc<dyn Provider>, path: VfsPath, size: u64) -> Self {
        Self {
            provider,
            path,
            size,
            pos: 0,
            open: None,
        }
    }

    fn ensure_open(&mut self) -> io::Result<()> {
        if let Some(open) = &mut self.open {
            if open.at == self.pos {
                return Ok(());
            }
            if open.at < self.pos && self.pos - open.at <= SKIP_LIMIT {
                let gap = self.pos - open.at;
                let skipped = io::copy(&mut (&mut open.reader).take(gap), &mut io::sink())?;
                open.at += skipped;
                if skipped == gap {
                    return Ok(());
                }
            }
        }
        let stream = self
            .provider
            .open_read_at(&self.path, self.pos)
            .map_err(|error| InjectedError(error).into_io())?;
        self.open = Some(Open {
            at: self.pos,
            reader: BufReader::with_capacity(BUFFER, stream),
        });
        Ok(())
    }
}

impl Read for RangeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.size || buf.is_empty() {
            return Ok(0);
        }
        self.ensure_open()?;
        let open = self.open.as_mut().expect("opened above");
        let read = open.reader.read(buf)?;
        open.at += read as u64;
        self.pos += read as u64;
        Ok(read)
    }
}

impl Seek for RangeReader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(delta) => self.size.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
        };
        let target = target.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before the start of the file",
            )
        })?;
        self.pos = target;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use waypoint_path::FilePath;
    use waypoint_vfs::{CancelToken, Capabilities, ScannedEntry};

    /// A provider whose one file is a counted byte pattern, counting how often it is opened.
    struct Pattern {
        opens: AtomicUsize,
        len: u64,
    }

    impl Provider for Pattern {
        fn scheme(&self) -> &'static str {
            "file"
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities::new(waypoint_path::CaseRule::Sensitive)
        }
        fn stat(&self, _: &VfsPath) -> Result<ScannedEntry, VfsError> {
            unreachable!()
        }
        fn list(
            &self,
            _: &VfsPath,
            _: &CancelToken,
            _: usize,
            _: &mut dyn FnMut(u32),
        ) -> Result<Vec<ScannedEntry>, VfsError> {
            unreachable!()
        }
        fn resolve_link(&self, _: &VfsPath, _: &ScannedEntry) -> Result<ScannedEntry, VfsError> {
            unreachable!()
        }
        fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
            self.open_read_at(path, 0)
        }
        fn open_read_at(&self, _: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            let len = self.len;
            Ok(Box::new(io::Cursor::new(
                (start..len).map(|at| (at % 251) as u8).collect::<Vec<u8>>(),
            )))
        }
    }

    fn reader(len: u64) -> (Arc<Pattern>, RangeReader) {
        let provider = Arc::new(Pattern {
            opens: AtomicUsize::new(0),
            len,
        });
        let path = VfsPath::File(
            FilePath::parse(if cfg!(windows) { "C:\\a.zip" } else { "/a.zip" }).unwrap(),
        );
        (provider.clone(), RangeReader::new(provider, path, len))
    }

    #[test]
    fn a_seek_reads_the_right_bytes_with_one_open_each() {
        let (provider, mut reader) = reader(1_000_000);
        reader.seek(SeekFrom::End(-10)).unwrap();
        let mut tail = [0u8; 10];
        reader.read_exact(&mut tail).unwrap();
        assert_eq!(tail[0], (999_990 % 251) as u8);
        reader.seek(SeekFrom::Start(5)).unwrap();
        let mut head = [0u8; 3];
        reader.read_exact(&mut head).unwrap();
        assert_eq!(head, [5, 6, 7]);
        assert_eq!(provider.opens.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_short_forward_seek_skips_instead_of_reopening() {
        let (provider, mut reader) = reader(1_000_000);
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte).unwrap();
        reader.seek(SeekFrom::Current(1000)).unwrap();
        reader.read_exact(&mut byte).unwrap();
        assert_eq!(byte[0], (1001 % 251) as u8);
        assert_eq!(provider.opens.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn reading_past_the_end_is_the_end() {
        let (_, mut reader) = reader(10);
        reader.seek(SeekFrom::Start(10)).unwrap();
        assert_eq!(reader.read(&mut [0u8; 4]).unwrap(), 0);
        assert!(reader.seek(SeekFrom::Current(-11)).is_err());
    }
}

// An upload waits here, in memory and then in a temporary file, until it is whole: a request can
// then carry its length (servers differ on chunked uploads) and be sent again after a login
// challenge or a redirect.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

use reqwest::Body;
use ring::digest;
use tempfile::{NamedTempFile, TempPath};
use tokio_util::io::ReaderStream;

use crate::auth::hex;

/// Uploads up to this size never touch the disk.
const MEMORY_LIMIT: usize = 4 * 1024 * 1024;

/// The bytes of an upload as they are written.
pub(crate) struct Spool {
    memory: Vec<u8>,
    file: Option<NamedTempFile>,
    len: u64,
    sha256: digest::Context,
    dir: Option<PathBuf>,
}

impl Spool {
    pub(crate) fn new(dir: Option<PathBuf>) -> Self {
        Self {
            memory: Vec::new(),
            file: None,
            len: 0,
            sha256: digest::Context::new(&digest::SHA256),
            dir,
        }
    }

    /// Closes the spool: the upload is whole.
    pub(crate) fn finish(mut self) -> io::Result<Upload> {
        let sha256 = hex(self.sha256.finish().as_ref());
        let data = match self.file.take() {
            Some(mut file) => {
                file.flush()?;
                Data::File(file.into_temp_path())
            }
            None => Data::Memory(std::mem::take(&mut self.memory)),
        };
        Ok(Upload {
            data,
            len: self.len,
            sha256,
        })
    }
}

impl Write for Spool {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.file.is_none() && self.memory.len() + buf.len() > MEMORY_LIMIT {
            let mut file = match &self.dir {
                Some(dir) => NamedTempFile::new_in(dir)?,
                None => NamedTempFile::new()?,
            };
            file.write_all(&self.memory)?;
            self.memory = Vec::new();
            self.file = Some(file);
        }
        match &mut self.file {
            Some(file) => file.write_all(buf)?,
            None => self.memory.extend_from_slice(buf),
        }
        self.sha256.update(buf);
        self.len += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.file {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

enum Data {
    Memory(Vec<u8>),
    File(TempPath),
}

/// A whole upload, which can be sent as often as a request needs it. Its file is removed when the
/// last holder lets go.
pub(crate) struct Upload {
    data: Data,
    pub len: u64,
    /// The SHA-256 of the bytes, in lower-case hex.
    pub sha256: String,
}

impl Upload {
    /// A fresh body over the same bytes.
    pub(crate) async fn body(self: &Arc<Self>) -> io::Result<Body> {
        match &self.data {
            Data::Memory(bytes) => Ok(Body::from(bytes.clone())),
            Data::File(path) => {
                let file = tokio::fs::File::open(path).await?;
                Ok(Body::wrap_stream(ReaderStream::new(file)))
            }
        }
    }

    pub(crate) fn empty() -> Arc<Self> {
        Arc::new(Self {
            data: Data::Memory(Vec::new()),
            len: 0,
            sha256: hex(digest::digest(&digest::SHA256, b"").as_ref()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_uploads_stay_in_memory_and_large_ones_spill() {
        let mut small = Spool::new(None);
        small.write_all(b"abc").unwrap();
        let upload = small.finish().unwrap();
        assert_eq!(upload.len, 3);
        assert!(matches!(upload.data, Data::Memory(_)));
        assert_eq!(
            upload.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        let dir = tempfile::tempdir().unwrap();
        let mut large = Spool::new(Some(dir.path().to_owned()));
        let block = vec![7u8; 1024 * 1024];
        for _ in 0..5 {
            large.write_all(&block).unwrap();
        }
        let upload = large.finish().unwrap();
        assert_eq!(upload.len, 5 * 1024 * 1024);
        let Data::File(path) = &upload.data else {
            panic!("spilled to a file")
        };
        assert_eq!(std::fs::metadata(path).unwrap().len(), upload.len);
        let at = path.to_path_buf();
        drop(upload);
        assert!(!at.exists(), "the file goes with the upload");
    }
}

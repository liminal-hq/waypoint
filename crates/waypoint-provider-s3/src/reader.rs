// Streaming reads: a ranged `GetObject` whose body is read as the caller asks, and reopened from
// where it stopped when the connection breaks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read};
use std::sync::Arc;

use aws_sdk_s3::primitives::ByteStream;
use tokio::runtime::Handle;
use waypoint_vfs::InjectedError;

use crate::address::Address;
use crate::errors::S3Error;
use crate::service::{Failed, Inner};

/// Times a body is reopened after the connection under it broke, before the read fails.
const REOPENS: u32 = 3;

/// An object being read. It reads on the provider's runtime from the calling (worker) thread, one
/// body chunk at a time, so nothing is read ahead of what the caller asks for.
pub(crate) struct S3Reader {
    inner: Arc<Inner>,
    handle: Handle,
    address: Address,
    body: Option<ByteStream>,
    chunk: bytes::Bytes,
    taken: usize,
    /// The offset in the object of the next byte this reader hands out.
    offset: u64,
    /// What the object looked like when it was opened, so a reopen never splices two versions.
    etag: Option<String>,
    reopens: u32,
}

/// What opening an object for reading gave.
pub(crate) struct Opened {
    pub body: ByteStream,
    pub etag: Option<String>,
}

impl Inner {
    /// `GetObject` from `start`. A start at or past the end is `InvalidRange`.
    pub(crate) async fn get(
        &self,
        address: &Address,
        start: u64,
        if_match: Option<&str>,
    ) -> Result<Opened, Failed> {
        let (bucket, key) = (address.bucket.clone(), address.key.clone());
        let if_match = if_match.map(str::to_owned);
        let output = self
            .run(address, |client| {
                let (bucket, key, if_match) = (bucket.clone(), key.clone(), if_match.clone());
                async move {
                    let mut request = client.get_object().bucket(bucket).key(key);
                    if start > 0 {
                        request = request.range(format!("bytes={start}-"));
                    }
                    if let Some(etag) = if_match {
                        request = request.if_match(etag);
                    }
                    request.send().await
                }
            })
            .await?;
        Ok(Opened {
            etag: output.e_tag().map(str::to_owned),
            body: output.body,
        })
    }
}

impl S3Reader {
    pub(crate) fn new(
        inner: Arc<Inner>,
        handle: Handle,
        address: Address,
        start: u64,
        opened: Opened,
    ) -> Self {
        Self {
            inner,
            handle,
            address,
            body: Some(opened.body),
            chunk: bytes::Bytes::new(),
            taken: 0,
            offset: start,
            etag: opened.etag,
            reopens: 0,
        }
    }

    fn failure(&self, error: S3Error) -> io::Error {
        InjectedError(error.into_vfs(&self.address.location)).into_io()
    }

    /// Opens the body again from `self.offset`.
    fn reopen(&mut self) -> io::Result<()> {
        if self.reopens >= REOPENS {
            return Err(self.failure(S3Error::Disconnected));
        }
        self.reopens += 1;
        let reopened = self.handle.block_on(self.inner.get(
            &self.address,
            self.offset,
            self.etag.as_deref(),
        ));
        match reopened {
            Ok(opened) => {
                self.body = Some(opened.body);
                Ok(())
            }
            Err(Failed::S3(error)) => Err(self.failure(error)),
            Err(Failed::Vfs(error)) => Err(InjectedError(error).into_io()),
        }
    }
}

impl Read for S3Reader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if self.taken < self.chunk.len() {
                let count = out.len().min(self.chunk.len() - self.taken);
                out[..count].copy_from_slice(&self.chunk[self.taken..self.taken + count]);
                self.taken += count;
                self.offset += count as u64;
                return Ok(count);
            }
            let Some(body) = self.body.as_mut() else {
                return Ok(0);
            };
            match self.handle.block_on(body.try_next()) {
                Ok(Some(chunk)) => {
                    self.chunk = chunk;
                    self.taken = 0;
                }
                Ok(None) => {
                    self.body = None;
                    return Ok(0);
                }
                Err(error) => {
                    log::debug!(
                        "s3: the body of {} broke at {}: {error}",
                        self.address.location.uri,
                        self.offset
                    );
                    self.reopen()?;
                }
            }
        }
    }
}

/// An empty reader, for a read that starts at or past the end.
pub(crate) fn empty() -> Box<dyn Read + Send> {
    Box::new(io::empty())
}

/// Whether a failed `get` was a range that starts past the end.
pub(crate) fn past_the_end(failed: &Failed) -> bool {
    failed.is(&S3Error::InvalidRange)
}

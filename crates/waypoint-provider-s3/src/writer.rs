// Streaming writes: one `PutObject` for a small file and a multipart upload for a large one, with
// parts sent while the next are being filled and the upload aborted when the write fails or is
// dropped.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Write};
use std::sync::Arc;

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::CompletedPart;
use tokio::runtime::Handle;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use waypoint_protocol::VfsError;
use waypoint_vfs::{guess_mime, InjectedError, WriteStream};

use crate::address::Address;
use crate::errors::S3Error;
use crate::options::{part_size_for, S3Options};
use crate::service::{Failed, Inner};

type PartTask = JoinHandle<Result<CompletedPart, Failed>>;

/// A file being written. Nothing is visible on the service until `finish` succeeds (S3 makes an
/// object appear whole), so the engine writes straight to the final name.
pub(crate) struct S3Writer {
    inner: Arc<Inner>,
    handle: Handle,
    address: Address,
    /// Send `If-None-Match: *`, so a name that appeared since the exclusive check is not replaced.
    conditional: bool,
    part_size: u64,
    permits: Arc<Semaphore>,
    buffer: Vec<u8>,
    upload_id: Option<String>,
    next_part: i32,
    tasks: Vec<PartTask>,
    done: Vec<CompletedPart>,
    finished: bool,
}

impl S3Writer {
    pub(crate) fn new(
        inner: Arc<Inner>,
        handle: Handle,
        address: Address,
        options: &S3Options,
        conditional: bool,
    ) -> Self {
        Self {
            inner,
            handle,
            address,
            conditional,
            part_size: options.part_size(),
            permits: Arc::new(Semaphore::new(options.parts_in_flight())),
            buffer: Vec::new(),
            upload_id: None,
            next_part: 1,
            tasks: Vec::new(),
            done: Vec::new(),
            finished: false,
        }
    }

    fn fail(&self, failed: Failed) -> io::Error {
        InjectedError(failed.into_vfs(&self.address.location)).into_io()
    }

    fn content_type(&self) -> Option<String> {
        guess_mime(self.address.name(), None)
    }

    /// Starts the multipart upload, if it has not been started.
    fn ensure_upload(&mut self) -> Result<(), Failed> {
        if self.upload_id.is_some() {
            return Ok(());
        }
        let (bucket, key) = (self.address.bucket.clone(), self.address.key.clone());
        let content_type = self.content_type();
        let created = self
            .handle
            .block_on(self.inner.run(&self.address, |client| {
                let (bucket, key, content_type) =
                    (bucket.clone(), key.clone(), content_type.clone());
                async move {
                    client
                        .create_multipart_upload()
                        .bucket(bucket)
                        .key(key)
                        .set_content_type(content_type)
                        .send()
                        .await
                }
            }))?;
        match created.upload_id() {
            Some(id) => {
                self.upload_id = Some(id.to_owned());
                Ok(())
            }
            None => Err(Failed::S3(S3Error::Service {
                status: None,
                code: None,
            })),
        }
    }

    /// Sends `part` as the next part, waiting for a free slot when several are in flight.
    fn send_part(&mut self, part: Vec<u8>) -> Result<(), Failed> {
        self.ensure_upload()?;
        let Some(upload_id) = self.upload_id.clone() else {
            return Ok(());
        };
        let number = self.next_part;
        self.next_part += 1;
        let permit = self
            .handle
            .block_on(self.permits.clone().acquire_owned())
            .map_err(|_| Failed::S3(S3Error::Disconnected))?;
        let inner = self.inner.clone();
        let address = self.address.clone();
        let body = bytes::Bytes::from(part);
        self.tasks.push(self.handle.spawn(async move {
            let (bucket, key) = (address.bucket.clone(), address.key.clone());
            let result = inner
                .run(&address, |client| {
                    let (bucket, key, upload_id) = (bucket.clone(), key.clone(), upload_id.clone());
                    let body = body.clone();
                    async move {
                        client
                            .upload_part()
                            .bucket(bucket)
                            .key(key)
                            .upload_id(upload_id)
                            .part_number(number)
                            .body(ByteStream::from(body))
                            .send()
                            .await
                    }
                })
                .await;
            drop(permit);
            result.map(|output| {
                CompletedPart::builder()
                    .part_number(number)
                    .set_e_tag(output.e_tag().map(str::to_owned))
                    .build()
            })
        }));
        self.reap(false)
    }

    /// Collects the parts that have been sent: only the finished ones, or every one.
    fn reap(&mut self, all: bool) -> Result<(), Failed> {
        let mut pending = Vec::new();
        for task in std::mem::take(&mut self.tasks) {
            if all || task.is_finished() {
                match self.handle.block_on(task) {
                    Ok(Ok(part)) => self.done.push(part),
                    Ok(Err(failed)) => return Err(failed),
                    Err(_) => return Err(Failed::S3(S3Error::Disconnected)),
                }
            } else {
                pending.push(task);
            }
        }
        self.tasks = pending;
        Ok(())
    }

    /// Gives up on the upload: stops the parts and has the service drop what it holds.
    fn abort(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
        if let Some(upload_id) = self.upload_id.take() {
            let inner = self.inner.clone();
            let address = self.address.clone();
            self.handle.spawn(async move {
                inner.abort_upload(&address, &upload_id).await;
            });
        }
    }

    fn write_buffered(&mut self) -> Result<(), Failed> {
        loop {
            let size = part_size_for(self.part_size, (self.next_part - 1) as u64) as usize;
            if self.buffer.len() < size {
                return Ok(());
            }
            let rest = self.buffer.split_off(size);
            let part = std::mem::replace(&mut self.buffer, rest);
            self.send_part(part)?;
        }
    }

    fn complete(&mut self) -> Result<(), Failed> {
        if self.upload_id.is_none() {
            let (bucket, key) = (self.address.bucket.clone(), self.address.key.clone());
            let body = bytes::Bytes::from(std::mem::take(&mut self.buffer));
            let content_type = self.content_type();
            let conditional = self.conditional;
            return self
                .handle
                .block_on(self.inner.run(&self.address, |client| {
                    let (bucket, key, content_type) =
                        (bucket.clone(), key.clone(), content_type.clone());
                    let body = body.clone();
                    async move {
                        let mut request = client
                            .put_object()
                            .bucket(bucket)
                            .key(key)
                            .set_content_type(content_type)
                            .body(ByteStream::from(body));
                        if conditional {
                            request = request.if_none_match("*");
                        }
                        request.send().await
                    }
                }))
                .map(|_| ());
        }
        if !self.buffer.is_empty() {
            let last = std::mem::take(&mut self.buffer);
            self.send_part(last)?;
        }
        self.reap(true)?;
        let Some(upload_id) = self.upload_id.clone() else {
            return Ok(());
        };
        let mut parts = std::mem::take(&mut self.done);
        parts.sort_by_key(|part| part.part_number());
        let completed = self.handle.block_on(self.inner.complete_upload(
            &self.address,
            &upload_id,
            parts,
            self.conditional,
        ));
        if completed.is_ok() {
            self.upload_id = None;
        }
        completed
    }
}

impl Write for S3Writer {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(data);
        if let Err(failed) = self.write_buffered().and_then(|()| self.reap(false)) {
            self.abort();
            return Err(self.fail(failed));
        }
        Ok(data.len())
    }

    /// Parts are sent as they fill; what is left waits for `finish`, as an object cannot be
    /// appended to.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl WriteStream for S3Writer {
    fn finish(mut self: Box<Self>, _sync: bool) -> Result<(), VfsError> {
        self.finished = true;
        match self.complete() {
            Ok(()) => Ok(()),
            Err(failed) => {
                self.abort();
                Err(failed.into_vfs(&self.address.location))
            }
        }
    }
}

impl Drop for S3Writer {
    fn drop(&mut self) {
        // Dropped without `finish` (cancelled, or an error elsewhere): nothing of it is kept.
        if !self.finished || self.upload_id.is_some() {
            self.abort();
        }
    }
}

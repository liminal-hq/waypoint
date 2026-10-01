// Copying one file's bytes between providers, behind a trait so the chunked engine with fast paths,
// verification and the cross-device policy can replace the plain loop.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{Read, Write};

use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{from_io, CancelToken, Provider, WriteOptions};

/// One file to copy.
pub struct CopyRequest<'a> {
    pub src_provider: &'a dyn Provider,
    pub src: &'a VfsPath,
    pub dst_provider: &'a dyn Provider,
    /// Where the bytes go, created exclusively. The executor passes the name of a partial file
    /// and renames it into place, so an implementation never writes under a final name.
    pub dst: &'a VfsPath,
}

/// Copies the bytes of a file. An implementation creates `dst` exclusively, reports the bytes
/// copied so far through `progress`, checks `cancel` between chunks (a cancel is
/// `VfsError::Cancelled`), and on any failure leaves no `dst` behind. Times and permissions are
/// the caller's business.
pub trait CopyFile: Send + Sync {
    fn copy_file(
        &self,
        request: &CopyRequest<'_>,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Result<u64, VfsError>;
}

/// The straightforward copy: read a chunk, write a chunk.
#[derive(Debug, Clone, Copy, Default)]
pub struct SimpleCopy;

const CHUNK: usize = 64 * 1024;

impl CopyFile for SimpleCopy {
    fn copy_file(
        &self,
        request: &CopyRequest<'_>,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Result<u64, VfsError> {
        let src_location = request.src.to_location();
        let dst_location = request.dst.to_location();
        let mut reader = request.src_provider.open_read(request.src)?;
        let mut writer = request
            .dst_provider
            .create_write(request.dst, WriteOptions::exclusive())?;
        let mut buffer = vec![0u8; CHUNK];
        let mut copied = 0u64;
        let result = (|| -> Result<(), VfsError> {
            loop {
                if cancel.is_cancelled() {
                    return Err(VfsError::Cancelled);
                }
                let read = reader
                    .read(&mut buffer)
                    .map_err(|e| from_io(&e, &src_location))?;
                if read == 0 {
                    return Ok(());
                }
                writer
                    .write_all(&buffer[..read])
                    .map_err(|e| from_io(&e, &dst_location))?;
                copied += read as u64;
                progress(copied);
            }
        })();
        match result {
            Ok(()) => match writer.finish(false) {
                Ok(()) => Ok(copied),
                Err(error) => {
                    let _ = request.dst_provider.remove_file(request.dst);
                    Err(error)
                }
            },
            Err(error) => {
                drop(writer);
                let _ = request.dst_provider.remove_file(request.dst);
                Err(error)
            }
        }
    }
}

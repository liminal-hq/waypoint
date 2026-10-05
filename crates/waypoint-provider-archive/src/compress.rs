// The decompressors a tar archive may be wrapped in, all in Rust.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, BufReader, Read};

use waypoint_protocol::VfsError;
use waypoint_vfs::{CancelToken, InjectedError};

use crate::errors::DecodeErrors;
use crate::format::TarCompression;

/// The most memory an xz stream may ask a decoder for (its dictionary), in KiB.
const XZ_MEMORY_KIB: u32 = 1024 * 1024;

/// Wraps `reader` in the decoder for `compression`.
pub(crate) fn decoder(
    compression: TarCompression,
    reader: Box<dyn Read + Send>,
) -> io::Result<Box<dyn Read + Send>> {
    let buffered = BufReader::with_capacity(128 * 1024, reader);
    let decoded: Box<dyn Read + Send> = match compression {
        TarCompression::None => return Ok(Box::new(buffered)),
        TarCompression::Gzip => Box::new(flate2::read::MultiGzDecoder::new(buffered)),
        TarCompression::Bzip2 => Box::new(bzip2::read::MultiBzDecoder::new(buffered)),
        TarCompression::Xz => Box::new(lzma_rust2::XzReader::new_mem_limit(
            buffered,
            true,
            XZ_MEMORY_KIB,
        )),
        TarCompression::Zstd => Box::new(
            ruzstd::decoding::StreamingDecoder::new(buffered)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?,
        ),
    };
    Ok(Box::new(DecodeErrors(decoded)))
}

/// Checks a token on every read, so a long decompression stops when a scan is cancelled.
pub(crate) struct CancelReader<R> {
    pub inner: R,
    pub cancel: CancelToken,
}

impl<R: Read> Read for CancelReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(InjectedError(VfsError::Cancelled).into_io());
        }
        self.inner.read(buf)
    }
}

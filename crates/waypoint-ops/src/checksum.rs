// A checksum of one file on request (the Properties window): the verification hashes, streamed, cancellable and reporting progress
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_vfs::{from_io, CancelToken, EntryKind, Provider};

use crate::model::VerifyAlgorithm;
use crate::verify::{hex, Hasher};

/// How much is read between looks at the cancel flag.
const READ_BYTES: usize = 1024 * 1024;

/// How often progress is reported at most.
pub const REPORT_EVERY: Duration = Duration::from_millis(100);

/// What a checksum run tells the window that started it. Exactly one of `done`, `cancelled` and
/// `failed` ends the stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ChecksumEvent {
    /// How far the hash has read, of the size the file had when the run started.
    Progress {
        #[ts(type = "number")]
        bytes_read: u64,
        #[ts(type = "number")]
        total: u64,
    },
    /// The digest, in lower-case hex, of the whole file.
    Done {
        algorithm: VerifyAlgorithm,
        digest: String,
        #[ts(type = "number")]
        bytes: u64,
    },
    Cancelled,
    Failed {
        error: VfsError,
    },
}

/// Hashes everything `reader` yields, `total` bytes being what it is expected to hold (a hint for
/// progress only). `report` hears the bytes read so far, at most once per `REPORT_EVERY` and not at
/// all for a file that is read faster than that. `Ok(None)` is a run that was cancelled.
pub fn checksum_reader(
    reader: &mut dyn Read,
    algorithm: VerifyAlgorithm,
    cancel: &CancelToken,
    mut report: impl FnMut(u64),
) -> std::io::Result<Option<(String, u64)>> {
    let mut hasher = Hasher::new(algorithm);
    let mut buffer = vec![0u8; READ_BYTES];
    let mut read = 0u64;
    let mut last = Instant::now();
    loop {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let n = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        hasher.update(&buffer[..n]);
        read += n as u64;
        if last.elapsed() >= REPORT_EVERY {
            report(read);
            last = Instant::now();
        }
    }
    Ok(Some((hex(&hasher.finish()), read)))
}

/// Hashes the file at `path` through `provider`, sending its events to `sink`. A folder, or
/// anything a provider cannot read, ends in `Failed`.
pub fn run_checksum(
    provider: &dyn Provider,
    path: &VfsPath,
    algorithm: VerifyAlgorithm,
    cancel: &CancelToken,
    sink: &mut dyn FnMut(ChecksumEvent),
) {
    let location = path.to_location();
    let total = match provider.stat(path) {
        Ok(entry) => {
            // A link is judged by what it leads to; a pipe or a device would never end.
            let target = if entry.kind == EntryKind::Symlink {
                entry.link_target.unwrap_or(EntryKind::Other)
            } else {
                entry.kind
            };
            match target {
                EntryKind::Directory => {
                    return sink(ChecksumEvent::Failed {
                        error: VfsError::IsADirectory { location },
                    });
                }
                EntryKind::File => entry.size.unwrap_or(0),
                _ => {
                    return sink(ChecksumEvent::Failed {
                        error: VfsError::Unsupported {
                            what: "a checksum of something that is not a regular file".to_owned(),
                        },
                    });
                }
            }
        }
        Err(error) => return sink(ChecksumEvent::Failed { error }),
    };
    let mut stream = match provider.open_read(path) {
        Ok(stream) => stream,
        Err(error) => return sink(ChecksumEvent::Failed { error }),
    };
    let result = checksum_reader(&mut stream, algorithm, cancel, |bytes_read| {
        sink(ChecksumEvent::Progress { bytes_read, total });
    });
    sink(match result {
        Ok(Some((digest, bytes))) => ChecksumEvent::Done {
            algorithm,
            digest,
            bytes,
        },
        Ok(None) => ChecksumEvent::Cancelled,
        Err(error) => ChecksumEvent::Failed {
            error: from_io(&error, &location),
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn bytes(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn a_digest_matches_the_known_vector() {
        let mut data = Cursor::new(b"abc".to_vec());
        let (digest, read) = checksum_reader(
            &mut data,
            VerifyAlgorithm::Sha256,
            &CancelToken::new(),
            |_| {},
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(read, 3);
    }

    #[test]
    fn blake3_is_offered_as_well() {
        let mut data = Cursor::new(Vec::new());
        let (digest, _) = checksum_reader(
            &mut data,
            VerifyAlgorithm::Blake3,
            &CancelToken::new(),
            |_| {},
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            digest,
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    #[test]
    fn a_cancelled_run_stops_without_a_digest() {
        let cancel = CancelToken::new();
        cancel.cancel();
        let mut data = Cursor::new(bytes(10 * READ_BYTES));
        let result = checksum_reader(&mut data, VerifyAlgorithm::Sha256, &cancel, |_| {}).unwrap();
        assert!(result.is_none());
        // Nothing was read once the flag was up.
        assert_eq!(data.position(), 0);
    }

    #[test]
    fn cancelling_part_way_stops_at_the_next_chunk() {
        let cancel = CancelToken::new();
        let mut data = Cursor::new(bytes(4 * READ_BYTES));
        let seen = cancel.clone();
        let mut reader = std::io::Read::take(&mut data, u64::MAX);
        struct Trip<'a, R: Read> {
            inner: &'a mut R,
            cancel: CancelToken,
            reads: usize,
        }
        impl<R: Read> Read for Trip<'_, R> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                self.reads += 1;
                if self.reads == 2 {
                    self.cancel.cancel();
                }
                self.inner.read(buf)
            }
        }
        let mut trip = Trip {
            inner: &mut reader,
            cancel: seen,
            reads: 0,
        };
        let result = checksum_reader(&mut trip, VerifyAlgorithm::Sha256, &cancel, |_| {}).unwrap();
        assert!(result.is_none());
        assert_eq!(trip.reads, 2);
    }
}

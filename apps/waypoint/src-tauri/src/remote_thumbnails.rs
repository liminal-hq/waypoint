// Thumbnails of files on servers: read through the provider, only as far as the connection allows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The thumbnails plugin opens local files only and calls no other plugin (A4). A file on a server
// is read here, through the same provider a listing uses, and handed to the plugin as bytes
// (`Thumbnails::from_bytes`), which keeps the result in a memory cache of its own keyed by the
// file's location, size and time. Nothing is read unless the file's connection turns thumbnails on
// (D14, D166): "small files only" reads an image no larger than the connection's cap, whole;
// "always" also takes, for a larger JPEG, the small preview a camera puts in its first bytes, read
// alone with a ranged read; anything else is skipped and never downloaded whole. A batch runs on a
// thread of its own, one file at a time in the order the page gave (the rows on screen first),
// and stops at the next file once the page cancels it (it scrolled away).

use std::io::Read;
use std::sync::Arc;

use tauri_plugin_thumbnails::{SkipWhy, ThumbEvent, ThumbSize, Thumbnails};
use waypoint_connections::{ConnectionOptions, RemoteThumbnails};
use waypoint_path::VfsPath;
use waypoint_protocol::Location;
use waypoint_vfs::{CancelToken, EntryKind, Provider};

/// How much of a large photo's start is read to find the preview it carries. Cameras put it in the
/// first segment, which the standard keeps under 64 KiB.
pub const PREFIX_BYTES: u64 = 64 * 1024;

/// What a connection lets a thumbnail read (D166).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub mode: RemoteThumbnails,
    /// The largest file read whole, in bytes.
    pub max_bytes: u64,
}

impl Policy {
    /// Nothing at all: a server nobody turned thumbnails on for.
    pub const OFF: Policy = Policy {
        mode: RemoteThumbnails::Off,
        max_bytes: 0,
    };

    /// What a saved connection's options allow.
    pub fn of(options: &ConnectionOptions) -> Policy {
        Policy {
            mode: options.thumbnails,
            max_bytes: options.thumbnail_max_bytes(),
        }
    }
}

/// What makes a thumbnail from bytes and remembers it; the plugin's `Thumbnails`, or a test's own.
pub trait Maker: Send + Sync {
    fn cached(&self, key: &str, uri: &str, mtime_ms: i64, size: ThumbSize) -> Option<ThumbEvent>;
    fn make(
        &self,
        key: &str,
        uri: &str,
        mtime_ms: i64,
        size: ThumbSize,
        bytes: &[u8],
    ) -> ThumbEvent;
}

impl Maker for Thumbnails {
    fn cached(&self, key: &str, uri: &str, mtime_ms: i64, size: ThumbSize) -> Option<ThumbEvent> {
        self.cached_from_bytes(key, uri, mtime_ms, size)
    }

    fn make(
        &self,
        key: &str,
        uri: &str,
        mtime_ms: i64,
        size: ThumbSize,
        bytes: &[u8],
    ) -> ThumbEvent {
        self.from_bytes(key, uri, mtime_ms, size, bytes)
    }
}

/// Reads up to `limit` bytes from the start of a stream (a whole small file, or a large one's start).
fn read_up_to(reader: &mut dyn Read, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    reader.take(limit).read_to_end(&mut out)?;
    Ok(out)
}

/// The thumbnail of one file on a server, under the page's `key`, as far as `policy` allows.
pub fn thumbnail(
    provider: &dyn Provider,
    path: &VfsPath,
    policy: Policy,
    size: ThumbSize,
    key: &str,
    maker: &dyn Maker,
) -> ThumbEvent {
    let skipped = |why| ThumbEvent::Skipped {
        key: key.to_owned(),
        why,
    };
    let failed = |reason: String| ThumbEvent::Failed {
        key: key.to_owned(),
        reason,
    };
    if policy.mode == RemoteThumbnails::Off {
        return skipped(SkipWhy::Remote);
    }
    let entry = match provider.stat(path) {
        Ok(entry) => entry,
        Err(error) => return failed(format!("{error:?}")),
    };
    if entry.kind != EntryKind::File {
        return skipped(SkipWhy::Unsupported);
    }
    let uri = path.to_uri();
    let mtime_ms = entry.modified_ms.unwrap_or(0);
    if let Some(known) = maker.cached(key, &uri, mtime_ms, size) {
        return known;
    }
    let file_size = entry.size.unwrap_or(u64::MAX);
    if file_size <= policy.max_bytes {
        let bytes = provider.open_read(path).and_then(|mut reader| {
            read_up_to(&mut reader, policy.max_bytes + 1)
                .map_err(|e| waypoint_vfs::from_io(&e, &path.to_location()))
        });
        return match bytes {
            Ok(bytes) if bytes.len() as u64 > policy.max_bytes => skipped(SkipWhy::TooLarge),
            Ok(bytes) => maker.make(key, &uri, mtime_ms, size, &bytes),
            Err(error) => failed(format!("{error:?}")),
        };
    }
    if policy.mode != RemoteThumbnails::Always {
        return skipped(SkipWhy::TooLarge);
    }
    // A large file: only a photo's own small preview, from its first bytes; the stream is dropped
    // after them, which ends the transfer.
    let prefix = provider.open_read_at(path, 0).and_then(|mut reader| {
        read_up_to(&mut reader, PREFIX_BYTES)
            .map_err(|e| waypoint_vfs::from_io(&e, &path.to_location()))
    });
    match prefix {
        Ok(prefix) => match embedded_jpeg(&prefix) {
            Some(preview) => maker.make(key, &uri, mtime_ms, size, preview),
            None => skipped(SkipWhy::TooLarge),
        },
        Err(error) => failed(format!("{error:?}")),
    }
}

/// One file the page wants a thumbnail of, on a server.
pub struct Remote {
    pub key: String,
    pub location: Location,
}

/// Where a batch finds providers and each connection's choice.
pub trait Servers: Send + Sync {
    fn provider(&self, path: &VfsPath) -> Option<Arc<dyn Provider>>;
    fn policy(&self, path: &VfsPath) -> Policy;
}

/// Makes the thumbnails of a batch of server files one after another, sending each result to
/// `send`, until `cancel` is set.
pub fn run_batch(
    servers: &dyn Servers,
    maker: &dyn Maker,
    items: Vec<Remote>,
    size: ThumbSize,
    cancel: &CancelToken,
    send: &dyn Fn(ThumbEvent),
) {
    for item in items {
        if cancel.is_cancelled() {
            return;
        }
        let event = match VfsPath::from_location(&item.location) {
            Err(error) => ThumbEvent::Failed {
                key: item.key,
                reason: error.to_string(),
            },
            Ok(path) => match servers.provider(&path) {
                None => ThumbEvent::Skipped {
                    key: item.key,
                    why: SkipWhy::Remote,
                },
                Some(provider) => thumbnail(
                    provider.as_ref(),
                    &path,
                    servers.policy(&path),
                    size,
                    &item.key,
                    maker,
                ),
            },
        };
        if cancel.is_cancelled() {
            return;
        }
        send(event);
    }
}

// ---- a photo's own preview ----

fn u16_at(bytes: &[u8], at: usize, big: bool) -> Option<u16> {
    let pair: [u8; 2] = bytes.get(at..at + 2)?.try_into().ok()?;
    Some(if big {
        u16::from_be_bytes(pair)
    } else {
        u16::from_le_bytes(pair)
    })
}

fn u32_at(bytes: &[u8], at: usize, big: bool) -> Option<u32> {
    let quad: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    Some(if big {
        u32::from_be_bytes(quad)
    } else {
        u32::from_le_bytes(quad)
    })
}

/// The JPEG preview an Exif block at the start of a JPEG carries (the second image directory's
/// `JPEGInterchangeFormat` and its length), when `bytes` holds all of it. Reads only what is
/// there: a block cut short, a bad offset or a preview that is not a JPEG is `None`.
pub fn embedded_jpeg(bytes: &[u8]) -> Option<&[u8]> {
    if bytes.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xFF {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        // The start of the image data: no Exif block came first.
        if marker == 0xDA || marker == 0xD9 {
            return None;
        }
        let length = usize::from(u16_at(bytes, at + 2, true)?);
        let body = at + 4;
        if marker == 0xE1 && bytes.get(body..body + 6)? == b"Exif\0\0" {
            let tiff = bytes.get(body + 6..at + 2 + length)?;
            return exif_preview(tiff);
        }
        at += 2 + length;
    }
}

fn exif_preview(tiff: &[u8]) -> Option<&[u8]> {
    let big = match tiff.get(..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    if u16_at(tiff, 2, big)? != 42 {
        return None;
    }
    let ifd0 = usize::try_from(u32_at(tiff, 4, big)?).ok()?;
    let count0 = usize::from(u16_at(tiff, ifd0, big)?);
    let ifd1 = usize::try_from(u32_at(tiff, ifd0 + 2 + count0 * 12, big)?).ok()?;
    if ifd1 == 0 {
        return None;
    }
    let count1 = usize::from(u16_at(tiff, ifd1, big)?);
    let (mut offset, mut length) = (None, None);
    for n in 0..count1 {
        let entry = ifd1 + 2 + n * 12;
        let value = usize::try_from(u32_at(tiff, entry + 8, big)?).ok()?;
        match u16_at(tiff, entry, big)? {
            0x0201 => offset = Some(value),
            0x0202 => length = Some(value),
            _ => {}
        }
    }
    let (offset, length) = (offset?, length?);
    let preview = tiff.get(offset..offset.checked_add(length)?)?;
    (preview.get(..2)? == [0xFF, 0xD8]).then_some(preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use waypoint_vfs::{FakeRemoteProvider, MemOp};

    /// A maker that records what it was given and makes nothing.
    #[derive(Default)]
    struct Recorder {
        made: Mutex<Vec<(String, Vec<u8>)>>,
    }

    impl Maker for Recorder {
        fn cached(&self, _: &str, uri: &str, _: i64, _: ThumbSize) -> Option<ThumbEvent> {
            let made = self.made.lock().unwrap();
            made.iter()
                .any(|(u, _)| u == uri)
                .then(|| ThumbEvent::Ready {
                    key: "cached".into(),
                    url: "thumb://cached".into(),
                })
        }

        fn make(&self, key: &str, uri: &str, _: i64, _: ThumbSize, bytes: &[u8]) -> ThumbEvent {
            self.made
                .lock()
                .unwrap()
                .push((uri.to_owned(), bytes.to_vec()));
            ThumbEvent::Ready {
                key: key.to_owned(),
                url: "thumb://made".into(),
            }
        }
    }

    /// A JPEG whose Exif block carries `preview` (in Motorola or Intel order), then `rest`.
    fn jpeg_with_preview(preview: &[u8], big: bool, rest: usize) -> Vec<u8> {
        let w16 = |v: u16| {
            if big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        let w32 = |v: u32| {
            if big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        let mut tiff = Vec::new();
        tiff.extend_from_slice(if big { b"MM" } else { b"II" });
        tiff.extend_from_slice(&w16(42));
        tiff.extend_from_slice(&w32(8));
        // IFD0: no entries, then IFD1 at 14.
        tiff.extend_from_slice(&w16(0));
        tiff.extend_from_slice(&w32(14));
        // IFD1: two entries, the preview right after it.
        let data = 14 + 2 + 2 * 12 + 4;
        tiff.extend_from_slice(&w16(2));
        for (tag, value) in [(0x0201u16, data as u32), (0x0202, preview.len() as u32)] {
            tiff.extend_from_slice(&w16(tag));
            tiff.extend_from_slice(&w16(4));
            tiff.extend_from_slice(&w32(1));
            tiff.extend_from_slice(&w32(value));
        }
        tiff.extend_from_slice(&w32(0));
        tiff.extend_from_slice(preview);
        let mut out = vec![0xFF, 0xD8, 0xFF, 0xE1];
        out.extend_from_slice(&((2 + 6 + tiff.len()) as u16).to_be_bytes());
        out.extend_from_slice(b"Exif\0\0");
        out.extend_from_slice(&tiff);
        out.extend_from_slice(&[0xFF, 0xDA, 0, 2]);
        out.extend(std::iter::repeat_n(7u8, rest));
        out
    }

    const PREVIEW: &[u8] = &[0xFF, 0xD8, 1, 2, 3, 0xFF, 0xD9];

    #[test]
    fn a_photos_own_preview_is_found_in_either_byte_order_and_nothing_else_passes() {
        for big in [true, false] {
            let jpeg = jpeg_with_preview(PREVIEW, big, 10);
            assert_eq!(embedded_jpeg(&jpeg), Some(PREVIEW), "big-endian {big}");
            // Cut short inside the block: nothing.
            assert_eq!(embedded_jpeg(&jpeg[..30]), None);
        }
        assert_eq!(embedded_jpeg(b"\x89PNG\r\n"), None);
        assert_eq!(embedded_jpeg(&[0xFF, 0xD8, 0xFF, 0xDA, 0, 2]), None);
        assert_eq!(
            embedded_jpeg(&jpeg_with_preview(b"not a jpeg", true, 0)),
            None
        );
    }

    fn server_with(name: &str, bytes: &[u8]) -> (FakeRemoteProvider, VfsPath) {
        let fake = FakeRemoteProvider::sftp();
        let path = fake.root("me@nas.lan").join(name).unwrap();
        fake.put_file(&path, bytes);
        (fake, path)
    }

    fn policy(mode: RemoteThumbnails, max_bytes: u64) -> Policy {
        Policy { mode, max_bytes }
    }

    #[test]
    fn nothing_is_read_from_a_server_with_thumbnails_off() {
        let (fake, path) = server_with("a.png", b"png");
        let maker = Recorder::default();
        let reads = fake.memory().calls(MemOp::OpenRead);
        let event = thumbnail(&fake, &path, Policy::OFF, ThumbSize::Normal, "k", &maker);
        assert!(matches!(
            event,
            ThumbEvent::Skipped {
                why: SkipWhy::Remote,
                ..
            }
        ));
        assert_eq!(fake.memory().calls(MemOp::OpenRead), reads);
        assert_eq!(fake.connects(), 0, "not even connected");
    }

    #[test]
    fn a_small_file_is_read_whole_once_and_a_large_one_never() {
        let (fake, path) = server_with("a.png", &[1u8; 100]);
        let maker = Recorder::default();
        let small = policy(RemoteThumbnails::SmallFiles, 1000);
        let event = thumbnail(&fake, &path, small, ThumbSize::Normal, "k", &maker);
        assert!(matches!(event, ThumbEvent::Ready { .. }));
        assert_eq!(maker.made.lock().unwrap()[0].1.len(), 100);
        // Known now: not read again.
        let reads = fake.memory().calls(MemOp::OpenRead);
        thumbnail(&fake, &path, small, ThumbSize::Normal, "k", &maker);
        assert_eq!(fake.memory().calls(MemOp::OpenRead), reads);

        let (fake, path) = server_with("big.png", &[1u8; 5000]);
        let event = thumbnail(&fake, &path, small, ThumbSize::Normal, "k", &maker);
        assert!(matches!(
            event,
            ThumbEvent::Skipped {
                why: SkipWhy::TooLarge,
                ..
            }
        ));
        assert_eq!(fake.memory().calls(MemOp::OpenRead), 0);
    }

    #[test]
    fn always_takes_a_large_photos_preview_from_its_first_bytes_only() {
        let jpeg = jpeg_with_preview(PREVIEW, true, 300_000);
        let (fake, path) = server_with("photo.jpg", &jpeg);
        let maker = Recorder::default();
        let always = policy(RemoteThumbnails::Always, 1000);
        let event = thumbnail(&fake, &path, always, ThumbSize::Normal, "k", &maker);
        assert!(matches!(event, ThumbEvent::Ready { .. }), "{event:?}");
        assert_eq!(maker.made.lock().unwrap()[0].1, PREVIEW);
        // A large file with no preview is skipped, not read whole.
        let (fake, path) = server_with("raw.png", &[3u8; 300_000]);
        let event = thumbnail(&fake, &path, always, ThumbSize::Normal, "k", &maker);
        assert!(matches!(
            event,
            ThumbEvent::Skipped {
                why: SkipWhy::TooLarge,
                ..
            }
        ));
    }

    struct OneServer(FakeRemoteProvider, Policy);

    impl Servers for OneServer {
        fn provider(&self, _: &VfsPath) -> Option<Arc<dyn Provider>> {
            Some(Arc::new(self.0.clone()))
        }

        fn policy(&self, _: &VfsPath) -> Policy {
            self.1
        }
    }

    #[test]
    fn a_cancelled_batch_stops_at_the_next_file() {
        let (fake, path) = server_with("a.png", &[1u8; 10]);
        fake.put_file(&fake.root("me@nas.lan").join("b.png").unwrap(), &[2u8; 10]);
        let servers = OneServer(fake, policy(RemoteThumbnails::SmallFiles, 1000));
        let maker = Recorder::default();
        let cancel = CancelToken::new();
        let sent = Mutex::new(Vec::new());
        let items = ["a.png", "b.png"]
            .iter()
            .map(|name| Remote {
                key: (*name).to_owned(),
                location: path.parent().unwrap().join(name).unwrap().to_location(),
            })
            .collect();
        run_batch(
            &servers,
            &maker,
            items,
            ThumbSize::Normal,
            &cancel,
            &|event| {
                sent.lock().unwrap().push(event.key().to_owned());
                cancel.cancel();
            },
        );
        assert_eq!(*sent.lock().unwrap(), ["a.png"]);
    }
}

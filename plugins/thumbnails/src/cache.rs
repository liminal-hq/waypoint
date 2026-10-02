// The thumbnail cache: the freedesktop.org Thumbnail Managing Standard layout, PNG `tEXt` metadata, staleness and the failure cache
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use md5::{Digest, Md5};
use percent_encoding::{percent_encode, AsciiSet, NON_ALPHANUMERIC};

use crate::models::ThumbSize;

pub const KEY_URI: &str = "Thumb::URI";
pub const KEY_MTIME: &str = "Thumb::MTime";
pub const KEY_SIZE: &str = "Thumb::Size";

/// Turns a path into the URI that names it in the cache. Injected, so tests never depend on the real file system layout.
pub type UriFn = Arc<dyn Fn(&Path) -> String + Send + Sync>;

/// The characters GLib's `g_filename_to_uri` leaves unescaped in a path, so the keys match the ones other file managers write.
const PATH_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/')
    .remove(b'!')
    .remove(b'$')
    .remove(b'&')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')')
    .remove(b'*')
    .remove(b'+')
    .remove(b',')
    .remove(b';')
    .remove(b'=')
    .remove(b':')
    .remove(b'@');

/// The `file://` URI of a path, percent-encoded byte by byte (so a name that is not UTF-8 still has one, and a distinct one).
pub fn file_uri(path: &Path) -> String {
    #[cfg(unix)]
    let encoded = {
        use std::os::unix::ffi::OsStrExt;
        percent_encode(path.as_os_str().as_bytes(), PATH_SAFE).to_string()
    };
    #[cfg(not(unix))]
    let encoded = {
        let text = path.to_string_lossy().replace('\\', "/");
        let text = if text.starts_with('/') {
            text
        } else {
            format!("/{text}")
        };
        percent_encode(text.as_bytes(), PATH_SAFE).to_string()
    };
    format!("file://{encoded}")
}

/// The lowercase hexadecimal MD5 of a string, which names the cache file.
pub fn md5_hex(text: &str) -> String {
    let digest = Md5::digest(text.as_bytes());
    let mut out = String::with_capacity(32);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The name of one cache entry: a size and the MD5 of the file's URI. This is all the `thumb://` scheme accepts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub size: ThumbSize,
    pub md5: String,
}

impl CacheKey {
    pub fn new(size: ThumbSize, uri: &str) -> Self {
        CacheKey {
            size,
            md5: md5_hex(uri),
        }
    }

    /// `normal/{md5}.png`, the path under the cache root and under the URL's host.
    pub fn relative(&self) -> String {
        format!("{}/{}.png", self.size.dir_name(), self.md5)
    }

    /// Parses `{size}/{md5}.png` (with or without a leading `/`) and nothing else: exactly a size folder name and 32 lowercase hexadecimal digits, so no `..`, no separator and no other name can pass.
    pub fn parse(text: &str) -> Option<CacheKey> {
        let text = text.strip_prefix('/').unwrap_or(text);
        let (dir, file) = text.split_once('/')?;
        let size = ThumbSize::from_dir_name(dir)?;
        let md5 = file.strip_suffix(".png")?;
        let valid = md5.len() == 32 && md5.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        valid.then(|| CacheKey {
            size,
            md5: md5.to_string(),
        })
    }
}

/// What a thumbnail records about the file it was made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub uri: String,
    /// The file's modified time in whole seconds, as the standard stores it.
    pub mtime_secs: i64,
    pub file_size: Option<u64>,
}

/// The modified time as the standard stores it: whole seconds, rounded towards negative infinity.
pub fn mtime_secs(mtime_ms: i64) -> i64 {
    mtime_ms.div_euclid(1000)
}

/// What the cache holds for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    /// A thumbnail made from this file as it is now.
    Fresh,
    /// A thumbnail exists but was made from another version of the file (or another file with the same name hash).
    Stale,
    /// A failure is recorded for this version of the file.
    Failed,
    Miss,
}

/// The cache directory and how to name things in it.
pub struct Store {
    root: PathBuf,
    fail_dir: PathBuf,
    uri_of: UriFn,
    counter: AtomicU64,
}

impl Store {
    /// A cache under `root` (the folder that holds `normal`, `large` and so on) that records failures under `fail/{app_name}-{app_version}`.
    pub fn new(root: PathBuf, app_name: &str, app_version: &str, uri_of: UriFn) -> Self {
        let fail_dir = root.join("fail").join(format!("{app_name}-{app_version}"));
        Store {
            root,
            fail_dir,
            uri_of,
            counter: AtomicU64::new(0),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn uri(&self, path: &Path) -> String {
        (self.uri_of)(path)
    }

    pub fn key_for(&self, size: ThumbSize, path: &Path) -> (CacheKey, String) {
        let uri = self.uri(path);
        (CacheKey::new(size, &uri), uri)
    }

    fn entry_path(&self, key: &CacheKey) -> PathBuf {
        self.root
            .join(key.size.dir_name())
            .join(format!("{}.png", key.md5))
    }

    fn fail_path(&self, key: &CacheKey) -> PathBuf {
        self.fail_dir.join(format!("{}.png", key.md5))
    }

    /// Checks the cache for a thumbnail of the file at `uri` modified at `mtime_secs`. Reads only the PNG header of the entry, so a hit is cheap.
    pub fn lookup(&self, key: &CacheKey, uri: &str, mtime_secs: i64) -> Lookup {
        match read_meta(&self.entry_path(key)) {
            Some(meta) if meta.uri == uri && meta.mtime_secs == mtime_secs => return Lookup::Fresh,
            Some(_) => return Lookup::Stale,
            None => {}
        }
        match read_meta(&self.fail_path(key)) {
            Some(meta) if meta.uri == uri && meta.mtime_secs == mtime_secs => Lookup::Failed,
            _ => Lookup::Miss,
        }
    }

    /// Writes an encoded thumbnail atomically with mode 0600, replacing any older one.
    pub fn store(&self, key: &CacheKey, png: &[u8]) -> io::Result<()> {
        self.write_atomic(&self.entry_path(key), png)
    }

    /// Records that making the thumbnail failed, as a one-pixel PNG with the same metadata.
    pub fn store_failure(&self, key: &CacheKey, meta: &Meta) -> io::Result<()> {
        let png = encode_png(1, 1, png::ColorType::Rgba, &[0, 0, 0, 0], meta)
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.write_atomic(&self.fail_path(key), &png)
    }

    /// A fresh path in the cache's own folder for something being made (a thumbnailer's output); the caller removes it.
    pub fn scratch_path(&self, size: ThumbSize) -> io::Result<PathBuf> {
        let dir = self.root.join(size.dir_name());
        create_private_dir(&dir)?;
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        Ok(dir.join(format!(".scratch-{}-{n}.png", std::process::id())))
    }

    /// Forgets a recorded failure (after a success).
    pub fn clear_failure(&self, key: &CacheKey) {
        let _ = fs::remove_file(self.fail_path(key));
    }

    /// Reads the bytes of an entry by key. Refuses anything that is not a regular file inside the cache (a symbolic link is not followed).
    pub fn read(&self, key: &CacheKey) -> Option<Vec<u8>> {
        let path = self.entry_path(key);
        let meta = fs::symlink_metadata(&path).ok()?;
        if !meta.is_file() {
            return None;
        }
        fs::read(path).ok()
    }

    fn write_atomic(&self, target: &Path, bytes: &[u8]) -> io::Result<()> {
        let dir = target.parent().expect("a cache entry has a folder");
        create_private_dir(dir)?;
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(".tmp-{}-{n}", std::process::id()));
        let result = (|| {
            let mut file = open_private(&tmp)?;
            file.write_all(bytes)?;
            file.flush()?;
            drop(file);
            fs::rename(&tmp, target)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }
}

fn create_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

fn open_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Reads the `Thumb::` text chunks of a PNG file without decoding its pixels; `None` if the file is missing, is not a PNG or lacks the URI or the time.
pub fn read_meta(path: &Path) -> Option<Meta> {
    let file = File::open(path).ok()?;
    let reader = png::Decoder::new(BufReader::new(file)).read_info().ok()?;
    meta_from_info(reader.info())
}

/// Reads the `Thumb::` text chunks of an encoded PNG.
pub fn meta_of_bytes(bytes: &[u8]) -> Option<Meta> {
    let reader = png::Decoder::new(io::Cursor::new(bytes)).read_info().ok()?;
    meta_from_info(reader.info())
}

fn meta_from_info(info: &png::Info<'_>) -> Option<Meta> {
    let find = |name: &str| {
        info.uncompressed_latin1_text
            .iter()
            .find(|chunk| chunk.keyword == name)
            .map(|chunk| chunk.text.clone())
    };
    Some(Meta {
        uri: find(KEY_URI)?,
        mtime_secs: find(KEY_MTIME)?.trim().parse().ok()?,
        file_size: find(KEY_SIZE).and_then(|text| text.trim().parse().ok()),
    })
}

/// Encodes pixels as a PNG that carries the `Thumb::URI`, `Thumb::MTime` and (when known) `Thumb::Size` chunks, written before the image data so a reader need not decode it.
pub fn encode_png(
    width: u32,
    height: u32,
    color: png::ColorType,
    pixels: &[u8],
    meta: &Meta,
) -> Result<Vec<u8>, png::EncodingError> {
    let mut out = Vec::with_capacity(pixels.len() / 3 + 512);
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.add_text_chunk(KEY_URI.to_string(), meta.uri.clone())?;
    encoder.add_text_chunk(KEY_MTIME.to_string(), meta.mtime_secs.to_string())?;
    if let Some(size) = meta.file_size {
        encoder.add_text_chunk(KEY_SIZE.to_string(), size.to_string())?;
    }
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels)?;
    writer.finish()?;
    Ok(out)
}

#[cfg(test)]
mod tests;

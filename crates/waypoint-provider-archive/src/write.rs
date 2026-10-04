// Making archives: zip, tar (plain, gzip, bzip2, xz) and 7z, written entry by entry to a stream.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{ArchiveBuilder, ArchiveKind, EntryAttrs, WriteStream};

use crate::errors::from_io;
use crate::names::os_name;

const DEFAULT_FILE_MODE: u32 = 0o644;
const DEFAULT_FOLDER_MODE: u32 = 0o755;
const S_IFLNK: u32 = 0o120_000;
const S_IFDIR: u32 = 0o040_000;
const S_IFREG: u32 = 0o100_000;
/// The largest file a zip entry holds without the zip64 extension.
const ZIP_SMALL_LIMIT: u64 = 0xffff_fffe;

fn io_failure(error: &io::Error, location: &Location) -> VfsError {
    // A typed error a source reader injected (a cancel, a lost connection) comes through as it was.
    from_io(error, location)
}

fn invalid_name(name: &[u8], why: &str) -> VfsError {
    VfsError::InvalidName {
        name: String::from_utf8_lossy(name).into_owned(),
        reason: why.to_owned(),
    }
}

/// A file's bytes, exactly `size` of them: a source that ends early or runs on has changed since it
/// was measured.
struct Exact<'a> {
    inner: io::Take<&'a mut dyn Read>,
    size: u64,
    read: u64,
}

impl<'a> Exact<'a> {
    fn new(data: &'a mut dyn Read, size: u64) -> Self {
        Self {
            inner: data.take(size),
            size,
            read: 0,
        }
    }

    /// Checks that the source held exactly `size` bytes, after they have been read.
    fn check(mut self, name: &[u8]) -> Result<(), VfsError> {
        if self.read < self.size {
            return Err(changed(name, "ended before its size"));
        }
        let mut extra = [0u8; 1];
        match self.inner.get_mut().read(&mut extra) {
            Ok(0) => Ok(()),
            Ok(_) => Err(changed(name, "grew while it was being read")),
            Err(_) => Ok(()),
        }
    }
}

impl Read for Exact<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.read += read as u64;
        Ok(read)
    }
}

fn changed(name: &[u8], what: &str) -> VfsError {
    VfsError::Io {
        message: format!("{} {what}", String::from_utf8_lossy(name)),
        location: None,
    }
}

fn mode_of(attrs: EntryAttrs, folder: bool) -> u32 {
    attrs.mode.map_or(
        if folder {
            DEFAULT_FOLDER_MODE
        } else {
            DEFAULT_FILE_MODE
        },
        |mode| mode & 0o7777,
    )
}

// ---- zip ----

/// Days since 1970-01-01 to a civil date.
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn zip_time(modified_ms: Option<i64>) -> Option<zip::DateTime> {
    let seconds = modified_ms?.div_euclid(1000);
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let rest = seconds.rem_euclid(86_400);
    // Zip's dates start in 1980 and have two-second steps.
    let year = u16::try_from(year).ok()?;
    zip::DateTime::from_date_and_time(
        year,
        month,
        day,
        (rest / 3600) as u8,
        ((rest % 3600) / 60) as u8,
        (rest % 60) as u8,
    )
    .ok()
}

/// Zip is built on disk and copied to the stream: the library's streaming mode writes a folder
/// entry in a way that `unzip` takes for overlapping entries, and a stream that cannot seek (a
/// server) cannot take the sizes back into the headers either.
struct ZipBuilder {
    zip: zip::ZipWriter<std::fs::File>,
    out: Box<dyn WriteStream>,
    location: Location,
}

impl ZipBuilder {
    fn options(&self, attrs: EntryAttrs, folder: bool, size: u64) -> zip::write::SimpleFileOptions {
        let mut options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(mode_of(attrs, folder))
            .large_file(size > ZIP_SMALL_LIMIT);
        if let Some(time) = zip_time(attrs.modified_ms) {
            options = options.last_modified_time(time);
        }
        options
    }

    fn name(name: &[u8]) -> Result<String, VfsError> {
        String::from_utf8(name.to_vec()).map_err(|_| {
            invalid_name(
                name,
                "a zip archive holds only names that are valid Unicode",
            )
        })
    }

    fn fail(&self, error: zip::result::ZipError) -> VfsError {
        match error {
            zip::result::ZipError::Io(io) => io_failure(&io, &self.location),
            other => VfsError::Io {
                message: other.to_string(),
                location: Some(self.location.clone()),
            },
        }
    }
}

impl ArchiveBuilder for ZipBuilder {
    fn add_dir(&mut self, name: &[u8], attrs: EntryAttrs) -> Result<(), VfsError> {
        let text = Self::name(name)?;
        let options = self.options(attrs, true, 0);
        self.zip
            .add_directory(text, options)
            .map_err(|e| self.fail(e))
    }

    fn add_file(
        &mut self,
        name: &[u8],
        size: u64,
        attrs: EntryAttrs,
        data: &mut dyn Read,
    ) -> Result<(), VfsError> {
        let text = Self::name(name)?;
        let options = self.options(attrs, false, size);
        self.zip
            .start_file(text, options)
            .map_err(|e| self.fail(e))?;
        let mut exact = Exact::new(data, size);
        io::copy(&mut exact, &mut self.zip).map_err(|e| io_failure(&e, &self.location))?;
        exact.check(name)
    }

    fn add_symlink(
        &mut self,
        name: &[u8],
        target: &[u8],
        attrs: EntryAttrs,
    ) -> Result<(), VfsError> {
        let text = Self::name(name)?;
        let target = String::from_utf8(target.to_vec()).map_err(|_| {
            invalid_name(
                target,
                "a zip link holds only a target that is valid Unicode",
            )
        })?;
        let options = self.options(attrs, false, 0);
        self.zip
            .add_symlink(text, target, options)
            .map_err(|e| self.fail(e))
    }

    fn finish(self: Box<Self>, sync: bool) -> Result<(), VfsError> {
        let Self {
            zip,
            mut out,
            location,
        } = *self;
        let mut file = zip.finish().map_err(|e| match e {
            zip::result::ZipError::Io(io) => io_failure(&io, &location),
            other => VfsError::Io {
                message: other.to_string(),
                location: Some(location.clone()),
            },
        })?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| io_failure(&e, &location))?;
        io::copy(&mut file, &mut out).map_err(|e| io_failure(&e, &location))?;
        out.finish(sync)
    }
}

// ---- tar ----

/// The encoder a tar is written through, so `finish` can close it and hand back the stream.
enum Encoder {
    Plain(Box<dyn WriteStream>),
    Gzip(flate2::write::GzEncoder<Box<dyn WriteStream>>),
    Bzip2(bzip2::write::BzEncoder<Box<dyn WriteStream>>),
    Xz(Box<lzma_rust2::XzWriter<Box<dyn WriteStream>>>),
}

impl Write for Encoder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(out) => out.write(buf),
            Self::Gzip(out) => out.write(buf),
            Self::Bzip2(out) => out.write(buf),
            Self::Xz(out) => out.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(out) => out.flush(),
            Self::Gzip(out) => out.flush(),
            Self::Bzip2(out) => out.flush(),
            Self::Xz(out) => out.flush(),
        }
    }
}

impl Encoder {
    /// Ends the compressed stream and returns the stream under it.
    fn close(self) -> io::Result<Box<dyn WriteStream>> {
        match self {
            Self::Plain(out) => Ok(out),
            Self::Gzip(out) => out.finish(),
            Self::Bzip2(out) => out.finish(),
            Self::Xz(out) => out.finish().map_err(|e| io::Error::other(e.to_string())),
        }
    }
}

struct TarBuilder {
    tar: tar::Builder<Encoder>,
    location: Location,
}

impl TarBuilder {
    fn header(
        &self,
        entry: tar::EntryType,
        mode: u32,
        size: u64,
        attrs: EntryAttrs,
    ) -> tar::Header {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(entry);
        header.set_mode(mode);
        header.set_size(size);
        header.set_mtime(
            attrs
                .modified_ms
                .map_or(0, |ms| u64::try_from(ms.div_euclid(1000)).unwrap_or(0)),
        );
        header
    }

    fn path(name: &[u8]) -> PathBuf {
        PathBuf::from(os_name(name))
    }

    fn fail(&self, error: &io::Error) -> VfsError {
        io_failure(error, &self.location)
    }
}

impl ArchiveBuilder for TarBuilder {
    fn add_dir(&mut self, name: &[u8], attrs: EntryAttrs) -> Result<(), VfsError> {
        let mut header = self.header(tar::EntryType::Directory, mode_of(attrs, true), 0, attrs);
        let mut path = Self::path(name);
        path.push("");
        self.tar
            .append_data(&mut header, path, io::empty())
            .map_err(|e| self.fail(&e))
    }

    fn add_file(
        &mut self,
        name: &[u8],
        size: u64,
        attrs: EntryAttrs,
        data: &mut dyn Read,
    ) -> Result<(), VfsError> {
        let mut header = self.header(tar::EntryType::Regular, mode_of(attrs, false), size, attrs);
        let mut exact = Exact::new(data, size);
        // The tar crate writes the header, then exactly `size` bytes (it errors if fewer arrive).
        self.tar
            .append_data(&mut header, Self::path(name), &mut exact)
            .map_err(|e| {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    changed(name, "ended before its size")
                } else {
                    self.fail(&e)
                }
            })?;
        exact.check(name)
    }

    fn add_symlink(
        &mut self,
        name: &[u8],
        target: &[u8],
        attrs: EntryAttrs,
    ) -> Result<(), VfsError> {
        let mut header = self.header(
            tar::EntryType::Symlink,
            mode_of(attrs, false) | 0o111,
            0,
            attrs,
        );
        self.tar
            .append_link(
                &mut header,
                Self::path(name),
                PathBuf::from(os_name(target)),
            )
            .map_err(|e| self.fail(&e))
    }

    fn finish(self: Box<Self>, sync: bool) -> Result<(), VfsError> {
        let location = self.location.clone();
        let encoder = self
            .tar
            .into_inner()
            .map_err(|e| io_failure(&e, &location))?;
        let stream = encoder.close().map_err(|e| io_failure(&e, &location))?;
        stream.finish(sync)
    }
}

// ---- 7z ----

const WINDOWS_UNIX_EXTENSION: u32 = 0x8000;
const WINDOWS_DIRECTORY: u32 = 0x10;
/// 100 ns ticks from 1601-01-01 to 1970-01-01, in milliseconds.
const NT_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

struct SevenZBuilder {
    writer: sevenz_rust2::ArchiveWriter<std::fs::File>,
    out: Box<dyn WriteStream>,
    location: Location,
}

impl SevenZBuilder {
    fn fail(&self, error: sevenz_rust2::Error) -> VfsError {
        match error {
            sevenz_rust2::Error::Io(io, _) | sevenz_rust2::Error::FileOpen(io, _) => {
                io_failure(&io, &self.location)
            }
            other => VfsError::Io {
                message: other.to_string(),
                location: Some(self.location.clone()),
            },
        }
    }

    fn entry(
        name: &[u8],
        attrs: EntryAttrs,
        folder: bool,
        link: bool,
    ) -> Result<sevenz_rust2::ArchiveEntry, VfsError> {
        let text = String::from_utf8(name.to_vec()).map_err(|_| {
            invalid_name(name, "a 7z archive holds only names that are valid Unicode")
        })?;
        let mut entry = if folder {
            sevenz_rust2::ArchiveEntry::new_directory(&text)
        } else {
            sevenz_rust2::ArchiveEntry::new_file(&text)
        };
        let kind = if link {
            S_IFLNK
        } else if folder {
            S_IFDIR
        } else {
            S_IFREG
        };
        let mode = mode_of(attrs, folder) | kind;
        entry.has_windows_attributes = true;
        entry.windows_attributes =
            (mode << 16) | WINDOWS_UNIX_EXTENSION | if folder { WINDOWS_DIRECTORY } else { 0 };
        if let Some(ms) = attrs.modified_ms {
            if let Ok(ticks) = u64::try_from((ms + NT_EPOCH_OFFSET_MS).saturating_mul(10_000)) {
                entry.has_last_modified_date = true;
                entry.last_modified_date = sevenz_rust2::NtTime::from(ticks);
            }
        }
        Ok(entry)
    }
}

impl ArchiveBuilder for SevenZBuilder {
    fn add_dir(&mut self, name: &[u8], attrs: EntryAttrs) -> Result<(), VfsError> {
        let entry = Self::entry(name, attrs, true, false)?;
        self.writer
            .push_archive_entry::<&[u8]>(entry, None)
            .map(|_| ())
            .map_err(|e| self.fail(e))
    }

    fn add_file(
        &mut self,
        name: &[u8],
        size: u64,
        attrs: EntryAttrs,
        data: &mut dyn Read,
    ) -> Result<(), VfsError> {
        let entry = Self::entry(name, attrs, false, false)?;
        let mut exact = Exact::new(data, size);
        self.writer
            .push_archive_entry(entry, Some(&mut exact))
            .map(|_| ())
            .map_err(|e| self.fail(e))?;
        exact.check(name)
    }

    fn add_symlink(
        &mut self,
        name: &[u8],
        target: &[u8],
        attrs: EntryAttrs,
    ) -> Result<(), VfsError> {
        let entry = Self::entry(name, attrs, false, true)?;
        self.writer
            .push_archive_entry(entry, Some(target))
            .map(|_| ())
            .map_err(|e| self.fail(e))
    }

    fn finish(self: Box<Self>, sync: bool) -> Result<(), VfsError> {
        let Self {
            writer,
            mut out,
            location,
        } = *self;
        let mut file = writer.finish().map_err(|e| io_failure(&e, &location))?;
        // 7z's header goes back to the start of the file, so the archive is built on disk and
        // copied to the stream, which cannot seek (a server).
        file.seek(SeekFrom::Start(0))
            .map_err(|e| io_failure(&e, &location))?;
        io::copy(&mut file, &mut out).map_err(|e| io_failure(&e, &location))?;
        out.finish(sync)
    }
}

fn scratch_file(scratch: Option<&std::path::Path>) -> io::Result<std::fs::File> {
    match scratch {
        Some(dir) => tempfile::tempfile_in(dir),
        None => tempfile::tempfile(),
    }
}

/// Starts an archive of `kind` written to `out`. `scratch` is where a zip or 7z archive is built.
pub(crate) fn begin(
    kind: ArchiveKind,
    out: Box<dyn WriteStream>,
    location: Location,
    scratch: Option<&std::path::Path>,
) -> Result<Box<dyn ArchiveBuilder>, VfsError> {
    let tar = |encoder: Encoder, location: Location| -> Box<dyn ArchiveBuilder> {
        let mut tar = tar::Builder::new(encoder);
        tar.mode(tar::HeaderMode::Complete);
        Box::new(TarBuilder { tar, location })
    };
    Ok(match kind {
        ArchiveKind::Zip => {
            let file = scratch_file(scratch).map_err(|e| io_failure(&e, &location))?;
            Box::new(ZipBuilder {
                zip: zip::ZipWriter::new(file),
                out,
                location,
            })
        }
        ArchiveKind::Tar => tar(Encoder::Plain(out), location),
        ArchiveKind::TarGz => tar(
            Encoder::Gzip(flate2::write::GzEncoder::new(
                out,
                flate2::Compression::default(),
            )),
            location,
        ),
        ArchiveKind::TarBz2 => tar(
            Encoder::Bzip2(bzip2::write::BzEncoder::new(
                out,
                bzip2::Compression::default(),
            )),
            location,
        ),
        ArchiveKind::TarXz => {
            let writer = lzma_rust2::XzWriter::new(out, lzma_rust2::XzOptions::with_preset(6))
                .map_err(|e| io_failure(&io::Error::other(e.to_string()), &location))?;
            tar(Encoder::Xz(Box::new(writer)), location)
        }
        ArchiveKind::SevenZ => {
            let file = scratch_file(scratch).map_err(|e| io_failure(&e, &location))?;
            let writer = sevenz_rust2::ArchiveWriter::new(file).map_err(|e| match e {
                sevenz_rust2::Error::Io(io, _) => io_failure(&io, &location),
                other => VfsError::Io {
                    message: other.to_string(),
                    location: Some(location.clone()),
                },
            })?;
            Box::new(SevenZBuilder {
                writer,
                out,
                location,
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip_known_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    #[test]
    fn a_zip_time_before_1980_is_dropped() {
        assert!(zip_time(Some(0)).is_none());
        assert!(zip_time(None).is_none());
        let time = zip_time(Some(1_704_110_400_000)).unwrap();
        assert_eq!(
            (time.year(), time.month(), time.day(), time.hour()),
            (2024, 1, 1, 12)
        );
    }
}

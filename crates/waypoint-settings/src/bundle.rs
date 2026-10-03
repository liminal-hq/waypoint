// The settings export file: a `.json` for one configuration file, a `.zip` for several
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Both shapes carry the same logical content: a format name and version, the app version and
// time of the export, and the configuration files by id.
//
// - **`.json`**: one object, `{ format, version, appVersion, exportedAt, files: { <id>: <document> } }`.
// - **`.zip`**: `manifest.json` (`{ format, version, appVersion, exportedAt, files: [{ id, name, size }] }`)
//   and one `<name>` entry per file holding that file's document.
//
// Reading is defensive: the input is bytes from anywhere. Sizes are capped, a zip is read only
// through the names its manifest lists, entries are never written anywhere by name, and every
// failure is an error (nothing here panics on corrupt, truncated or hostile input).

use std::io::{Cursor, Read, Write};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;
use ts_rs::TS;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// The `format` both shapes carry.
pub const BUNDLE_FORMAT: &str = "waypoint-settings";

/// The bundle layout this build writes and reads. A file with a higher version is refused.
pub const BUNDLE_VERSION: u32 = 1;

/// The zip entry that lists the other entries.
pub const MANIFEST_NAME: &str = "manifest.json";

/// The most one configuration file's document may take, as JSON text.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;

/// The most an import reads: the file's own size, and the zip's entries once uncompressed.
pub const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;

/// The most entries a zip may hold (the manifest included), and the most files a bundle lists.
pub const MAX_ENTRIES: usize = 16;

/// The longest a zip entry name or a file id may be.
const MAX_NAME_LEN: usize = 64;

/// Which shape an export took, or an import was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum BundleKind {
    Json,
    Zip,
}

impl BundleKind {
    pub fn extension(self) -> &'static str {
        match self {
            BundleKind::Json => "json",
            BundleKind::Zip => "zip",
        }
    }
}

/// One configuration file to export: its id (`settings`) and its document.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportFile {
    pub id: String,
    pub document: Value,
}

/// What an export records about the moment and the build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportMeta {
    pub app_version: String,
    /// Seconds since the Unix epoch, recorded as UTC in the file.
    pub exported_at_unix: u64,
    /// The person's offset from UTC, so the date in the suggested name is their local date.
    pub local_offset_minutes: i32,
}

/// An export ready to be written: the bytes, how they are packed, and a name to offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedBundle {
    pub suggested_name: String,
    pub bytes: Vec<u8>,
    pub kind: BundleKind,
}

/// A bundle read from bytes: the header and every configuration file, not yet checked against
/// what each file means.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundle {
    pub kind: BundleKind,
    pub app_version: String,
    pub exported_at: String,
    pub files: Vec<ExportFile>,
    /// Zip entries that are not listed in the manifest and so were not read.
    pub unlisted_entries: Vec<String>,
}

/// Why a bundle could not be written, read or applied. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BundleError {
    #[error("there is nothing to export")]
    NothingToExport,
    #[error("`{0}` is not a usable file id")]
    BadId(String),
    #[error("the file is not a Waypoint settings file")]
    NotABundle,
    #[error("the file is damaged or cut short: {0}")]
    Corrupt(String),
    #[error("the file was made by a newer Waypoint (format {found}); this version reads format {supported}")]
    NewerFormat { found: u64, supported: u32 },
    #[error("{what} is larger than the {limit} byte limit")]
    TooLarge { what: String, limit: usize },
    #[error("the archive holds too many entries (at most {MAX_ENTRIES})")]
    TooManyEntries,
    #[error("the archive has an entry name that is not allowed: `{0}`")]
    UnsafeName(String),
    #[error("the archive lists `{0}` more than once")]
    DuplicateName(String),
    #[error("the archive is missing `{0}`")]
    MissingEntry(String),
    #[error("`{file}` is not valid: {message}")]
    Invalid { file: String, message: String },
    #[error("`{file}` was saved by a newer Waypoint (version {found}); this version reads version {supported}")]
    NewerDocument {
        file: String,
        found: u64,
        supported: u32,
    },
    #[error("the file has no settings this version of Waypoint recognises")]
    NothingRecognised,
}

impl BundleError {
    /// The stable name the page tells refusals apart by.
    pub fn code(&self) -> &'static str {
        match self {
            BundleError::NothingToExport | BundleError::BadId(_) => "nothing",
            BundleError::NotABundle | BundleError::NothingRecognised => "not-a-bundle",
            BundleError::Corrupt(_) | BundleError::MissingEntry(_) => "corrupt",
            BundleError::NewerFormat { .. } | BundleError::NewerDocument { .. } => "newer-format",
            BundleError::TooLarge { .. } | BundleError::TooManyEntries => "too-large",
            BundleError::UnsafeName(_) | BundleError::DuplicateName(_) => "unsafe",
            BundleError::Invalid { .. } => "invalid",
        }
    }
}

/// Whether `id` can name a configuration file: lower-case letters, digits and hyphens.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_NAME_LEN
        && !id.starts_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether `name` is safe as a zip entry name: a plain file name ending in `.json`, with no
/// directory part, drive, control character or dot-only name.
fn safe_entry_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name != "."
        && name != ".."
        && !name.starts_with('.')
        && name.ends_with(".json")
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
}

/// Writes `files` as one `.json` when there is exactly one, and as a `.zip` when there are more.
pub fn export(files: &[ExportFile], meta: &ExportMeta) -> Result<ExportedBundle, BundleError> {
    if files.is_empty() {
        return Err(BundleError::NothingToExport);
    }
    if files.len() >= MAX_ENTRIES {
        return Err(BundleError::TooManyEntries);
    }
    let mut seen = Vec::new();
    let mut documents = Vec::new();
    let mut total = 0usize;
    for file in files {
        if !valid_id(&file.id) {
            return Err(BundleError::BadId(file.id.clone()));
        }
        if seen.contains(&file.id) {
            return Err(BundleError::DuplicateName(file.id.clone()));
        }
        seen.push(file.id.clone());
        let bytes = serde_json::to_vec_pretty(&file.document)
            .map_err(|e| BundleError::Corrupt(e.to_string()))?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(BundleError::TooLarge {
                what: format!("`{}`", file.id),
                limit: MAX_FILE_BYTES,
            });
        }
        total += bytes.len();
        documents.push(bytes);
    }
    if total > MAX_TOTAL_BYTES {
        return Err(BundleError::TooLarge {
            what: "the export".to_owned(),
            limit: MAX_TOTAL_BYTES,
        });
    }
    let exported_at = rfc3339(meta.exported_at_unix);
    let kind = if files.len() == 1 {
        BundleKind::Json
    } else {
        BundleKind::Zip
    };
    let bytes = match kind {
        BundleKind::Json => {
            let mut map = Map::new();
            for file in files {
                map.insert(file.id.clone(), file.document.clone());
            }
            let envelope = json!({
                "format": BUNDLE_FORMAT,
                "version": BUNDLE_VERSION,
                "appVersion": meta.app_version,
                "exportedAt": exported_at,
                "files": Value::Object(map),
            });
            serde_json::to_vec_pretty(&envelope).map_err(|e| BundleError::Corrupt(e.to_string()))?
        }
        BundleKind::Zip => {
            let listing: Vec<Value> = files
                .iter()
                .zip(&documents)
                .map(|(file, bytes)| {
                    json!({ "id": file.id, "name": format!("{}.json", file.id), "size": bytes.len() })
                })
                .collect();
            let manifest = json!({
                "format": BUNDLE_FORMAT,
                "version": BUNDLE_VERSION,
                "appVersion": meta.app_version,
                "exportedAt": exported_at,
                "files": listing,
            });
            let manifest = serde_json::to_vec_pretty(&manifest)
                .map_err(|e| BundleError::Corrupt(e.to_string()))?;
            let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            let zip_error = |e: &dyn std::fmt::Display| BundleError::Corrupt(e.to_string());
            writer
                .start_file(MANIFEST_NAME, options)
                .map_err(|e| zip_error(&e))?;
            writer.write_all(&manifest).map_err(|e| zip_error(&e))?;
            for (file, bytes) in files.iter().zip(&documents) {
                writer
                    .start_file(format!("{}.json", file.id), options)
                    .map_err(|e| zip_error(&e))?;
                writer.write_all(bytes).map_err(|e| zip_error(&e))?;
            }
            writer.finish().map_err(|e| zip_error(&e))?.into_inner()
        }
    };
    Ok(ExportedBundle {
        suggested_name: format!(
            "waypoint-settings-{}.{}",
            local_date(meta.exported_at_unix, meta.local_offset_minutes),
            kind.extension()
        ),
        bytes,
        kind,
    })
}

/// Reads a bundle from `bytes`, telling a zip from a JSON file by its first bytes.
pub fn read(bytes: &[u8]) -> Result<Bundle, BundleError> {
    if bytes.len() > MAX_TOTAL_BYTES {
        return Err(BundleError::TooLarge {
            what: "the file".to_owned(),
            limit: MAX_TOTAL_BYTES,
        });
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return read_zip(bytes);
    }
    let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if text.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        return read_json(text);
    }
    Err(BundleError::NotABundle)
}

fn parse(bytes: &[u8]) -> Result<Value, BundleError> {
    serde_json::from_slice(bytes).map_err(|e| BundleError::Corrupt(e.to_string()))
}

/// Checks the header every shape shares: our format name, and a version this build can read.
fn check_header(header: &Value) -> Result<(String, String), BundleError> {
    let object = header.as_object().ok_or(BundleError::NotABundle)?;
    if object.get("format").and_then(Value::as_str) != Some(BUNDLE_FORMAT) {
        return Err(BundleError::NotABundle);
    }
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .filter(|v| *v >= 1)
        .ok_or_else(|| BundleError::Corrupt("the format version is missing".to_owned()))?;
    if version > u64::from(BUNDLE_VERSION) {
        return Err(BundleError::NewerFormat {
            found: version,
            supported: BUNDLE_VERSION,
        });
    }
    let text = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_control())
            .take(64)
            .collect::<String>()
    };
    Ok((text("appVersion"), text("exportedAt")))
}

fn check_document_size(id: &str, document: &Value) -> Result<(), BundleError> {
    let size = serde_json::to_vec(document).map_or(usize::MAX, |bytes| bytes.len());
    if size > MAX_FILE_BYTES {
        return Err(BundleError::TooLarge {
            what: format!("`{id}`"),
            limit: MAX_FILE_BYTES,
        });
    }
    Ok(())
}

fn read_json(bytes: &[u8]) -> Result<Bundle, BundleError> {
    let value = parse(bytes)?;
    let (app_version, exported_at) = check_header(&value)?;
    let files = value
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| BundleError::Corrupt("the file list is missing".to_owned()))?;
    if files.len() >= MAX_ENTRIES {
        return Err(BundleError::TooManyEntries);
    }
    let mut out = Vec::new();
    for (id, document) in files {
        if !valid_id(id) {
            return Err(BundleError::BadId(id.clone()));
        }
        check_document_size(id, document)?;
        out.push(ExportFile {
            id: id.clone(),
            document: document.clone(),
        });
    }
    Ok(Bundle {
        kind: BundleKind::Json,
        app_version,
        exported_at,
        files: out,
        unlisted_entries: Vec::new(),
    })
}

/// Reads at most `limit` bytes of an entry, failing when there is more than that however large
/// the entry's own header says it is.
fn read_limited(entry: &mut impl Read, name: &str, limit: usize) -> Result<Vec<u8>, BundleError> {
    let mut out = Vec::new();
    entry
        .take(limit as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| BundleError::Corrupt(format!("`{name}` could not be read: {e}")))?;
    if out.len() > limit {
        return Err(BundleError::TooLarge {
            what: format!("`{name}`"),
            limit,
        });
    }
    Ok(out)
}

/// The entry names in the zip's central directory, in order. The zip library keeps one entry
/// per name and drops the others without saying, so a repeated name has to be found here.
fn central_names(bytes: &[u8]) -> Result<Vec<String>, BundleError> {
    let cut = || BundleError::Corrupt("the archive's directory is cut short".to_owned());
    let u16_at = |at: usize| -> Result<usize, BundleError> {
        let b = bytes.get(at..at + 2).ok_or_else(cut)?;
        Ok(usize::from(u16::from_le_bytes([b[0], b[1]])))
    };
    let u32_at = |at: usize| -> Result<usize, BundleError> {
        let b = bytes.get(at..at + 4).ok_or_else(cut)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    // The end-of-directory record is the last 22 or more bytes (a comment may follow it).
    let last = bytes.len().checked_sub(22).ok_or_else(cut)?;
    let earliest = last.saturating_sub(usize::from(u16::MAX));
    let end = (earliest..=last)
        .rev()
        .find(|&at| bytes.get(at..at + 4) == Some(b"PK\x05\x06"))
        .ok_or_else(cut)?;
    let count = u16_at(end + 10)?;
    if count > MAX_ENTRIES {
        return Err(BundleError::TooManyEntries);
    }
    let mut at = u32_at(end + 16)?;
    let mut names = Vec::new();
    for _ in 0..count {
        if bytes.get(at..at + 4) != Some(b"PK\x01\x02") {
            return Err(cut());
        }
        let name_len = u16_at(at + 28)?;
        let extra_len = u16_at(at + 30)?;
        let comment_len = u16_at(at + 32)?;
        let name = bytes.get(at + 46..at + 46 + name_len).ok_or_else(cut)?;
        names.push(String::from_utf8_lossy(name).into_owned());
        at += 46 + name_len + extra_len + comment_len;
    }
    Ok(names)
}

fn read_zip(bytes: &[u8]) -> Result<Bundle, BundleError> {
    let corrupt = |e: &dyn std::fmt::Display| BundleError::Corrupt(e.to_string());
    // Every name is judged, listed or not, before anything is read.
    let names = central_names(bytes)?;
    for (at, name) in names.iter().enumerate() {
        if name != MANIFEST_NAME && !safe_entry_name(name) {
            return Err(BundleError::UnsafeName(name.clone()));
        }
        if names[..at].contains(name) {
            return Err(BundleError::DuplicateName(name.clone()));
        }
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|e| corrupt(&e))?;
    if archive.len() != names.len() {
        return Err(BundleError::Corrupt(
            "the archive's directory does not agree with itself".to_owned(),
        ));
    }
    let index_of = |name: &str| names.iter().position(|n| n == name);
    let mut total = 0usize;
    let mut read_entry = |archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str| {
        let index = index_of(name).ok_or_else(|| BundleError::MissingEntry(name.to_owned()))?;
        let mut entry = archive.by_index(index).map_err(|e| corrupt(&e))?;
        if entry.size() > MAX_FILE_BYTES as u64 {
            return Err(BundleError::TooLarge {
                what: format!("`{name}`"),
                limit: MAX_FILE_BYTES,
            });
        }
        let data = read_limited(&mut entry, name, MAX_FILE_BYTES)?;
        total += data.len();
        if total > MAX_TOTAL_BYTES {
            return Err(BundleError::TooLarge {
                what: "the archive once unpacked".to_owned(),
                limit: MAX_TOTAL_BYTES,
            });
        }
        Ok(data)
    };

    let manifest = parse(&read_entry(&mut archive, MANIFEST_NAME)?)?;
    let (app_version, exported_at) = check_header(&manifest)?;
    let listed = manifest
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| BundleError::Corrupt("the manifest lists no files".to_owned()))?;
    if listed.len() >= MAX_ENTRIES {
        return Err(BundleError::TooManyEntries);
    }
    let mut files: Vec<ExportFile> = Vec::new();
    let mut read_names: Vec<String> = vec![MANIFEST_NAME.to_owned()];
    for item in listed {
        let field = |key: &str| item.get(key).and_then(Value::as_str);
        let (Some(id), Some(name)) = (field("id"), field("name")) else {
            return Err(BundleError::Corrupt(
                "a manifest entry has no id or name".to_owned(),
            ));
        };
        if !valid_id(id) {
            return Err(BundleError::BadId(id.to_owned()));
        }
        if !safe_entry_name(name) {
            return Err(BundleError::UnsafeName(name.to_owned()));
        }
        if read_names.iter().any(|n| n == name) || files.iter().any(|f| f.id == id) {
            return Err(BundleError::DuplicateName(name.to_owned()));
        }
        let data = read_entry(&mut archive, name)?;
        if let Some(size) = item.get("size").and_then(Value::as_u64) {
            if size != data.len() as u64 {
                return Err(BundleError::Corrupt(format!(
                    "`{name}` is not the size the manifest records"
                )));
            }
        }
        read_names.push(name.to_owned());
        files.push(ExportFile {
            id: id.to_owned(),
            document: parse(&data)?,
        });
    }
    let unlisted_entries = names
        .into_iter()
        .filter(|name| !read_names.contains(name))
        .collect();
    Ok(Bundle {
        kind: BundleKind::Zip,
        app_version,
        exported_at,
        files,
        unlisted_entries,
    })
}

/// The proleptic Gregorian date of a day count since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// `2026-10-03T14:05:09Z`.
fn rfc3339(unix: u64) -> String {
    let unix = unix.min(i64::MAX as u64 / 4) as i64;
    let (year, month, day) = civil_from_days(unix.div_euclid(86_400));
    let seconds = unix.rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

/// `2026-10-03`, the date at the person's offset from UTC.
fn local_date(unix: u64, offset_minutes: i32) -> String {
    let unix =
        unix.min(i64::MAX as u64 / 4) as i64 + i64::from(offset_minutes.clamp(-1440, 1440)) * 60;
    let (year, month, day) = civil_from_days(unix.div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-03T02:30:00Z
    const NOW: u64 = 1_790_994_600;

    fn meta() -> ExportMeta {
        ExportMeta {
            app_version: "0.1.0".to_owned(),
            exported_at_unix: NOW,
            local_offset_minutes: 0,
        }
    }

    fn file(id: &str, value: Value) -> ExportFile {
        ExportFile {
            id: id.to_owned(),
            document: value,
        }
    }

    /// A zip built by hand, so the tests can make shapes the writer never would.
    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn manifest(files: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "format": BUNDLE_FORMAT, "version": 1, "appVersion": "0.1.0",
            "exportedAt": "2026-10-03T02:30:00Z", "files": files,
        }))
        .unwrap()
    }

    #[test]
    fn dates_are_civil_dates() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(NOW), "2026-10-03T02:30:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(local_date(NOW, 0), "2026-10-03");
        assert_eq!(local_date(NOW, -240), "2026-10-02", "evening in Toronto");
        assert_eq!(local_date(NOW, 600), "2026-10-03");
    }

    #[test]
    fn one_file_is_a_json_with_the_document_under_files() {
        let out = export(
            &[file("settings", json!({"version": 1, "body": {}}))],
            &meta(),
        )
        .unwrap();
        assert_eq!(out.kind, BundleKind::Json);
        assert_eq!(out.suggested_name, "waypoint-settings-2026-10-03.json");
        let value: Value = serde_json::from_slice(&out.bytes).unwrap();
        assert_eq!(value["format"], "waypoint-settings");
        assert_eq!(value["version"], 1);
        assert_eq!(value["appVersion"], "0.1.0");
        assert_eq!(value["exportedAt"], "2026-10-03T02:30:00Z");
        assert_eq!(value["files"]["settings"]["version"], 1);
    }

    #[test]
    fn several_files_are_a_zip_with_a_manifest_and_one_entry_each() {
        let out = export(
            &[
                file("settings", json!({"a": 1})),
                file("ops", json!({"b": 2})),
            ],
            &meta(),
        )
        .unwrap();
        assert_eq!(out.kind, BundleKind::Zip);
        assert_eq!(out.suggested_name, "waypoint-settings-2026-10-03.zip");
        assert!(out.bytes.starts_with(b"PK\x03\x04"));
        let mut archive = ZipArchive::new(Cursor::new(out.bytes.as_slice())).unwrap();
        let names: Vec<String> = archive.file_names().map(str::to_owned).collect();
        assert_eq!(names, ["manifest.json", "settings.json", "ops.json"]);
        let mut text = String::new();
        archive
            .by_name("manifest.json")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        let manifest: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(manifest["files"][1]["id"], "ops");
        assert_eq!(manifest["files"][1]["name"], "ops.json");
        assert!(manifest["files"][1]["size"].as_u64().unwrap() > 0);
    }

    #[test]
    fn nothing_to_export_and_bad_ids_are_refused() {
        assert_eq!(export(&[], &meta()), Err(BundleError::NothingToExport));
        for id in ["", "Settings", "../x", "a/b", "-a", "a b"] {
            assert!(
                matches!(
                    export(&[file(id, json!({}))], &meta()),
                    Err(BundleError::BadId(_))
                ),
                "{id}"
            );
        }
        assert!(matches!(
            export(&[file("a", json!({})), file("a", json!({}))], &meta()),
            Err(BundleError::DuplicateName(_))
        ));
    }

    #[test]
    fn a_document_over_the_cap_is_not_exported() {
        let big = json!({ "text": "x".repeat(MAX_FILE_BYTES) });
        assert!(matches!(
            export(&[file("settings", big)], &meta()),
            Err(BundleError::TooLarge { .. })
        ));
    }

    #[test]
    fn json_and_zip_round_trip_to_the_same_files() {
        let one = vec![file(
            "settings",
            json!({"version": 1, "body": {"x": [1, 2]}}),
        )];
        let two = vec![one[0].clone(), file("ops", json!({"concurrency": 3}))];
        for files in [one, two] {
            let out = export(&files, &meta()).unwrap();
            let bundle = read(&out.bytes).unwrap();
            assert_eq!(bundle.kind, out.kind);
            assert_eq!(bundle.files, files);
            assert_eq!(bundle.app_version, "0.1.0");
            assert_eq!(bundle.exported_at, "2026-10-03T02:30:00Z");
            assert!(bundle.unlisted_entries.is_empty());
        }
    }

    #[test]
    fn a_json_with_a_byte_order_mark_and_leading_space_reads() {
        let out = export(&[file("settings", json!({}))], &meta()).unwrap();
        let mut bytes = b"\xEF\xBB\xBF  \n".to_vec();
        bytes.extend(out.bytes);
        assert_eq!(read(&bytes).unwrap().files.len(), 1);
    }

    #[test]
    fn what_is_not_a_bundle_is_refused() {
        assert_eq!(read(b""), Err(BundleError::NotABundle));
        assert_eq!(read(b"hello"), Err(BundleError::NotABundle));
        assert_eq!(read(b"\x89PNG\r\n"), Err(BundleError::NotABundle));
        assert_eq!(read(b"[1, 2]"), Err(BundleError::NotABundle));
        assert_eq!(
            read(br#"{"format": "something-else"}"#),
            Err(BundleError::NotABundle)
        );
        assert_eq!(read(b"{}"), Err(BundleError::NotABundle));
    }

    #[test]
    fn truncated_and_corrupt_input_is_an_error_and_never_a_panic() {
        let json = export(&[file("settings", json!({"a": "b"}))], &meta())
            .unwrap()
            .bytes;
        let zip = export(&[file("a", json!({})), file("b", json!({}))], &meta())
            .unwrap()
            .bytes;
        for bytes in [&json, &zip] {
            for cut in (0..bytes.len()).step_by(7) {
                let result = read(&bytes[..cut]);
                assert!(result.is_err(), "cut at {cut} read");
            }
        }
        // A flipped byte in the middle of a compressed entry.
        let mut damaged = zip.clone();
        let middle = damaged.len() / 3;
        damaged[middle] ^= 0xFF;
        assert!(read(&damaged).is_err());
        assert!(matches!(
            read(b"PK\x03\x04garbage"),
            Err(BundleError::Corrupt(_))
        ));
        assert!(matches!(
            read(b"{\"format\": \"waypoint-sett"),
            Err(BundleError::Corrupt(_))
        ));
    }

    #[test]
    fn a_newer_format_version_is_refused_with_its_number_in_both_shapes() {
        let newer = json!({"format": BUNDLE_FORMAT, "version": 2, "files": {}});
        assert_eq!(
            read(&serde_json::to_vec(&newer).unwrap()),
            Err(BundleError::NewerFormat {
                found: 2,
                supported: 1
            })
        );
        let manifest = serde_json::to_vec(&newer).unwrap();
        assert_eq!(
            read(&zip_of(&[("manifest.json", &manifest)])),
            Err(BundleError::NewerFormat {
                found: 2,
                supported: 1
            })
        );
        let none = json!({"format": BUNDLE_FORMAT, "files": {}});
        assert!(matches!(
            read(&serde_json::to_vec(&none).unwrap()),
            Err(BundleError::Corrupt(_))
        ));
    }

    #[test]
    fn a_json_over_the_total_cap_is_refused_before_it_is_parsed() {
        let mut bytes = b"{\"format\":\"x\",\"pad\":\"".to_vec();
        bytes.resize(MAX_TOTAL_BYTES + 1, b'a');
        assert!(matches!(read(&bytes), Err(BundleError::TooLarge { .. })));
    }

    #[test]
    fn a_zip_bomb_is_stopped_whatever_the_header_says() {
        // One entry that expands to far more than the per-file cap: highly compressible.
        let huge = vec![b' '; MAX_FILE_BYTES * 3];
        let list = json!([{ "id": "settings", "name": "settings.json" }]);
        let bytes = zip_of(&[("manifest.json", &manifest(list)), ("settings.json", &huge)]);
        assert!(bytes.len() < MAX_FILE_BYTES, "the bomb itself is small");
        assert!(matches!(read(&bytes), Err(BundleError::TooLarge { .. })));
    }

    #[test]
    fn many_files_that_each_fit_but_together_do_not_are_stopped_by_the_total() {
        let mut document = br#"{"pad": ""#.to_vec();
        document.resize(MAX_FILE_BYTES - 100, b'a');
        document.extend_from_slice(b"\"}");
        let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
        let mut listed = Vec::new();
        for n in 0..5 {
            listed.push(json!({ "id": format!("f{n}"), "name": format!("f{n}.json") }));
            entries.push((format!("f{n}.json"), document.clone()));
        }
        let manifest_bytes = manifest(Value::Array(listed));
        let mut all: Vec<(&str, &[u8])> = vec![("manifest.json", &manifest_bytes)];
        all.extend(entries.iter().map(|(n, d)| (n.as_str(), d.as_slice())));
        assert!(matches!(
            read(&zip_of(&all)),
            Err(BundleError::TooLarge { .. })
        ));
    }

    #[test]
    fn too_many_entries_are_refused() {
        let names: Vec<String> = (0..MAX_ENTRIES + 1).map(|n| format!("f{n}.json")).collect();
        let entries: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b"{}"[..])).collect();
        assert_eq!(read(&zip_of(&entries)), Err(BundleError::TooManyEntries));
    }

    #[test]
    fn unsafe_entry_names_are_refused_even_when_unlisted() {
        let list = json!([{ "id": "settings", "name": "settings.json" }]);
        let manifest_bytes = manifest(list);
        for bad in [
            "../evil.json",
            "/etc/passwd.json",
            "a/b.json",
            "a\\b.json",
            "C:evil.json",
            "..",
            ".hidden.json",
            "dir/",
            "nul\0.json",
        ] {
            let bytes = zip_of(&[
                ("manifest.json", &manifest_bytes),
                ("settings.json", b"{}"),
                (bad, b"{}"),
            ]);
            assert!(
                matches!(read(&bytes), Err(BundleError::UnsafeName(_))),
                "{bad:?} was let through"
            );
        }
    }

    #[test]
    fn a_manifest_that_lists_an_unsafe_name_is_refused_and_nothing_is_read_by_it() {
        for bad in ["../settings.json", "/abs.json", "x/y.json", "settings.txt"] {
            let list = json!([{ "id": "settings", "name": bad }]);
            let bytes = zip_of(&[("manifest.json", &manifest(list)), ("settings.json", b"{}")]);
            assert!(
                matches!(read(&bytes), Err(BundleError::UnsafeName(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn duplicate_names_are_refused() {
        let list = json!([{ "id": "settings", "name": "settings.json" }]);
        // The writer refuses a repeated name, so one is made by renaming a second entry in the
        // bytes (same length, so every offset stays right).
        let mut bytes = zip_of(&[
            ("manifest.json", &manifest(list.clone())),
            ("settings.json", b"{}"),
            ("settingsXjson", b"{\"other\": 1}"),
        ]);
        let from = b"settingsXjson";
        let mut at = 0;
        while let Some(found) = bytes[at..].windows(from.len()).position(|w| w == from) {
            bytes[at + found + 8] = b'.';
            at += found + from.len();
        }
        assert!(matches!(read(&bytes), Err(BundleError::DuplicateName(_))));
        // The manifest naming one entry twice.
        let twice = json!([
            { "id": "settings", "name": "settings.json" },
            { "id": "other", "name": "settings.json" },
        ]);
        let bytes = zip_of(&[
            ("manifest.json", &manifest(twice)),
            ("settings.json", b"{}"),
        ]);
        assert!(matches!(read(&bytes), Err(BundleError::DuplicateName(_))));
    }

    #[test]
    fn entries_the_manifest_does_not_list_are_not_read() {
        let list = json!([{ "id": "settings", "name": "settings.json" }]);
        let bytes = zip_of(&[
            ("manifest.json", &manifest(list)),
            ("settings.json", b"{}"),
            ("extra.json", b"not even json"),
        ]);
        let bundle = read(&bytes).unwrap();
        assert_eq!(bundle.files.len(), 1);
        assert_eq!(bundle.unlisted_entries, ["extra.json"]);
    }

    #[test]
    fn a_missing_manifest_or_listed_entry_is_an_error() {
        assert!(matches!(
            read(&zip_of(&[("settings.json", b"{}")])),
            Err(BundleError::MissingEntry(_))
        ));
        let list = json!([{ "id": "settings", "name": "settings.json" }]);
        assert!(matches!(
            read(&zip_of(&[("manifest.json", &manifest(list))])),
            Err(BundleError::MissingEntry(_))
        ));
    }

    #[test]
    fn a_size_that_disagrees_with_the_manifest_is_corrupt() {
        let list = json!([{ "id": "settings", "name": "settings.json", "size": 99 }]);
        let bytes = zip_of(&[("manifest.json", &manifest(list)), ("settings.json", b"{}")]);
        assert!(matches!(read(&bytes), Err(BundleError::Corrupt(_))));
    }

    #[test]
    fn deeply_nested_json_is_an_error_and_not_a_stack_overflow() {
        let mut text =
            String::from(r#"{"format":"waypoint-settings","version":1,"files":{"settings":"#);
        text.push_str(&"[".repeat(5000));
        text.push_str(&"]".repeat(5000));
        text.push_str("}}");
        assert!(matches!(
            read(text.as_bytes()),
            Err(BundleError::Corrupt(_))
        ));
    }
}

// Listing a zip archive from its central directory alone, without touching any entry's data or its
// local header, so a 10 000-entry archive on a server costs a few ranged reads.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, BufReader, Read, Seek, SeekFrom};

use waypoint_vfs::{CancelToken, EntryKind};

use crate::index::{ArchiveIndex, Locator, NewEntry};

const EOCD_SIG: u32 = 0x0605_4b50;
const EOCD64_SIG: u32 = 0x0606_4b50;
const LOCATOR64_SIG: u32 = 0x0706_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_LEN: u64 = 22;
const LOCATOR64_LEN: u64 = 20;
const MAX_COMMENT: u64 = 65_535;

#[derive(Debug)]
pub(crate) enum ScanError {
    /// Not a zip archive, or one whose directory is damaged.
    Corrupt(&'static str),
    Io(io::Error),
    TooMany(usize),
    Cancelled,
}

impl From<io::Error> for ScanError {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            Self::Corrupt("the central directory ends early")
        } else {
            Self::Io(error)
        }
    }
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
}

/// What the end of central directory record says.
struct Directory {
    entries: u64,
    /// Where the central directory starts in the file, including anything in front of the archive.
    start: u64,
    size: u64,
    /// How far into the file the archive starts: the offsets in the directory are relative to it.
    prefix: u64,
    comment: Vec<u8>,
}

fn find_directory(source: &mut (impl Read + Seek), length: u64) -> Result<Directory, ScanError> {
    if length < EOCD_LEN {
        return Err(ScanError::Corrupt("too short to be a zip archive"));
    }
    let tail_len = length.min(EOCD_LEN + MAX_COMMENT + LOCATOR64_LEN);
    let tail_start = length - tail_len;
    source.seek(SeekFrom::Start(tail_start))?;
    let mut tail = vec![0u8; tail_len as usize];
    source.read_exact(&mut tail)?;
    // The last signature whose comment length reaches the end of the file.
    let mut at = tail.len() - EOCD_LEN as usize;
    let eocd = loop {
        if u32_at(&tail, at) == EOCD_SIG {
            let comment_len = u16_at(&tail, at + 20) as usize;
            if at + EOCD_LEN as usize + comment_len <= tail.len() {
                break at;
            }
        }
        if at == 0 {
            return Err(ScanError::Corrupt("no end of central directory record"));
        }
        at -= 1;
    };
    let comment_len = u16_at(&tail, eocd + 20) as usize;
    let comment = tail[eocd + 22..eocd + 22 + comment_len].to_vec();
    let eocd_at = tail_start + eocd as u64;
    let mut entries = u64::from(u16_at(&tail, eocd + 10));
    let mut size = u64::from(u32_at(&tail, eocd + 12));
    let mut offset = u64::from(u32_at(&tail, eocd + 16));
    // Where the central directory ends: right before the end record, or before the zip64 records.
    let mut directory_end = eocd_at;
    let needs64 = entries == 0xffff || size == 0xffff_ffff || offset == 0xffff_ffff;
    if eocd >= LOCATOR64_LEN as usize
        && u32_at(&tail, eocd - LOCATOR64_LEN as usize) == LOCATOR64_SIG
    {
        let locator = eocd - LOCATOR64_LEN as usize;
        let claimed = u64_at(&tail, locator + 8);
        // The record is where the locator says, or (with bytes in front of the archive) 56 bytes
        // before the locator when it has no extensible data.
        let locator_at = tail_start + locator as u64;
        for candidate in [Some(claimed), locator_at.checked_sub(56)]
            .into_iter()
            .flatten()
        {
            if candidate + 56 > length {
                continue;
            }
            source.seek(SeekFrom::Start(candidate))?;
            let mut record = [0u8; 56];
            if source.read_exact(&mut record).is_err() || u32_at(&record, 0) != EOCD64_SIG {
                continue;
            }
            entries = u64_at(&record, 32);
            size = u64_at(&record, 40);
            offset = u64_at(&record, 48);
            directory_end = candidate;
            break;
        }
    } else if needs64 {
        return Err(ScanError::Corrupt(
            "a zip64 archive without its zip64 record",
        ));
    }
    let start = directory_end.checked_sub(size).ok_or(ScanError::Corrupt(
        "the central directory is bigger than the file",
    ))?;
    if offset > start {
        return Err(ScanError::Corrupt(
            "the central directory starts after where it ends",
        ));
    }
    Ok(Directory {
        entries,
        start,
        size,
        prefix: start - offset,
        comment,
    })
}

/// The characters of code page 437 above ASCII, which zip uses for names that are not marked UTF-8.
const CP437: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', 'É', 'æ', 'Æ',
    'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', 'á', 'í', 'ó', 'ú', 'ñ', 'Ñ',
    'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕',
    '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦',
    '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐',
    '▀', 'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', '≡', '±',
    '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

fn decode_cp437(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| {
            if byte < 0x80 {
                byte as char
            } else {
                CP437[(byte - 0x80) as usize]
            }
        })
        .collect()
}

/// The name as text: UTF-8 when it is (flagged or not, as most tools write it), else code page 437.
fn name_text(raw: &[u8]) -> Vec<u8> {
    if std::str::from_utf8(raw).is_ok() {
        raw.to_vec()
    } else {
        decode_cp437(raw).into_bytes()
    }
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// An MS-DOS date and time, taken as UTC (zip stores local time and no zone).
fn dos_time_ms(date: u16, time: u16) -> Option<i64> {
    if date == 0 {
        return None;
    }
    let year = 1980 + i64::from(date >> 9);
    let month = i64::from((date >> 5) & 0xf);
    let day = i64::from(date & 0x1f);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let hour = i64::from(time >> 11).min(23);
    let minute = i64::from((time >> 5) & 0x3f).min(59);
    let second = i64::from((time & 0x1f) * 2).min(59);
    let days = days_from_civil(year, month, day);
    Some(((days * 24 + hour) * 3600 + minute * 60 + second) * 1000)
}

const S_IFMT: u32 = 0o170_000;
const S_IFDIR: u32 = 0o040_000;
const S_IFLNK: u32 = 0o120_000;
const S_IFREG: u32 = 0o100_000;

struct Extra {
    zip64: Option<Vec<u8>>,
    unicode_name: Option<Vec<u8>>,
    unix_mtime: Option<i64>,
    aes: bool,
}

fn parse_extra(mut extra: &[u8], header_name: &[u8]) -> Extra {
    let mut found = Extra {
        zip64: None,
        unicode_name: None,
        unix_mtime: None,
        aes: false,
    };
    while extra.len() >= 4 {
        let tag = u16_at(extra, 0);
        let len = u16_at(extra, 2) as usize;
        let Some(body) = extra.get(4..4 + len) else {
            break;
        };
        match tag {
            0x0001 => found.zip64 = Some(body.to_vec()),
            // Info-ZIP Unicode Path: version, CRC-32 of the header's name, the name.
            0x7075 if body.len() > 5 && body[0] == 1 => {
                let mut crc = flate2::Crc::new();
                crc.update(header_name);
                if crc.sum() == u32_at(body, 1) {
                    found.unicode_name = Some(body[5..].to_vec());
                }
            }
            // Extended timestamp: flags, then the modification time when bit 0 is set.
            0x5455 if body.len() >= 5 && body[0] & 1 == 1 => {
                found.unix_mtime = Some(i64::from(i32::from_le_bytes(
                    body[1..5].try_into().expect("four bytes"),
                )));
            }
            0x9901 => found.aes = true,
            _ => {}
        }
        extra = &extra[4 + len..];
    }
    found
}

/// Lists the archive into `index`. `report` is called with the entries read so far.
pub(crate) fn scan(
    source: &mut (impl Read + Seek),
    length: u64,
    index: &mut ArchiveIndex,
    max_entries: usize,
    cancel: &CancelToken,
    report: &mut dyn FnMut(u32),
) -> Result<(), ScanError> {
    let directory = find_directory(source, length)?;
    if directory.entries > max_entries as u64 {
        return Err(ScanError::TooMany(max_entries));
    }
    index.signature.0 = length;
    if !directory.comment.is_empty() {
        index.comment = Some(String::from_utf8_lossy(&directory.comment).into_owned());
    }
    // The archive may start after the beginning of the file (a program in front of it).
    source.seek(SeekFrom::Start(directory.start))?;
    let mut reader = BufReader::with_capacity(1 << 20, source.by_ref().take(directory.size));
    // The zip crate numbers distinct names in the order they first appear.
    let mut ordinals: std::collections::HashMap<Vec<u8>, usize> = std::collections::HashMap::new();
    let mut count = 0usize;
    loop {
        let mut fixed = [0u8; 46];
        match reader.read(&mut fixed[..1])? {
            0 => break,
            _ => reader.read_exact(&mut fixed[1..])?,
        }
        if u32_at(&fixed, 0) != CENTRAL_SIG {
            return Err(ScanError::Corrupt("a bad central directory header"));
        }
        if count >= max_entries {
            return Err(ScanError::TooMany(max_entries));
        }
        if count.is_multiple_of(256) {
            if cancel.is_cancelled() {
                return Err(ScanError::Cancelled);
            }
            report(count as u32);
        }
        count += 1;
        let made_by = u16_at(&fixed, 4);
        let flags = u16_at(&fixed, 8);
        let method = u16_at(&fixed, 10);
        let time = u16_at(&fixed, 12);
        let date = u16_at(&fixed, 14);
        let mut compressed = u64::from(u32_at(&fixed, 20));
        let mut size = u64::from(u32_at(&fixed, 24));
        let name_len = u16_at(&fixed, 28) as usize;
        let extra_len = u16_at(&fixed, 30) as usize;
        let comment_len = u16_at(&fixed, 32) as usize;
        let external = u32_at(&fixed, 38);
        let mut header_start = u64::from(u32_at(&fixed, 42));
        let mut variable = vec![0u8; name_len + extra_len + comment_len];
        reader.read_exact(&mut variable)?;
        let header_name = &variable[..name_len];
        let extra = parse_extra(&variable[name_len..name_len + extra_len], header_name);
        if let Some(zip64) = &extra.zip64 {
            let mut at = 0;
            let mut take = |wanted: bool| -> Option<u64> {
                if !wanted {
                    return None;
                }
                let value = zip64
                    .get(at..at + 8)
                    .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("eight")));
                at += 8;
                value
            };
            if let Some(value) = take(size == 0xffff_ffff) {
                size = value;
            }
            if let Some(value) = take(compressed == 0xffff_ffff) {
                compressed = value;
            }
            if let Some(value) = take(header_start == 0xffff_ffff) {
                header_start = value;
            }
        }
        let name_bytes = extra
            .unicode_name
            .clone()
            .unwrap_or_else(|| name_text(header_name));
        let host = (made_by >> 8) as u8;
        let mode = (host == 3 && external >> 16 != 0).then_some(external >> 16);
        let dos_dir = external & 0x10 != 0;
        let kind = match mode.map(|mode| mode & S_IFMT) {
            Some(S_IFDIR) => EntryKind::Directory,
            Some(S_IFLNK) => EntryKind::Symlink,
            Some(S_IFREG) | None => {
                if name_bytes.last() == Some(&b'/') || (dos_dir && mode.is_none()) {
                    EntryKind::Directory
                } else {
                    EntryKind::File
                }
            }
            Some(_) => EntryKind::Other,
        };
        let encrypted = flags & 1 != 0 || method == 99 || extra.aes;
        let modified_ms = extra
            .unix_mtime
            .map(|seconds| seconds * 1000)
            .or_else(|| dos_time_ms(date, time));
        let distinct = ordinals.len();
        let ordinal = *ordinals.entry(header_name.to_vec()).or_insert(distinct);
        index.insert(
            &name_bytes,
            true,
            NewEntry {
                kind: Some(kind),
                size: Some(size),
                compressed: Some(compressed),
                modified_ms,
                mode: mode.map(|mode| mode & 0o7777),
                link: None,
                hardlink: None,
                encrypted,
                sparse: false,
                locator: Some(Locator::Zip {
                    ordinal,
                    header_start: header_start + directory.prefix,
                }),
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_dates_become_milliseconds() {
        // 2024-01-01 12:00:00.
        assert_eq!(dos_time_ms(0x5821, 0x6000), Some(1_704_110_400_000));
        // The epoch of the format (1980-01-01).
        assert_eq!(dos_time_ms(0x0021, 0), Some(315_532_800_000));
        // No date, or a nonsense one, is no time.
        assert_eq!(dos_time_ms(0, 0x6000), None);
        assert_eq!(dos_time_ms(0x5800, 0), None);
    }

    #[test]
    fn legacy_names_read_as_code_page_437() {
        assert_eq!(name_text(b"caf\x82"), "caf\u{e9}".as_bytes());
        assert_eq!(name_text("caf\u{e9}".as_bytes()), "caf\u{e9}".as_bytes());
        assert_eq!(decode_cp437(&[0xb0, 0xe1, 0xff]), "\u{2591}\u{df}\u{a0}");
    }

    #[test]
    fn an_end_record_is_found_after_a_long_comment() {
        let mut bytes = vec![b'x'; 100];
        bytes.extend_from_slice(&EOCD_SIG.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(b"PK\x05");
        let length = bytes.len() as u64;
        let found = find_directory(&mut io::Cursor::new(bytes), length).unwrap();
        assert_eq!(found.entries, 0);
        assert_eq!(found.comment, b"PK\x05");
    }
}

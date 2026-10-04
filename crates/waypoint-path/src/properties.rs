// Randomised round-trip properties for the path rules, from a fixed seed so failures reproduce.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{posix, windows};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const CASES: usize = 2000;

/// Bytes that stress the URI encoding and the normaliser, including ones that are not UTF-8.
const BYTE_PARTS: [&[u8]; 14] = [
    b"a",
    b"B",
    b"docs",
    b"with space",
    b"100%",
    b"a#b",
    b"q?x",
    b".",
    b"..",
    b"..hidden",
    b"",
    b"caf\xc3\xa9",
    b"\xff\xfe",
    b"~",
];

fn posix_path(rng: &mut Rng) -> Vec<u8> {
    let mut out = Vec::new();
    for _ in 0..=rng.below(6) {
        out.push(b'/');
        if rng.below(8) == 0 {
            out.push(b'/');
        }
        out.extend_from_slice(BYTE_PARTS[rng.below(BYTE_PARTS.len())]);
    }
    out
}

#[test]
fn posix_normalise_is_idempotent_and_uri_round_trips() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..CASES {
        let raw = posix_path(&mut rng);
        let once = posix::normalise(&raw).unwrap();
        assert_eq!(posix::normalise(&once).unwrap(), once, "{raw:?}");
        let uri = posix::to_uri(&once);
        assert!(uri.is_ascii(), "{uri}");
        assert_eq!(
            posix::from_uri(uri.strip_prefix("file://").unwrap()).unwrap(),
            once,
            "{raw:?} via {uri}"
        );
    }
}

#[test]
fn posix_join_then_parent_returns_the_base() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for _ in 0..CASES {
        let base = posix::normalise(&posix_path(&mut rng)).unwrap();
        let name: Vec<u8> = BYTE_PARTS[2 + rng.below(3)].to_vec(); // never `.`, `..` or empty
        let child = posix::join(&base, &name).unwrap();
        assert_eq!(posix::file_name(&child), Some(&name[..]));
        assert_eq!(posix::parent(&child).unwrap(), base);
    }
}

const WIN_PARTS: [&str; 12] = [
    "a",
    "Docs",
    "with space",
    "100%",
    "é",
    "日本",
    "a#b",
    ".",
    "..",
    "x.y",
    "UPPER",
    "{guid}",
];

fn windows_path(rng: &mut Rng) -> String {
    let sep = |rng: &mut Rng| if rng.below(2) == 0 { "\\" } else { "/" };
    let mut out = match rng.below(5) {
        0 => "C:".to_owned(),
        1 => "d:".to_owned(),
        2 => r"\\server\share".to_owned(),
        3 => r"\\?\E:".to_owned(),
        _ => r"\\?\UNC\srv\sh".to_owned(),
    };
    // Verbatim paths only take `\`.
    let verbatim = out.starts_with(r"\\?\");
    for _ in 0..rng.below(5) {
        out.push_str(if verbatim { "\\" } else { sep(rng) });
        out.push_str(WIN_PARTS[rng.below(WIN_PARTS.len())]);
    }
    if !out.contains('\\') || out == "C:" || out == "d:" {
        out.push('\\');
    }
    out
}

#[test]
fn windows_rendering_reparses_to_the_same_path() {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for _ in 0..CASES {
        let text = windows_path(&mut rng);
        let parsed = windows::parse(&text).unwrap();
        let rendered = parsed.to_native_string();
        assert_eq!(
            windows::parse(&rendered).unwrap(),
            parsed,
            "{text} -> {rendered}"
        );
    }
}

#[test]
fn windows_uri_round_trips_every_absolute_path() {
    let mut rng = Rng(0x94D0_49BB_1331_11EB);
    for _ in 0..CASES {
        let text = windows_path(&mut rng);
        let parsed = windows::parse(&text).unwrap();
        if !parsed.is_absolute() {
            continue;
        }
        let uri = windows::to_uri(&parsed).unwrap();
        assert!(uri.is_ascii(), "{uri}");
        let back = windows::from_uri(uri.strip_prefix("file://").unwrap()).unwrap();
        assert_eq!(back, parsed, "{text} via {uri}");
    }
}

#[test]
fn windows_case_folding_ignores_case_and_verbatim_form() {
    let mut rng = Rng(0xBF58_476D_1CE4_E5B9);
    for _ in 0..CASES {
        let text = windows_path(&mut rng);
        let parsed = windows::parse(&text).unwrap();
        let shouted = windows::parse(&text.to_uppercase()).unwrap();
        // Upper-casing can change `UNC` and drive letters, never which file is meant.
        assert_eq!(parsed.fold_key(), shouted.fold_key(), "{text}");
    }
}

#[test]
fn windows_join_then_parent_returns_the_base() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    for _ in 0..CASES {
        let base = windows::parse(&windows_path(&mut rng)).unwrap();
        let name = windows::parse(WIN_PARTS[rng.below(5)]).unwrap();
        let child = base.join(&name);
        assert_eq!(child.file_name(), name.file_name());
        assert_eq!(child.parent().unwrap(), base);
    }
}

const HOSTS: [&str; 8] = [
    "nas",
    "NAS.Example.org",
    "10.0.0.7",
    "[::1]",
    "[2001:DB8::0:1]",
    "[fe80::1%25eth0]",
    "b%C3%BCcher.example",
    "h-1.lan",
];

const USERS: [&str; 5] = ["", "me@", "WORK;me@", "a%40b@", "caf%C3%A9@"];

const SCHEMES: [&str; 5] = ["sftp", "SMB", "dav", "davs", "webdavs"];

fn remote_uri(rng: &mut Rng) -> String {
    let mut out = format!(
        "{}://{}{}",
        SCHEMES[rng.below(SCHEMES.len())],
        USERS[rng.below(USERS.len())],
        HOSTS[rng.below(HOSTS.len())]
    );
    if rng.below(3) == 0 {
        out.push_str([":22", ":445", ":8443", ":1"][rng.below(4)]);
    }
    for _ in 0..rng.below(5) {
        out.push('/');
        let part = BYTE_PARTS[rng.below(BYTE_PARTS.len())];
        let mut encoded = String::new();
        crate::encoding::encode_into(&mut encoded, part, &[]);
        out.push_str(&encoded);
    }
    out
}

#[test]
fn remote_canonical_forms_are_fixed_points_and_round_trip() {
    let mut rng = Rng(0x5851_F42D_4C95_7F2D);
    for _ in 0..CASES {
        let text = remote_uri(&mut rng);
        let path = crate::RemotePath::from_uri(&text).unwrap_or_else(|e| panic!("{text}: {e}"));
        let uri = path.to_uri();
        assert!(uri.is_ascii(), "{uri}");
        let again = crate::RemotePath::from_uri(&uri).unwrap();
        assert_eq!(again, path, "{text} via {uri}");
        assert_eq!(again.to_uri(), uri, "{text}");
        // Every parent is a prefix of the path, and joining the name back returns it.
        if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
            assert_eq!(parent.join(&name).unwrap(), path, "{text}");
            assert!(
                uri.starts_with(parent.to_uri().trim_end_matches('/')),
                "{uri}"
            );
        }
    }
}

#[test]
fn archives_of_remotes_round_trip() {
    let mut rng = Rng(0x2127_599B_F432_5C37);
    for _ in 0..CASES / 4 {
        let container = remote_uri(&mut rng) + "/x!.zip";
        let uri = format!(
            "archive:{}!/a%21/b",
            crate::RemotePath::from_uri(&container).unwrap().to_uri()
        );
        let path = crate::VfsPath::from_uri(&uri).unwrap_or_else(|e| panic!("{uri}: {e}"));
        assert_eq!(path.to_uri(), uri);
        let nested = format!("archive:{uri}!/c");
        assert_eq!(crate::VfsPath::from_uri(&nested).unwrap().to_uri(), nested);
    }
}

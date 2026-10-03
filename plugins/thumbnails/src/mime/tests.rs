// Tests glob matching and MIME lookup by name
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn globs_match_like_a_shell() {
    for (glob, name, expected) in [
        ("*.png", "a.png", true),
        ("*.png", "a.png.bak", false),
        ("*.png", ".png", true),
        ("*", "anything", true),
        ("*", "", true),
        ("a?c", "abc", true),
        ("a?c", "ac", false),
        ("*.tar.gz", "x.tar.gz", true),
        ("*.tar.gz", "x.tar", false),
        ("*.[1-9]", "ls.1", true),
        ("*.[1-9]", "ls.a", false),
        ("*.[!1-9]", "ls.a", true),
        ("README*", "README.md", true),
        ("README*", "readme.md", false),
        ("*a*b*c", "xxaxxbxxc", true),
        ("*a*b*c", "xxaxxcxxb", false),
        ("a\\*b", "a*b", true),
        ("a\\*b", "axb", false),
        ("[", "[", false),
        ("*.jp*g", "p.jpeg", true),
    ] {
        assert_eq!(glob_matches(glob, name), expected, "{glob:?} vs {name:?}");
    }
}

const GLOBS: &str = "\
# This file was automatically generated
50:image/png:*.png
50:image/jpeg:*.jpg
50:image/jpeg:*.jpeg
50:application/pdf:*.pdf
50:application/x-compressed-tar:*.tar.gz
50:application/gzip:*.gz
50:text/x-readme:README*
10:text/x-lowweight:*.png
50:text/x-case:*.PNGCS:cs
0:__NOGLOBS__:
bad line
";

#[test]
fn the_database_resolves_by_weight_then_by_glob_length_and_ignores_case() {
    let db = MimeDb::parse(GLOBS);
    assert_eq!(db.mime_of("photo.png"), Some("image/png"));
    assert_eq!(db.mime_of("PHOTO.JPG"), Some("image/jpeg"));
    assert_eq!(db.mime_of("a.tar.gz"), Some("application/x-compressed-tar"));
    assert_eq!(db.mime_of("a.gz"), Some("application/gzip"));
    assert_eq!(db.mime_of("README.md"), Some("text/x-readme"));
    assert_eq!(db.mime_of("notes.txt"), None);
    // The flag `cs` makes a glob case sensitive.
    assert_eq!(db.mime_of("a.PNGCS"), Some("text/x-case"));
    assert_eq!(db.mime_of("a.pngcs"), None);
}

#[test]
fn a_missing_database_falls_back_to_the_built_in_table() {
    let tmp = tempfile::tempdir().unwrap();
    let db = MimeDb::load(&[tmp.path().to_path_buf()]);
    assert_eq!(db.mime_of("a.JPG"), Some("image/jpeg"));
    assert_eq!(db.mime_of("a.pdf"), Some("application/pdf"));
    assert_eq!(db.mime_of("a.xyz"), None);
}

#[test]
fn on_a_tie_the_first_data_directory_wins() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a");
    let b = tmp.path().join("b");
    std::fs::create_dir_all(a.join("mime")).unwrap();
    std::fs::create_dir_all(b.join("mime")).unwrap();
    std::fs::write(a.join("mime/globs2"), "50:text/x-a:*.zzz\n").unwrap();
    std::fs::write(b.join("mime/globs2"), "50:text/x-b:*.zzz\n").unwrap();
    let db = MimeDb::load(&[tmp.path().join("none"), a, b]);
    assert_eq!(db.mime_of("f.zzz"), Some("text/x-a"));
}

#[test]
fn a_users_database_adds_to_the_system_one_instead_of_hiding_it() {
    let tmp = tempfile::tempdir().unwrap();
    let user = tmp.path().join("user");
    let system = tmp.path().join("system");
    std::fs::create_dir_all(user.join("mime")).unwrap();
    std::fs::create_dir_all(system.join("mime")).unwrap();
    std::fs::write(user.join("mime/globs2"), "50:text/x-mine:*.mine\n").unwrap();
    std::fs::write(system.join("mime/globs2"), "50:video/mp4:*.mp4\n").unwrap();
    let db = MimeDb::load(&[user, system]);
    assert_eq!(db.mime_of("a.mine"), Some("text/x-mine"));
    assert_eq!(db.mime_of("clip.mp4"), Some("video/mp4"));
}

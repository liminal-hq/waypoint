// Tests the pure Windows mappings on every platform
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

#[test]
fn cloud_placeholders_only_ever_ask_the_shell_cache() {
    assert_eq!(shell_mode(0), ShellMode::Make);
    assert_eq!(shell_mode(0x20), ShellMode::Make); // ARCHIVE
    assert_eq!(
        shell_mode(FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS),
        ShellMode::CacheOnly
    );
    assert_eq!(
        shell_mode(FILE_ATTRIBUTE_RECALL_ON_OPEN | 0x20),
        ShellMode::CacheOnly
    );
    assert_eq!(shell_mode(FILE_ATTRIBUTE_OFFLINE), ShellMode::CacheOnly);
    // The values are the documented ones.
    assert_eq!(FILE_ATTRIBUTE_RECALL_ON_OPEN, 262_144);
    assert_eq!(FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, 4_194_304);
}

#[test]
fn only_local_drives_are_local() {
    for local in [
        "C:\\Users\\a\\b.png",
        "d:/photos/a.jpg",
        r"\\?\C:\long\path.png",
        "Z:\\x",
    ] {
        assert!(is_local(local), "{local}");
    }
    for remote in [
        r"\\server\share\a.png",
        r"\\?\UNC\server\share\a.png",
        "sftp://host/a.png",
        "C://odd",
        "a.png",
        r"relative\a.png",
        "/unix/path.png",
        "C:",
        "",
    ] {
        assert!(!is_local(remote), "{remote}");
    }
}

#[test]
fn shell_errors_map_to_outcomes() {
    assert_eq!(
        classify_failure(0x8004_B200_u32 as i32),
        ShellFailure::NoThumbnail
    );
    assert_eq!(
        classify_failure(0x8004_B205_u32 as i32),
        ShellFailure::NoThumbnail
    );
    assert_eq!(
        classify_failure(0x8000_4005_u32 as i32),
        ShellFailure::NoThumbnail
    );
    assert_eq!(
        classify_failure(0x8000_000A_u32 as i32),
        ShellFailure::NoThumbnail
    );
    assert_eq!(
        classify_failure(0x8007_0002_u32 as i32),
        ShellFailure::NotFound
    );
    assert_eq!(
        classify_failure(0x8007_0005_u32 as i32),
        ShellFailure::Other
    );
    assert_eq!(
        skip_for(ShellMode::CacheOnly, ShellFailure::NoThumbnail),
        Some(SkipWhy::Cloud)
    );
    assert_eq!(
        skip_for(ShellMode::CacheOnly, ShellFailure::Other),
        Some(SkipWhy::Cloud)
    );
    assert_eq!(
        skip_for(ShellMode::Make, ShellFailure::NoThumbnail),
        Some(SkipWhy::NoGenerator)
    );
    assert_eq!(skip_for(ShellMode::Make, ShellFailure::NotFound), None);
    assert_eq!(skip_for(ShellMode::Make, ShellFailure::Other), None);
}

#[test]
fn a_bitmap_with_no_alpha_becomes_opaque_rgb() {
    // Two pixels, blue then red, in BGRA with every alpha zero.
    let out = bitmap_to_rendered(2, 1, &[255, 0, 0, 0, 0, 0, 255, 0]).unwrap();
    assert_eq!(out.color, png::ColorType::Rgb);
    assert_eq!(out.pixels, [0, 0, 255, 255, 0, 0]);
}

#[test]
fn a_bitmap_with_alpha_keeps_it_as_rgba() {
    let out = bitmap_to_rendered(2, 1, &[255, 0, 0, 255, 0, 0, 255, 0]).unwrap();
    assert_eq!(out.color, png::ColorType::Rgba);
    assert_eq!(out.pixels, [0, 0, 255, 255, 255, 0, 0, 0]);
}

#[test]
fn a_bitmap_of_the_wrong_size_is_refused() {
    assert!(bitmap_to_rendered(0, 1, &[]).is_none());
    assert!(bitmap_to_rendered(2, 2, &[0; 8]).is_none());
    assert!(bitmap_to_rendered(u32::MAX, u32::MAX, &[0; 8]).is_none());
}

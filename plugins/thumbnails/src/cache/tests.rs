// Tests the cache naming, the PNG metadata round trip, staleness and the failure cache in a temporary directory
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

fn store(root: &Path) -> Store {
    Store::new(root.to_path_buf(), "test-app", "1.2.3", Arc::new(file_uri))
}

fn meta(uri: &str, mtime: i64) -> Meta {
    Meta {
        uri: uri.to_string(),
        mtime_secs: mtime,
        file_size: Some(42),
    }
}

#[test]
fn the_name_is_the_md5_of_the_uri_as_the_standard_gives_it() {
    // The example in the Thumbnail Managing Standard.
    assert_eq!(
        md5_hex("file:///home/jens/photos/me.png"),
        "c6ee772d9e49320e97ec29a7eb5b1697"
    );
    let key = CacheKey::new(ThumbSize::Large, "file:///home/jens/photos/me.png");
    assert_eq!(key.relative(), "large/c6ee772d9e49320e97ec29a7eb5b1697.png");
}

#[cfg(unix)]
#[test]
fn uris_are_percent_encoded_like_glib() {
    assert_eq!(
        file_uri(Path::new("/home/a/me.png")),
        "file:///home/a/me.png"
    );
    assert_eq!(
        file_uri(Path::new("/home/a b/ré#1?.png")),
        "file:///home/a%20b/r%C3%A9%231%3F.png"
    );
    // GLib leaves these reserved characters alone in a path.
    assert_eq!(
        file_uri(Path::new("/a/b'(c)+d=e,f;g:h@i!j$k&l*m.png")),
        "file:///a/b'(c)+d=e,f;g:h@i!j$k&l*m.png"
    );
    assert_eq!(
        md5_hex(&file_uri(Path::new("/home/a b/ré#1?.png"))),
        md5_hex("file:///home/a%20b/r%C3%A9%231%3F.png")
    );
}

#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf8_still_has_a_distinct_uri() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let a = PathBuf::from(OsStr::from_bytes(b"/tmp/\xff\xfe.png"));
    let b = PathBuf::from(OsStr::from_bytes(b"/tmp/\xff\xfd.png"));
    assert_eq!(file_uri(&a), "file:///tmp/%FF%FE.png");
    assert_ne!(file_uri(&a), file_uri(&b));
    assert_ne!(md5_hex(&file_uri(&a)), md5_hex(&file_uri(&b)));
}

#[test]
fn keys_parse_strictly() {
    let md5 = "c6ee772d9e49320e97ec29a7eb5b1697";
    for size in ThumbSize::ALL {
        let text = format!("/{}/{md5}.png", size.dir_name());
        assert_eq!(
            CacheKey::parse(&text),
            Some(CacheKey {
                size,
                md5: md5.to_string()
            })
        );
    }
    for bad in [
        "",
        "/",
        "/normal/",
        "/normal/../normal/c6ee772d9e49320e97ec29a7eb5b1697.png",
        "/../c6ee772d9e49320e97ec29a7eb5b1697.png",
        "/normal/..%2f..%2fetc%2fpasswd",
        "/normal/c6ee772d9e49320e97ec29a7eb5b1697.png/../x",
        "/normal/C6EE772D9E49320E97EC29A7EB5B1697.png",
        "/normal/c6ee772d9e49320e97ec29a7eb5b169.png",
        "/normal/c6ee772d9e49320e97ec29a7eb5b16977.png",
        "/normal/c6ee772d9e49320e97ec29a7eb5b1697.jpg",
        "/fail/c6ee772d9e49320e97ec29a7eb5b1697.png",
        "/huge/c6ee772d9e49320e97ec29a7eb5b1697.png",
        "//normal/c6ee772d9e49320e97ec29a7eb5b1697.png",
        "/normal\\c6ee772d9e49320e97ec29a7eb5b1697.png",
        "/etc/passwd",
    ] {
        assert_eq!(CacheKey::parse(bad), None, "{bad:?} must be refused");
    }
}

#[test]
fn the_text_chunks_round_trip_and_precede_the_pixels() {
    let m = meta("file:///x/a%20b.png", 1_700_000_000);
    let png = encode_png(
        2,
        1,
        png::ColorType::Rgba,
        &[1, 2, 3, 255, 4, 5, 6, 255],
        &m,
    )
    .unwrap();
    assert_eq!(meta_of_bytes(&png), Some(m.clone()));
    // A header-only truncation after the chunks still reads, which is what makes a lookup cheap.
    let idat = png.windows(4).position(|w| w == b"IDAT").unwrap();
    assert_eq!(meta_of_bytes(&png[..idat + 4]), Some(m));
    // The pixels survive.
    let mut reader = png::Decoder::new(io::Cursor::new(&png))
        .read_info()
        .unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut buf).unwrap();
    assert_eq!(buf, [1, 2, 3, 255, 4, 5, 6, 255]);
}

#[test]
fn a_png_without_the_chunks_has_no_meta() {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 1, 1);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().unwrap();
    w.write_image_data(&[0]).unwrap();
    w.finish().unwrap();
    assert_eq!(meta_of_bytes(&out), None);
    assert_eq!(meta_of_bytes(b"not a png"), None);
}

#[test]
fn an_entry_is_fresh_until_the_mtime_or_uri_differs() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store(tmp.path());
    let uri = "file:///data/a.png";
    let (key, _) = (CacheKey::new(ThumbSize::Normal, uri), ());
    assert_eq!(store.lookup(&key, uri, 100), Lookup::Miss);
    let png = encode_png(1, 1, png::ColorType::Rgba, &[0; 4], &meta(uri, 100)).unwrap();
    store.store(&key, &png).unwrap();
    assert_eq!(store.lookup(&key, uri, 100), Lookup::Fresh);
    assert_eq!(store.lookup(&key, uri, 101), Lookup::Stale);
    assert_eq!(
        store.lookup(&key, "file:///data/other.png", 100),
        Lookup::Stale
    );
    assert_eq!(store.read(&key), Some(png));
}

#[test]
fn mtimes_are_whole_seconds() {
    assert_eq!(mtime_secs(1_999), 1);
    assert_eq!(mtime_secs(2_000), 2);
    assert_eq!(mtime_secs(-1), -1);
}

#[test]
fn a_failure_is_remembered_for_that_version_of_the_file_only() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store(tmp.path());
    let uri = "file:///data/broken.jpg";
    let key = CacheKey::new(ThumbSize::Large, uri);
    store.store_failure(&key, &meta(uri, 7)).unwrap();
    assert!(tmp
        .path()
        .join(format!("fail/test-app-1.2.3/{}.png", key.md5))
        .is_file());
    assert_eq!(store.lookup(&key, uri, 7), Lookup::Failed);
    assert_eq!(store.lookup(&key, uri, 8), Lookup::Miss);
    store.clear_failure(&key);
    assert_eq!(store.lookup(&key, uri, 7), Lookup::Miss);
}

#[cfg(unix)]
#[test]
fn entries_are_private_and_written_atomically() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let store = store(&tmp.path().join("thumbnails"));
    let uri = "file:///data/a.png";
    let key = CacheKey::new(ThumbSize::XLarge, uri);
    store.store(&key, b"first").unwrap();
    store.store(&key, b"second").unwrap();
    let dir = tmp.path().join("thumbnails/x-large");
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir.join(format!("{}.png", key.md5))), 0o600);
    assert_eq!(mode(&dir), 0o700);
    // No temporary file is left behind.
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    assert_eq!(store.read(&key), Some(b"second".to_vec()));
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_in_the_cache_is_not_served() {
    let tmp = tempfile::tempdir().unwrap();
    let store = store(&tmp.path().join("c"));
    let secret = tmp.path().join("secret.txt");
    fs::write(&secret, "secret").unwrap();
    let key = CacheKey::new(ThumbSize::Normal, "file:///x");
    fs::create_dir_all(tmp.path().join("c/normal")).unwrap();
    std::os::unix::fs::symlink(
        &secret,
        tmp.path().join(format!("c/normal/{}.png", key.md5)),
    )
    .unwrap();
    assert_eq!(store.read(&key), None);
}

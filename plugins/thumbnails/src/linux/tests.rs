// Tests the Linux processor in a temporary directory with fake thumbnailers; the real home and cache are never touched
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::*;
use crate::cache::{self, Lookup};
use crate::models::ThumbSize;

struct Fixture {
    tmp: tempfile::TempDir,
    platform: Platform,
    processor: Arc<dyn Processor>,
    limits: Arc<Limits>,
}

impl Fixture {
    fn root(&self) -> &Path {
        self.tmp.path()
    }

    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.root().join("files").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn request(&self, path: &str, mtime_ms: i64) -> ThumbRequest {
        ThumbRequest {
            key: path.to_string(),
            path: path.to_string(),
            size: ThumbSize::Normal,
            mtime_ms,
        }
    }

    fn process(&self, path: &str, mtime_ms: i64) -> Outcome {
        self.processor
            .process(&self.request(path, mtime_ms), &AtomicBool::new(false))
    }
}

fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        w,
        h,
        image::Rgb([200, 30, 30]),
    ));
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn write_script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A system with one fake thumbnailer for `*.xyz` files; `script` gives its body for a root folder, with the output path as `$2`.
fn fixture(script: impl Fn(&Path) -> String, timeout: Duration) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let data = root.join("share");
    fs::create_dir_all(data.join("mime")).unwrap();
    fs::create_dir_all(data.join("thumbnailers")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("sample.png"), png_bytes(300, 200)).unwrap();
    fs::write(
        data.join("mime/globs2"),
        "50:image/png:*.png\n50:image/jpeg:*.jpg\n50:application/x-xyz:*.xyz\n50:text/plain:*.txt\n",
    )
    .unwrap();
    write_script(&root.join("bin/xyz-thumbnailer"), &script(root));
    fs::write(
        data.join("thumbnailers/xyz.thumbnailer"),
        "[Thumbnailer Entry]\nTryExec=xyz-thumbnailer\nExec=xyz-thumbnailer %i %o %s\nMimeType=application/x-xyz;\n",
    )
    .unwrap();
    let env = Env {
        cache_root: root.join("cache/thumbnails"),
        data_dirs: vec![data],
        path_dirs: vec![root.join("bin")],
        uri_of: Arc::new(file_uri),
    };
    let config = Config {
        app_name: "test".into(),
        app_version: "9".into(),
        ..Config::default()
    };
    let limits = Arc::new(Limits::new(config.max_file_bytes, timeout));
    let platform = Platform::new(
        env,
        &config,
        Arc::new(MemCache::new(1 << 20)),
        Arc::clone(&limits),
    );
    let processor = platform.processor();
    Fixture {
        tmp,
        platform,
        processor,
        limits,
    }
}

/// The usual fixture: the fake thumbnailer copies a prebuilt 300 by 200 PNG.
fn default_fixture() -> Fixture {
    fixture(
        |root| format!("cp {}/sample.png \"$2\"", root.display()),
        Duration::from_secs(10),
    )
}

fn entry_path(fx: &Fixture, path: &str) -> PathBuf {
    let (key, _) = fx
        .platform
        .store()
        .key_for(ThumbSize::Normal, Path::new(path));
    fx.root().join("cache/thumbnails").join(key.relative())
}

#[test]
fn the_environment_follows_the_xdg_variables() {
    let vars = |pairs: &'static [(&str, &str)]| {
        Env::from_vars(move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        })
    };
    let env = vars(&[
        ("HOME", "/home/u"),
        ("XDG_CACHE_HOME", "/cache"),
        ("XDG_DATA_HOME", "/data-home"),
        ("XDG_DATA_DIRS", "/a:/b:relative"),
        ("PATH", "/bin:/usr/bin"),
    ]);
    assert_eq!(env.cache_root, Path::new("/cache/thumbnails"));
    assert_eq!(
        env.data_dirs,
        [Path::new("/data-home"), Path::new("/a"), Path::new("/b")]
    );
    assert_eq!(env.path_dirs, [Path::new("/bin"), Path::new("/usr/bin")]);
    let defaults = vars(&[("HOME", "/home/u")]);
    assert_eq!(defaults.cache_root, Path::new("/home/u/.cache/thumbnails"));
    assert_eq!(
        defaults.data_dirs,
        [
            Path::new("/home/u/.local/share"),
            Path::new("/usr/local/share"),
            Path::new("/usr/share")
        ]
    );
    // A relative XDG_CACHE_HOME is ignored, as the base directory specification says.
    let relative = vars(&[("HOME", "/home/u"), ("XDG_CACHE_HOME", "cache")]);
    assert_eq!(relative.cache_root, Path::new("/home/u/.cache/thumbnails"));
}

#[test]
fn the_status_counts_the_thumbnailers_it_found() {
    let fx = default_fixture();
    let status = fx.platform.status();
    assert!(status.available);
    assert_eq!(status.flavour, Flavour::Freedesktop);
    let feature = |name: &str| {
        status
            .features
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .clone()
    };
    assert!(feature(FEATURE_CACHE).available);
    assert!(feature(FEATURE_BUILTIN).available);
    assert_eq!(feature(FEATURE_EXTERNAL).count, Some(1));
    let shell = feature(FEATURE_SHELL);
    assert!(!shell.available);
    assert_eq!(shell.reason.unwrap().kind, ReasonKind::OtherPlatform);
}

#[test]
fn a_system_without_thumbnailers_says_so() {
    let tmp = tempfile::tempdir().unwrap();
    let env = Env {
        cache_root: tmp.path().join("t"),
        data_dirs: vec![],
        path_dirs: vec![],
        uri_of: Arc::new(file_uri),
    };
    let platform = Platform::new(
        env,
        &Config::default(),
        Arc::new(MemCache::new(1)),
        Arc::new(Limits::new(1, Duration::from_secs(1))),
    );
    let status = platform.status();
    assert!(status.available, "the built-in generator still works");
    let external = status
        .features
        .iter()
        .find(|f| f.name == FEATURE_EXTERNAL)
        .unwrap();
    assert!(!external.available);
    assert_eq!(
        external.reason.as_ref().unwrap().kind,
        ReasonKind::NoThumbnailers
    );
}

#[test]
fn a_png_gets_a_standard_thumbnail_in_the_cache() {
    let fx = default_fixture();
    let path = fx.file("pic.png", &png_bytes(600, 300));
    let Outcome::Ready { url } = fx.process(&path, 5_500) else {
        panic!("expected a thumbnail");
    };
    let entry = entry_path(&fx, &path);
    assert!(url.contains(&format!(
        "normal/{}",
        entry.file_name().unwrap().to_string_lossy()
    )));
    assert!(url.ends_with("?v=5"));
    let meta = cache::read_meta(&entry).unwrap();
    assert_eq!(meta.uri, file_uri(Path::new(&path)));
    assert_eq!(meta.mtime_secs, 5);
    assert_eq!(meta.file_size, Some(fs::metadata(&path).unwrap().len()));
    let decoded = image::open(&entry).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (128, 64));
}

#[test]
fn a_second_request_is_a_cache_hit_that_decodes_nothing() {
    let fx = default_fixture();
    let path = fx.file("pic.png", &png_bytes(300, 300));
    let first = fx.process(&path, 1_000);
    // Replace the file with garbage and keep the time: a hit must not look at the file's content.
    fs::write(&path, b"garbage").unwrap();
    assert_eq!(fx.process(&path, 1_000), first);
}

#[test]
fn a_changed_mtime_makes_the_thumbnail_stale_and_it_is_made_again() {
    let fx = default_fixture();
    let path = fx.file("pic.png", &png_bytes(300, 300));
    assert!(matches!(fx.process(&path, 1_000), Outcome::Ready { .. }));
    fs::write(&path, png_bytes(100, 50)).unwrap();
    assert!(matches!(fx.process(&path, 9_000), Outcome::Ready { .. }));
    let entry = entry_path(&fx, &path);
    assert_eq!(cache::read_meta(&entry).unwrap().mtime_secs, 9);
    let decoded = image::open(&entry).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (100, 50));
}

#[test]
fn a_broken_image_fails_once_and_is_remembered_until_it_changes() {
    let fx = default_fixture();
    let path = fx.file("broken.png", &png_bytes(64, 64)[..50]);
    assert!(matches!(fx.process(&path, 1_000), Outcome::Failed { .. }));
    let (key, _) = fx
        .platform
        .store()
        .key_for(ThumbSize::Normal, Path::new(&path));
    let fail = fx
        .root()
        .join("cache/thumbnails/fail/test-9")
        .join(format!("{}.png", key.md5));
    assert!(fail.is_file());
    // The file is fixed but the time is the same: the recorded failure answers, without decoding.
    fs::write(&path, png_bytes(64, 64)).unwrap();
    let Outcome::Failed { reason } = fx.process(&path, 1_000) else {
        panic!("expected the remembered failure");
    };
    assert!(reason.contains("before"));
    // A new modified time retries, and success removes the record.
    assert!(matches!(fx.process(&path, 2_000), Outcome::Ready { .. }));
    assert!(!fail.exists());
}

#[test]
fn remote_missing_oversized_and_unknown_files_are_told_apart() {
    let fx = default_fixture();
    for remote in [
        "sftp://host/a.png",
        "smb://h/s/a.png",
        "relative/a.png",
        "https://x/a.png",
    ] {
        assert_eq!(
            fx.process(remote, 0),
            Outcome::Skipped {
                why: SkipWhy::Remote
            },
            "{remote}"
        );
    }
    assert!(matches!(
        fx.process("/no/such/file.png", 0),
        Outcome::Failed { .. }
    ));
    let text = fx.file("notes.txt", b"just text");
    assert_eq!(
        fx.process(&text, 0),
        Outcome::Skipped {
            why: SkipWhy::NoGenerator
        }
    );
    assert_eq!(
        fx.process(fx.root().join("files").to_str().unwrap(), 0),
        Outcome::Skipped {
            why: SkipWhy::NoGenerator
        }
    );
    let big = fx.file("big.png", &png_bytes(300, 300));
    fx.limits.set_max_file_bytes(10);
    assert_eq!(
        fx.process(&big, 0),
        Outcome::Skipped {
            why: SkipWhy::TooLarge
        }
    );
    // Nothing was written for a skip.
    assert!(!entry_path(&fx, &big).exists());
}

#[test]
fn an_image_without_a_telling_name_is_found_by_its_content() {
    let fx = default_fixture();
    let path = fx.file("no-extension", &png_bytes(300, 300));
    assert!(matches!(fx.process(&path, 0), Outcome::Ready { .. }));
}

#[test]
fn an_external_thumbnailer_handles_what_the_built_in_cannot() {
    let fx = default_fixture();
    let path = fx.file("thing.xyz", b"proprietary");
    assert!(matches!(fx.process(&path, 3_000), Outcome::Ready { .. }));
    let decoded = image::open(entry_path(&fx, &path)).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (128, 85));
    let meta = cache::read_meta(&entry_path(&fx, &path)).unwrap();
    assert_eq!(meta.mtime_secs, 3);
    // The scratch file is gone.
    let leftovers = fs::read_dir(fx.root().join("cache/thumbnails/normal"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with('.')
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn a_thumbnailer_that_fails_or_writes_rubbish_is_a_remembered_failure() {
    let fx = fixture(
        |_| "echo not an image > \"$2\"".to_string(),
        Duration::from_secs(10),
    );
    let path = fx.file("a.xyz", b"x");
    assert!(matches!(fx.process(&path, 1), Outcome::Failed { .. }));
    write_script(&fx.root().join("bin/xyz-thumbnailer"), "exit 2");
    // The same time answers from the record; a new time runs the new script and fails on its status.
    let Outcome::Failed { reason } = fx.process(&path, 1) else {
        panic!("expected a failure");
    };
    assert!(reason.contains("before"), "{reason}");
    let Outcome::Failed { reason } = fx.process(&path, 2_000) else {
        panic!("expected a failure");
    };
    assert!(reason.contains("status 2"), "{reason}");
}

#[test]
fn a_thumbnailer_that_sleeps_is_killed_at_the_timeout() {
    let fx = fixture(|_| "sleep 30".to_string(), Duration::from_millis(250));
    let path = fx.file("slow.xyz", b"x");
    let started = Instant::now();
    let outcome = fx.process(&path, 1);
    assert!(
        matches!(outcome, Outcome::Failed { ref reason } if reason.contains("timed out")),
        "{outcome:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_cancel_stops_a_running_thumbnailer_and_records_nothing() {
    let fx = fixture(|_| "sleep 30".to_string(), Duration::from_secs(60));
    let path = fx.file("slow.xyz", b"x");
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let processor = fx.platform.processor();
    let request = fx.request(&path, 1);
    let handle = std::thread::spawn(move || processor.process(&request, &flag));
    std::thread::sleep(Duration::from_millis(200));
    let started = Instant::now();
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(handle.join().unwrap(), Outcome::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2));
    let (key, _) = fx
        .platform
        .store()
        .key_for(ThumbSize::Normal, Path::new(&path));
    assert_eq!(
        fx.platform
            .store()
            .lookup(&key, &file_uri(Path::new(&path)), 0),
        Lookup::Miss
    );
}

/// A platform over the real system's thumbnailers and `PATH`, with a private cache, so a type the installed tools handle is tried for real.
fn system_platform(tmp: &tempfile::TempDir) -> Platform {
    let cache = tmp.path().join("cache");
    let mut env = Env::from_vars(|name| match name {
        "XDG_CACHE_HOME" => Some(cache.to_string_lossy().into_owned()),
        "XDG_DATA_HOME" => Some(tmp.path().join("no-data").to_string_lossy().into_owned()),
        other => std::env::var(other).ok(),
    });
    env.cache_root = cache.join("thumbnails");
    let config = Config {
        app_name: "test".into(),
        app_version: "9".into(),
        ..Config::default()
    };
    let limits = Arc::new(Limits::new(config.max_file_bytes, Duration::from_secs(30)));
    Platform::new(env, &config, Arc::new(MemCache::new(1 << 20)), limits)
}

fn on_path(program: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    thumbnailer::program_exists(program, &std::env::split_paths(&path).collect::<Vec<_>>())
}

/// Runs the real installed thumbnailer for `path` and asserts a thumbnail results.
fn assert_system_thumbnail(path: &Path) {
    let tmp = tempfile::tempdir().unwrap();
    let processor = system_platform(&tmp).processor();
    let path = path.to_string_lossy().into_owned();
    let request = ThumbRequest {
        key: path.clone(),
        path,
        size: ThumbSize::Normal,
        mtime_ms: 1_700_000_000_000,
    };
    let outcome = processor.process(&request, &AtomicBool::new(false));
    assert!(
        matches!(outcome, Outcome::Ready { .. }),
        "expected a thumbnail, got {outcome:?}"
    );
}

#[test]
fn a_video_gets_a_thumbnail_from_the_installed_thumbnailer() {
    if !on_path("ffmpeg") || !on_path("ffmpegthumbnailer") {
        eprintln!("skipped: ffmpeg and ffmpegthumbnailer are not both installed");
        return;
    }
    if !Path::new("/usr/share/thumbnailers/ffmpegthumbnailer.thumbnailer").exists() {
        eprintln!("skipped: no ffmpegthumbnailer.thumbnailer is installed");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let video = tmp.path().join("clip.mp4");
    let made = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=160x120:rate=5",
        ])
        .args(["-t", "1", "-pix_fmt", "yuv420p", "-y"])
        .arg(&video)
        .status();
    if !made.is_ok_and(|status| status.success()) {
        eprintln!("skipped: ffmpeg could not make a sample video (no encoder?)");
        return;
    }
    assert_system_thumbnail(&video);
}

#[test]
fn a_pdf_gets_a_thumbnail_from_the_installed_thumbnailer() {
    if !on_path("evince-thumbnailer")
        || !Path::new("/usr/share/thumbnailers/evince.thumbnailer").exists()
    {
        eprintln!("skipped: evince-thumbnailer is not installed");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let pdf = tmp.path().join("page.pdf");
    // The smallest valid one-page PDF, with a cross-reference table evince can read.
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>",
    ];
    let mut body = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (n, object) in objects.iter().enumerate() {
        offsets.push(body.len());
        body.push_str(&format!("{} 0 obj\n{object}\nendobj\n", n + 1));
    }
    let xref = body.len();
    body.push_str("xref\n0 4\n0000000000 65535 f \n");
    for offset in offsets {
        body.push_str(&format!("{offset:010} 00000 n \n"));
    }
    body.push_str(&format!(
        "trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
    ));
    fs::write(&pdf, body).unwrap();
    assert_system_thumbnail(&pdf);
}

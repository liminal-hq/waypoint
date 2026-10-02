// Exercises the plugin through Tauri's mock runtime over a temporary environment
//
// The mock runtime has no capability file, so the IPC layer would refuse plugin commands; these tests call the same `Thumbnails` methods the commands delegate to, and check the JSON the commands would send and receive. Nothing here touches the real `~/.cache/thumbnails`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "linux")]

use std::fs;
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tauri::http::{Method, StatusCode};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::App;
use tauri_plugin_thumbnails::linux::Env;
use tauri_plugin_thumbnails::scheme::respond;
use tauri_plugin_thumbnails::{
    cache, Config, Flavour, SkipWhy, ThumbEvent, ThumbRequest, ThumbSize, ThumbnailsExt,
    FEATURE_BUILTIN, FEATURE_CACHE, FEATURE_EXTERNAL, FEATURE_SHELL,
};

struct Fixture {
    tmp: tempfile::TempDir,
    app: App<MockRuntime>,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(tmp.path()).unwrap();
    fs::create_dir_all(root.join("files")).unwrap();
    let env = Env {
        cache_root: root.join("cache/thumbnails"),
        data_dirs: vec![root.join("share")],
        path_dirs: vec![],
        uri_of: Arc::new(cache::file_uri),
    };
    let app = mock_builder()
        .plugin(tauri_plugin_thumbnails::init_with_env(
            Config {
                workers: Some(2),
                ..Config::default()
            },
            env,
        ))
        .build(mock_context(noop_assets()))
        .expect("the plugin should initialise");
    Fixture { tmp, app }
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(w, h, image::Rgb([10, 120, 220]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn request_one(fx: &Fixture, path: &Path) -> ThumbEvent {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    fx.app.thumbnails().request(
        vec![ThumbRequest {
            key: "k".into(),
            path: path.to_string_lossy().into_owned(),
            size: ThumbSize::Large,
            mtime_ms: 1_000,
        }],
        Arc::new(move |event| {
            let _ = tx.lock().unwrap().send(event);
        }),
    );
    rx.recv_timeout(Duration::from_secs(10)).expect("an event")
}

fn url_path(url: &str) -> &str {
    let without_scheme = url
        .strip_prefix("thumb://localhost")
        .expect("the thumb scheme");
    without_scheme.split('?').next().unwrap()
}

#[test]
fn the_status_lists_every_feature_and_the_flavour() {
    let fx = fixture();
    let status = fx.app.thumbnails().get_status();
    assert!(status.available);
    assert_eq!(status.flavour, Flavour::Freedesktop);
    let names: Vec<&str> = status.features.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            FEATURE_CACHE,
            FEATURE_BUILTIN,
            FEATURE_EXTERNAL,
            FEATURE_SHELL
        ]
    );
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["features"][2]["reason"]["kind"], "noThumbnailers");
    assert_eq!(json["features"][3]["reason"]["kind"], "otherPlatform");
    assert_eq!(json["flavour"], "freedesktop");
}

#[test]
fn requests_and_events_use_the_documented_json() {
    let request: ThumbRequest = serde_json::from_value(json!({
        "key": "7", "path": "/a/b.png", "size": "x-large", "mtimeMs": 1234
    }))
    .unwrap();
    assert_eq!(request.size, ThumbSize::XLarge);
    assert_eq!(request.mtime_ms, 1234);
    for (size, text) in [
        (ThumbSize::Normal, "normal"),
        (ThumbSize::Large, "large"),
        (ThumbSize::XLarge, "x-large"),
        (ThumbSize::XXLarge, "xx-large"),
    ] {
        assert_eq!(serde_json::to_value(size).unwrap(), json!(text));
    }
    assert_eq!(
        serde_json::to_value(ThumbEvent::Ready {
            key: "7".into(),
            url: "thumb://localhost/x".into()
        })
        .unwrap(),
        json!({"kind": "ready", "key": "7", "url": "thumb://localhost/x"})
    );
    assert_eq!(
        serde_json::to_value(ThumbEvent::Failed {
            key: "7".into(),
            reason: "bad".into()
        })
        .unwrap(),
        json!({"kind": "failed", "key": "7", "reason": "bad"})
    );
    assert_eq!(
        serde_json::to_value(ThumbEvent::Skipped {
            key: "7".into(),
            why: SkipWhy::TooLarge
        })
        .unwrap(),
        json!({"kind": "skipped", "key": "7", "why": "tooLarge"})
    );
}

#[test]
fn a_requested_thumbnail_is_served_by_its_cache_key() {
    let fx = fixture();
    let path = fx.tmp.path().join("files/photo.png");
    fs::write(&path, png(500, 400)).unwrap();
    let ThumbEvent::Ready { key, url } = request_one(&fx, &path) else {
        panic!("expected a thumbnail");
    };
    assert_eq!(key, "k");
    assert!(url.starts_with("thumb://localhost/large/"));
    assert!(
        !url.contains("photo"),
        "the address names the cache entry, never the file: {url}"
    );
    let response = respond(fx.app.thumbnails(), &Method::GET, url_path(&url));
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    let decoded = image::load_from_memory(response.body()).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (256, 205));
    // The same bytes are in the shared cache folder, under the standard's name.
    let on_disk = fs::read(
        fx.tmp
            .path()
            .join("cache/thumbnails")
            .join(url_path(&url).trim_start_matches('/')),
    )
    .unwrap();
    assert_eq!(&on_disk, response.body());
    // HEAD says how big it is and sends nothing.
    let head = respond(fx.app.thumbnails(), &Method::HEAD, url_path(&url));
    assert_eq!(head.status(), StatusCode::OK);
    assert!(head.body().is_empty());
    assert_eq!(head.headers()["content-length"], on_disk.len().to_string());
    // Other methods are refused.
    assert_eq!(
        respond(fx.app.thumbnails(), &Method::POST, url_path(&url)).status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}

#[test]
fn the_scheme_refuses_anything_that_is_not_a_cache_key_in_the_cache() {
    let fx = fixture();
    // A real file outside the cache that a path trick would reach.
    fs::write(fx.tmp.path().join("secret.png"), png(8, 8)).unwrap();
    fs::write(fx.tmp.path().join("cache/secret.png"), b"secret").ok();
    let path = fx.tmp.path().join("files/photo.png");
    fs::write(&path, png(50, 50)).unwrap();
    let ThumbEvent::Ready { url, .. } = request_one(&fx, &path) else {
        panic!("expected a thumbnail");
    };
    let good = url_path(&url).to_string();
    let md5 = good.rsplit('/').next().unwrap().to_string();
    let tmp = fx.tmp.path().display().to_string();
    for bad in [
        "/".to_string(),
        "/large".to_string(),
        "/large/".to_string(),
        format!("/normal/{md5}"),
        // Well-formed but not in the cache.
        format!("/normal/{md5}.png"),
        "/xx-large/00000000000000000000000000000000.png".to_string(),
        "/large/../large/".to_string() + &md5,
        format!("/large/../{md5}"),
        format!("/large/%2e%2e/large/{md5}"),
        "/large/..%2f..%2fsecret.png".to_string(),
        "/../secret.png".to_string(),
        "/large/../../secret.png".to_string(),
        "/large/../../../../../../etc/passwd".to_string(),
        format!("{tmp}/secret.png"),
        format!("/{tmp}/secret.png"),
        "/large/secret.png".to_string(),
        "/fail/test.png".to_string(),
        format!("/large\\{md5}"),
        format!("/large/{md5}/"),
        format!("/large/{}", md5.to_uppercase()),
        "file:///etc/passwd".to_string(),
        "".to_string(),
    ] {
        assert_eq!(
            respond(fx.app.thumbnails(), &Method::GET, &bad).status(),
            StatusCode::NOT_FOUND,
            "{bad:?} must not be served"
        );
    }
    assert_eq!(
        respond(fx.app.thumbnails(), &Method::GET, &good).status(),
        StatusCode::OK
    );
}

#[test]
fn a_remote_path_is_skipped_through_the_whole_pipeline() {
    let fx = fixture();
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    fx.app.thumbnails().request(
        vec![ThumbRequest {
            key: "r".into(),
            path: "sftp://host/a.png".into(),
            size: ThumbSize::Normal,
            mtime_ms: 0,
        }],
        Arc::new(move |event| {
            let _ = tx.lock().unwrap().send(event);
        }),
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ThumbEvent::Skipped {
            key: "r".into(),
            why: SkipWhy::Remote
        }
    );
}

#[test]
fn cancel_and_prioritise_are_safe_for_unknown_tickets() {
    let fx = fixture();
    let thumbs = fx.app.thumbnails();
    assert!(!thumbs.cancel(tauri_plugin_thumbnails::Ticket(12345)));
    thumbs.prioritise(tauri_plugin_thumbnails::Ticket(12345), &["a".to_string()]);
}

#[test]
fn the_unsupported_platform_reports_every_feature_unavailable() {
    use tauri_plugin_thumbnails::engine::{Limits, Outcome};
    use tauri_plugin_thumbnails::memcache::MemCache;
    use tauri_plugin_thumbnails::unsupported::{Env, Platform};
    use tauri_plugin_thumbnails::ReasonKind;

    let platform = Platform::new(
        Env,
        &Config::default(),
        Arc::new(MemCache::new(1)),
        Arc::new(Limits::new(1, Duration::from_secs(1))),
    );
    let status = platform.status();
    assert!(!status.available);
    assert_eq!(status.flavour, Flavour::Unsupported);
    assert_eq!(status.features.len(), 4);
    assert!(status.features.iter().all(|f| !f.available
        && f.reason
            .as_ref()
            .is_some_and(|r| r.kind == ReasonKind::Unsupported)));
    assert_eq!(status.reason.unwrap().kind, ReasonKind::Unsupported);
    let request = ThumbRequest {
        key: "k".into(),
        path: "/a.png".into(),
        size: ThumbSize::Normal,
        mtime_ms: 0,
    };
    assert_eq!(
        platform
            .processor()
            .process(&request, &std::sync::atomic::AtomicBool::new(false)),
        Outcome::Skipped {
            why: SkipWhy::Unsupported
        }
    );
}

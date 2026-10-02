// Measures thumbnails per second, peak memory and cache-hit latency over thousands of generated images; run with `--ignored --nocapture`
//
// `THUMB_BENCH_COUNT` sets the number of images (2000 by default). The big JPEGs are hard links to a few generated 12 megapixel photographs, so they cost almost no disk.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(target_os = "linux")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use image::{ImageEncoder, Rgb, RgbImage};
use tauri_plugin_thumbnails::engine::{Engine, Limits, Outcome};
use tauri_plugin_thumbnails::linux::{Env, Platform};
use tauri_plugin_thumbnails::memcache::MemCache;
use tauri_plugin_thumbnails::{cache, Config, ThumbEvent, ThumbRequest, ThumbSize};

fn count() -> usize {
    std::env::var("THUMB_BENCH_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000)
}

fn peak_rss_mib() -> f64 {
    let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| {
            rest.trim()
                .trim_end_matches("kB")
                .trim()
                .parse::<f64>()
                .ok()
        })
        .map_or(0.0, |kib| kib / 1024.0)
}

/// A photograph-like image: smooth shapes with noise, which compresses like a real photograph rather than like a flat fill.
fn photo(w: u32, h: u32, seed: u32) -> RgbImage {
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    RgbImage::from_fn(w, h, |x, y| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let noise = (state & 0x1f) as i32;
        let fx = x as f32 / w as f32;
        let fy = y as f32 / h as f32;
        let r = (128.0 + 100.0 * (fx * 9.0 + seed as f32).sin()) as i32 + noise;
        let g = (128.0 + 100.0 * (fy * 7.0).cos()) as i32 + noise;
        let b = (128.0 + 100.0 * ((fx + fy) * 5.0).sin()) as i32 + noise;
        Rgb([
            r.clamp(0, 255) as u8,
            g.clamp(0, 255) as u8,
            b.clamp(0, 255) as u8,
        ])
    })
}

fn write_jpeg(path: &Path, image: &RgbImage) {
    let file = fs::File::create(path).unwrap();
    image::codecs::jpeg::JpegEncoder::new_with_quality(file, 88)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
}

fn write_png(path: &Path, image: &RgbImage) {
    let file = fs::File::create(path).unwrap();
    image::codecs::png::PngEncoder::new_with_quality(
        file,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Sub,
    )
    .write_image(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();
}

struct Bench {
    root: PathBuf,
    platform: Platform,
    _tmp: tempfile::TempDir,
}

fn bench(workers: usize) -> (Bench, Engine) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let env = Env {
        cache_root: root.join("cache/thumbnails"),
        data_dirs: vec![],
        path_dirs: vec![],
        uri_of: Arc::new(cache::file_uri),
    };
    let config = Config::default();
    let platform = Platform::new(
        env,
        &config,
        Arc::new(MemCache::new(config.memory_cache_bytes)),
        Arc::new(Limits::new(config.max_file_bytes, config.external_timeout)),
    );
    let engine = Engine::new(workers, platform.processor());
    (
        Bench {
            root,
            platform,
            _tmp: tmp,
        },
        engine,
    )
}

fn requests(paths: &[PathBuf], size: ThumbSize) -> Vec<ThumbRequest> {
    paths
        .iter()
        .map(|path| ThumbRequest {
            key: path.to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            size,
            mtime_ms: 1_700_000_000_000,
        })
        .collect()
}

fn run_all(engine: &Engine, items: Vec<ThumbRequest>) -> Duration {
    let total = items.len();
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let started = Instant::now();
    engine.request(
        items,
        Arc::new(move |event| {
            let _ = tx.lock().unwrap().send(event);
        }),
    );
    let mut ready = 0;
    for _ in 0..total {
        match rx
            .recv_timeout(Duration::from_secs(600))
            .expect("every item answers")
        {
            ThumbEvent::Ready { .. } => ready += 1,
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(ready, total);
    started.elapsed()
}

fn report(label: &str, n: usize, elapsed: Duration) {
    println!(
        "{label}: {n} thumbnails in {:.2} s = {:.0} per second; peak RSS so far {:.0} MiB",
        elapsed.as_secs_f64(),
        n as f64 / elapsed.as_secs_f64(),
        peak_rss_mib()
    );
}

fn p95_ms(bench: &Bench, items: &[ThumbRequest]) -> (f64, f64) {
    let processor = bench.platform.processor();
    let never = std::sync::atomic::AtomicBool::new(false);
    let mut times: Vec<f64> = items
        .iter()
        .map(|request| {
            let started = Instant::now();
            assert!(matches!(
                processor.process(request, &never),
                Outcome::Ready { .. }
            ));
            started.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (
        times[times.len() / 2],
        times[(times.len() * 95 / 100).min(times.len() - 1)],
    )
}

#[test]
#[ignore = "a benchmark: run with --ignored --nocapture"]
fn small_pngs_per_second() {
    let n = count();
    let workers = Config::default().worker_count();
    let (b, engine) = bench(workers);
    let dir = b.root.join("pngs");
    fs::create_dir_all(&dir).unwrap();
    let templates: Vec<RgbImage> = (0..8).map(|i| photo(256, 192, i)).collect();
    let paths: Vec<PathBuf> = (0..n)
        .map(|i| {
            let path = dir.join(format!("img-{i:05}.png"));
            write_png(&path, &templates[i % templates.len()]);
            path
        })
        .collect();
    let items = requests(&paths, ThumbSize::Normal);
    println!(
        "{workers} workers, {} logical cores",
        std::thread::available_parallelism().unwrap()
    );
    let cold = run_all(&engine, items.clone());
    report("small PNGs, cold", n, cold);
    let (p50, p95) = p95_ms(&b, &items);
    println!("small PNGs, cache hit: p50 {p50:.3} ms, p95 {p95:.3} ms");
}

#[test]
#[ignore = "a benchmark: run with --ignored --nocapture"]
fn twelve_megapixel_jpegs_per_second() {
    let n = count();
    let workers = Config::default().worker_count();
    let (b, engine) = bench(workers);
    let dir = b.root.join("jpegs");
    fs::create_dir_all(&dir).unwrap();
    let originals: Vec<PathBuf> = (0..4)
        .map(|i| {
            let path = dir.join(format!("original-{i}.jpg"));
            write_jpeg(&path, &photo(4000, 3000, i));
            path
        })
        .collect();
    println!(
        "one 12 MP JPEG is {} KiB",
        fs::metadata(&originals[0]).unwrap().len() / 1024
    );
    // Hard links: each name is its own file (its own cache key) with no extra disk.
    let paths: Vec<PathBuf> = (0..n)
        .map(|i| {
            let path = dir.join(format!("photo-{i:05}.jpg"));
            fs::hard_link(&originals[i % originals.len()], &path).unwrap();
            path
        })
        .collect();
    let items = requests(&paths, ThumbSize::Large);
    println!(
        "{workers} workers, {} logical cores",
        std::thread::available_parallelism().unwrap()
    );
    let cold = run_all(&engine, items.clone());
    report("12 MP JPEGs (Large, 256 px), cold", n, cold);
    let (p50, p95) = p95_ms(&b, &items);
    println!("12 MP JPEGs, cache hit: p50 {p50:.3} ms, p95 {p95:.3} ms");
}

// THROWAWAY milestone 0 spikes: Rust-held listings, range fetch, and streaming directory scans
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    cmp::Ordering as CmpOrdering,
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{
    ipc::{Channel, Response},
    State,
};

#[derive(Clone)]
pub struct Entry {
    name: String,
    size: u64,
    modified: i64,
    kind: u8,
}

pub struct Listing {
    entries: Vec<Entry>,
    /// The current view order: indices into `entries`.
    order: Mutex<Vec<u32>>,
}

#[derive(Default)]
pub struct Spike {
    next: AtomicU64,
    listings: Mutex<HashMap<u64, Arc<Listing>>>,
    watcher_events: Arc<AtomicU64>,
    watchers: Mutex<Vec<notify::RecommendedWatcher>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    index: u32,
    name: String,
    size: u64,
    modified: i64,
    kind: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResult {
    handle: u64,
    count: usize,
    build_ms: f64,
}

/// A small deterministic generator, so runs are comparable.
struct Xorshift(u64);
impl Xorshift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

const WORDS: [&str; 12] = [
    "report", "photo", "invoice", "draft", "backup", "notes", "archive", "screenshot", "data",
    "build", "log", "track",
];
const EXTS: [&str; 8] = ["txt", "jpg", "pdf", "rs", "ts", "mp3", "zip", "md"];

fn cmp_names(a: &str, b: &str) -> CmpOrdering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

#[tauri::command]
pub fn spike_create_synthetic(count: usize, state: State<'_, Spike>) -> CreateResult {
    let started = Instant::now();
    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let r = rng.next();
        let word = WORDS[(r % WORDS.len() as u64) as usize];
        let ext = EXTS[((r >> 8) % EXTS.len() as u64) as usize];
        // Shuffled numbering, so the natural order is not already sorted.
        let number = (r >> 16) % 1_000_000;
        entries.push(Entry {
            name: format!("{word}-{number:06}-{i}.{ext}"),
            size: (r >> 24) % 50_000_000,
            modified: 1_600_000_000 + ((r >> 40) % 100_000_000) as i64,
            kind: if r % 17 == 0 { 1 } else { 0 },
        });
    }
    let order: Vec<u32> = (0..count as u32).collect();
    let handle = state.next.fetch_add(1, Ordering::SeqCst) + 1;
    let listing = Arc::new(Listing {
        entries,
        order: Mutex::new(order),
    });
    state.listings.lock().unwrap().insert(handle, listing);
    CreateResult {
        handle,
        count,
        build_ms: started.elapsed().as_secs_f64() * 1000.0,
    }
}

fn listing(state: &State<'_, Spike>, handle: u64) -> Result<Arc<Listing>, String> {
    state
        .listings
        .lock()
        .unwrap()
        .get(&handle)
        .cloned()
        .ok_or_else(|| format!("no listing {handle}"))
}

/// Sorts the view order in place and returns how long it took, in milliseconds.
#[tauri::command]
pub fn spike_sort(handle: u64, by: String, desc: bool, state: State<'_, Spike>) -> Result<f64, String> {
    let listing = listing(&state, handle)?;
    let started = Instant::now();
    let mut order = listing.order.lock().unwrap();
    let entries = &listing.entries;
    match by.as_str() {
        "name" => order.sort_unstable_by(|&a, &b| {
            cmp_names(&entries[a as usize].name, &entries[b as usize].name)
        }),
        "size" => order.sort_unstable_by_key(|&a| entries[a as usize].size),
        "modified" => order.sort_unstable_by_key(|&a| entries[a as usize].modified),
        _ => return Err(format!("unknown sort key {by}")),
    }
    if desc {
        order.reverse();
    }
    Ok(started.elapsed().as_secs_f64() * 1000.0)
}

fn row(listing: &Listing, order: &[u32], at: usize) -> Row {
    let e = &listing.entries[order[at] as usize];
    Row {
        index: at as u32,
        name: e.name.clone(),
        size: e.size,
        modified: e.modified,
        kind: e.kind,
    }
}

/// A range of rows as JSON, the ordinary Tauri command path.
#[tauri::command]
pub fn spike_range(handle: u64, start: usize, count: usize, state: State<'_, Spike>) -> Result<Vec<Row>, String> {
    let listing = listing(&state, handle)?;
    let order = listing.order.lock().unwrap();
    let end = (start + count).min(order.len());
    Ok((start..end).map(|at| row(&listing, &order, at)).collect())
}

/// The same range as packed bytes through a raw response: `u32 name length, u64 size, i64 modified,
/// u8 kind, name bytes` per row, little-endian.
#[tauri::command]
pub fn spike_range_bin(handle: u64, start: usize, count: usize, state: State<'_, Spike>) -> Result<Response, String> {
    let listing = listing(&state, handle)?;
    let order = listing.order.lock().unwrap();
    let end = (start + count).min(order.len());
    let mut out = Vec::with_capacity((end - start) * 48);
    for at in start..end {
        let e = &listing.entries[order[at] as usize];
        out.extend_from_slice(&(e.name.len() as u32).to_le_bytes());
        out.extend_from_slice(&e.size.to_le_bytes());
        out.extend_from_slice(&e.modified.to_le_bytes());
        out.push(e.kind);
        out.extend_from_slice(e.name.as_bytes());
    }
    Ok(Response::new(out))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanBatch {
    rows: Vec<Row>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    entries: usize,
    batches: usize,
    first_batch_ms: f64,
    total_ms: f64,
}

fn scan_entry(index: u32, entry: fs::DirEntry) -> Row {
    let meta = entry.metadata().ok();
    Row {
        index,
        name: entry.file_name().to_string_lossy().into_owned(),
        size: meta.as_ref().map_or(0, |m| m.len()),
        modified: meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64),
        kind: meta.as_ref().map_or(0, |m| u8::from(m.is_dir())),
    }
}

/// Reads a directory with `read_dir` plus metadata and streams it in batches over a `Channel`.
#[tauri::command]
pub async fn spike_scan(path: String, batch: usize, on_batch: Channel<ScanBatch>) -> Result<ScanSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let mut first_batch_ms = 0.0;
        let (mut entries, mut batches) = (0usize, 0usize);
        let mut pending: Vec<Row> = Vec::with_capacity(batch);
        for entry in fs::read_dir(PathBuf::from(&path)).map_err(|e| e.to_string())? {
            let Ok(entry) = entry else { continue };
            pending.push(scan_entry(entries as u32, entry));
            entries += 1;
            if pending.len() >= batch {
                if batches == 0 {
                    first_batch_ms = started.elapsed().as_secs_f64() * 1000.0;
                }
                batches += 1;
                on_batch
                    .send(ScanBatch {
                        rows: std::mem::replace(&mut pending, Vec::with_capacity(batch)),
                    })
                    .map_err(|e| e.to_string())?;
            }
        }
        if !pending.is_empty() {
            if batches == 0 {
                first_batch_ms = started.elapsed().as_secs_f64() * 1000.0;
            }
            batches += 1;
            on_batch.send(ScanBatch { rows: pending }).map_err(|e| e.to_string())?;
        }
        Ok(ScanSummary {
            entries,
            batches,
            first_batch_ms,
            total_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The same filesystem work with no IPC at all, to separate disk cost from transport cost.
#[tauri::command]
pub async fn spike_scan_baseline(path: String) -> Result<ScanSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let mut entries = 0usize;
        let mut first_ms = 0.0;
        for entry in fs::read_dir(PathBuf::from(&path)).map_err(|e| e.to_string())? {
            let Ok(entry) = entry else { continue };
            let _ = scan_entry(entries as u32, entry);
            if entries == 0 {
                first_ms = started.elapsed().as_secs_f64() * 1000.0;
            }
            entries += 1;
        }
        Ok(ScanSummary {
            entries,
            batches: 0,
            first_batch_ms: first_ms,
            total_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Creates `count` empty files in `path`, for the scan spike's fixture.
#[tauri::command]
pub fn spike_make_dir(path: String, count: usize) -> Result<f64, String> {
    let started = Instant::now();
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    for i in 0..count {
        fs::File::create(PathBuf::from(&path).join(format!("entry-{i:07}.dat"))).map_err(|e| e.to_string())?;
    }
    Ok(started.elapsed().as_secs_f64() * 1000.0)
}

/// Starts an inotify watcher on `path` that counts events.
#[tauri::command]
pub fn spike_watch(path: String, state: State<'_, Spike>) -> Result<(), String> {
    let counter = state.watcher_events.clone();
    counter.store(0, Ordering::SeqCst);
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if event.is_ok() {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    })
    .map_err(|e| e.to_string())?;
    watcher
        .watch(&PathBuf::from(path), RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;
    state.watchers.lock().unwrap().push(watcher);
    Ok(())
}

#[tauri::command]
pub fn spike_watch_events(state: State<'_, Spike>) -> u64 {
    state.watcher_events.load(Ordering::SeqCst)
}

/// Stops every watcher.
#[tauri::command]
pub fn spike_unwatch(state: State<'_, Spike>) {
    state.watchers.lock().unwrap().clear();
}

/// Creates then removes `count` files in `path`, to generate watcher events while a scan runs.
#[tauri::command]
pub async fn spike_churn(path: String, count: usize) -> Result<f64, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let dir = PathBuf::from(path);
        for i in 0..count {
            let file = dir.join(format!("churn-{i:06}.tmp"));
            fs::File::create(&file).map_err(|e| e.to_string())?;
            fs::remove_file(&file).map_err(|e| e.to_string())?;
        }
        Ok(started.elapsed().as_secs_f64() * 1000.0)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Removes a fixture directory.
#[tauri::command]
pub fn spike_remove_dir(path: String) -> Result<(), String> {
    fs::remove_dir_all(path).map_err(|e| e.to_string())
}

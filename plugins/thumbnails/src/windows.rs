// The Windows implementation: thumbnails from the shell, on single-threaded-apartment worker threads, cached under the app's local cache folder
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shell's `IShellItemImageFactory` makes the same thumbnails Explorer shows, through the thumbnail handlers the user has installed. Its COM objects need a single-threaded apartment, so each engine worker enters one when it starts ([`Processor::on_worker_start`]) and makes its own calls there; there is no marshalling to a main thread.
//!
//! A cloud placeholder (`FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` or `RECALL_ON_OPEN`, or offline) is only ever asked for a thumbnail the shell already has (`SIIGBF_INCACHEONLY`), because making one would download the file; with none cached, the request is skipped as `cloud`.

use std::any::Any;
use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};
use windows::core::HSTRING;
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF, SIIGBF_INCACHEONLY,
    SIIGBF_THUMBNAILONLY,
};

use crate::builtin::Rendered;
use crate::cache::{file_uri, Meta, Store, UriFn};
use crate::engine::{Limits, Outcome, Processor};
use crate::memcache::MemCache;
use crate::models::{
    Config, FeatureStatus, Flavour, PluginStatus, ReasonKind, SkipWhy, ThumbRequest,
    FEATURE_BUILTIN, FEATURE_CACHE, FEATURE_EXTERNAL, FEATURE_SHELL,
};
use crate::pipeline::{self, Start};
use crate::winmap::{self, ShellFailure, ShellMode};

/// Everything about the system the plugin reads, injected so a test can use a temporary directory.
#[derive(Clone)]
pub struct Env {
    /// The folder that holds `normal`, `large`, `x-large`, `xx-large` and `fail`.
    pub cache_root: PathBuf,
    /// Names a file in the cache.
    pub uri_of: UriFn,
}

impl Env {
    /// The app's local cache folder (`%LOCALAPPDATA%\{identifier}`) plus `thumbnails`.
    pub fn system<R: Runtime>(app: &AppHandle<R>) -> Env {
        let base = app
            .path()
            .app_cache_dir()
            .unwrap_or_else(|_| std::env::temp_dir());
        Env {
            cache_root: base.join("thumbnails"),
            uri_of: Arc::new(file_uri),
        }
    }
}

pub struct Platform {
    store: Arc<Store>,
    processor: Arc<ShellProcessor>,
    status: PluginStatus,
}

impl Platform {
    pub fn new(env: Env, config: &Config, mem: Arc<MemCache>, _limits: Arc<Limits>) -> Self {
        let store = Arc::new(Store::new(
            env.cache_root.clone(),
            &config.app_name,
            &config.app_version,
            env.uri_of,
        ));
        let cache = match fs::create_dir_all(&env.cache_root) {
            Ok(()) => FeatureStatus::available(FEATURE_CACHE),
            Err(error) => FeatureStatus::unavailable(
                FEATURE_CACHE,
                ReasonKind::NoCacheDirectory,
                format!(
                    "the thumbnail cache {} cannot be used: {error}",
                    env.cache_root.display()
                ),
            ),
        };
        let status = PluginStatus::build(
            Flavour::Windows,
            vec![
                cache,
                FeatureStatus::unavailable(
                    FEATURE_BUILTIN,
                    ReasonKind::OtherPlatform,
                    "the shell makes thumbnails on Windows",
                ),
                FeatureStatus::unavailable(
                    FEATURE_EXTERNAL,
                    ReasonKind::OtherPlatform,
                    "*.thumbnailer files are a freedesktop.org convention",
                ),
                FeatureStatus::available(FEATURE_SHELL),
            ],
        );
        Platform {
            processor: Arc::new(ShellProcessor {
                store: Arc::clone(&store),
                mem,
            }),
            store,
            status,
        }
    }

    pub fn store(&self) -> Arc<Store> {
        Arc::clone(&self.store)
    }

    pub fn processor(&self) -> Arc<dyn Processor> {
        Arc::clone(&self.processor) as Arc<dyn Processor>
    }

    pub fn status(&self) -> PluginStatus {
        self.status.clone()
    }
}

/// Enters a single-threaded apartment for the life of a worker thread.
struct ComGuard(bool);

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful `CoInitializeEx` of `on_worker_start`, on the same thread.
            unsafe { CoUninitialize() };
        }
    }
}

pub struct ShellProcessor {
    store: Arc<Store>,
    mem: Arc<MemCache>,
}

impl Processor for ShellProcessor {
    fn on_worker_start(&self) -> Box<dyn Any> {
        // SAFETY: plain COM initialisation of this worker thread; the result says whether a matching `CoUninitialize` is owed.
        let entered = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        Box::new(ComGuard(entered))
    }

    fn process(&self, request: &ThumbRequest, cancel: &AtomicBool) -> Outcome {
        if !winmap::is_local(&request.path) {
            return Outcome::Skipped {
                why: SkipWhy::Remote,
            };
        }
        let (key, uri, secs) = match pipeline::begin(&self.store, &self.mem, request) {
            Start::Done(outcome) => return outcome,
            Start::Make { key, uri, secs } => (key, uri, secs),
        };
        if cancel.load(Ordering::Relaxed) {
            return Outcome::Cancelled;
        }
        let metadata = match fs::metadata(&request.path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => {
                return Outcome::Skipped {
                    why: SkipWhy::NoGenerator,
                }
            }
            Err(error) => {
                return Outcome::Failed {
                    reason: error.to_string(),
                }
            }
        };
        let mode = winmap::shell_mode(metadata.file_attributes());
        let meta = Meta {
            uri,
            mtime_secs: secs,
            file_size: Some(metadata.len()),
        };
        match shell_thumbnail(&request.path, request.size.pixels(), mode) {
            Ok(rendered) => pipeline::finish(&self.store, &self.mem, &key, &meta, &rendered),
            Err((failure, message)) => match winmap::skip_for(mode, failure) {
                Some(why) => Outcome::Skipped { why },
                None => pipeline::fail(&self.store, &key, &meta, message),
            },
        }
    }
}

/// Asks the shell for the file's thumbnail. In `CacheOnly` mode the shell answers only from its cache and never reads the file's content.
fn shell_thumbnail(
    path: &str,
    side: u32,
    mode: ShellMode,
) -> Result<Rendered, (ShellFailure, String)> {
    let fail =
        |error: &windows::core::Error| (winmap::classify_failure(error.code().0), error.message());
    let mut flags: SIIGBF = SIIGBF_THUMBNAILONLY;
    if mode == ShellMode::CacheOnly {
        flags |= SIIGBF_INCACHEONLY;
    }
    // SAFETY: shell calls on an item this function owns; the bitmap the shell returns is ours to delete, and `read_bitmap` deletes it.
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(&HSTRING::from(path), None).map_err(|e| fail(&e))?;
        let bitmap = factory
            .GetImage(
                SIZE {
                    cx: side as i32,
                    cy: side as i32,
                },
                flags,
            )
            .map_err(|e| fail(&e))?;
        let object = HGDIOBJ(bitmap.0);
        let result = read_bitmap(object);
        let _ = DeleteObject(object);
        result.ok_or((
            ShellFailure::Other,
            "the shell returned an unreadable bitmap".to_string(),
        ))
    }
}

/// Reads a bitmap's pixels as top-down 32-bit `BGRA` and converts them.
///
/// # Safety
/// `bitmap` must be a valid GDI bitmap handle.
unsafe fn read_bitmap(bitmap: HGDIOBJ) -> Option<Rendered> {
    let mut info = BITMAP::default();
    if GetObjectW(
        bitmap,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut info as *mut BITMAP as *mut _),
    ) == 0
    {
        return None;
    }
    let (width, height) = (
        u32::try_from(info.bmWidth).ok()?,
        u32::try_from(info.bmHeight).ok()?,
    );
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: info.bmWidth,
            // A negative height asks for rows from the top.
            biHeight: -info.bmHeight,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0, // BI_RGB
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![
        0u8;
        (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?
    ];
    let dc = CreateCompatibleDC(None);
    if dc.is_invalid() {
        return None;
    }
    let copied = GetDIBits(
        dc,
        windows::Win32::Graphics::Gdi::HBITMAP(bitmap.0),
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut header,
        DIB_RGB_COLORS,
    );
    let _ = DeleteDC(dc);
    if copied == 0 {
        return None;
    }
    winmap::bitmap_to_rendered(width, height, &pixels)
}

/// Runs the shell for one file on the calling thread, which must be in a COM apartment. Used by the ignored live test.
pub fn live_thumbnail(path: &str, side: u32) -> Result<Rendered, String> {
    shell_thumbnail(path, side, ShellMode::Make).map_err(|(_, message)| message)
}

#[cfg(test)]
mod tests;

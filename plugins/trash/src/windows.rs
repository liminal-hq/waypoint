// The Windows backend: IFileOperation to trash, and the Recycle Bin shell folder to list, restore, delete and empty
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Everything that talks to the shell runs on the main thread (see `main_thread`) with COM initialised, as `IFileOperation` and the shell folders require a single-threaded apartment. The Recycle Bin is read through `IShellItem2` properties: the original location (`System.Recycle.DeletedFrom`) and the deletion time (`System.Recycle.DateDeleted`).
//!
//! An item's id is the path of its file in the bin (`C:\$Recycle.Bin\S-1-5-21-…\$R1A2B3C.txt`). It is unique, survives restarts, and is what the shell itself names the item by.
//!
//! Restore moves the item out of the bin with `IFileOperation::MoveItem`, after checking the destination here: the shell's own "undelete" verb answers conflicts with dialogs, and this plugin must report `OriginExists` and `OriginMissingParent` instead and never overwrite.
//!

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Runtime};
use windows::core::{Interface, GUID, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{FILETIME, PROPERTYKEY};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_RecycleBinFolder, FileOperation, IEnumShellItems, IFileOperation,
    IShellItem, IShellItem2, SHCreateItemFromParsingName, SHEmptyRecycleBinW, SHGetKnownFolderItem,
    FOFX_EARLYFAILURE, FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NOERRORUI, FOF_NO_UI, FOF_SILENT,
    KF_FLAG_DEFAULT, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SIGDN,
    SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY,
};

use crate::error::TrashError;
use crate::main_thread;
use crate::models::{
    EmptyFailure, EmptyReport, FeatureStatus, Flavour, PluginStatus, RestoreTarget, TrashReceipt,
    TrashedItem, FEATURE_EMPTY, FEATURE_EXPIRY, FEATURE_LIST, FEATURE_PER_VOLUME, FEATURE_RESTORE,
    FEATURE_TRASH,
};
use crate::winmap;

/// `System.Recycle.DeletedFrom` and `System.Recycle.DateDeleted`: the Recycle Bin's own property set.
const RECYCLE_FMTID: GUID = GUID::from_u128(0x9b174b33_40ff_11d2_a27e_00c04fc30871);
const PKEY_DELETED_FROM: PROPERTYKEY = PROPERTYKEY {
    fmtid: RECYCLE_FMTID,
    pid: 2,
};
const PKEY_DATE_DELETED: PROPERTYKEY = PROPERTYKEY {
    fmtid: RECYCLE_FMTID,
    pid: 3,
};

/// Initialises COM for the calling thread, apartment-threaded, and undoes it when dropped. The main thread normally has COM already (the webview needs it), which is fine; a different threading model is also tolerated, so the guard never fails.
struct ComGuard(bool);

impl ComGuard {
    fn new() -> Self {
        // SAFETY: plain COM initialisation; the result says whether a matching `CoUninitialize` is owed.
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        ComGuard(result.is_ok())
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful `CoInitializeEx` of `new`.
            unsafe { CoUninitialize() };
        }
    }
}

fn error_of(error: &windows::core::Error) -> TrashError {
    let message = error.message();
    match winmap::win32_code_of_hresult(error.code().0) {
        Some(code) => winmap::error_from_win32(code, &message),
        None => TrashError::Io { message },
    }
}

/// Reads a `PWSTR` the shell allocated, and frees it.
fn take_string(text: PWSTR) -> String {
    // SAFETY: the shell returns a NUL-terminated string allocated with `CoTaskMemAlloc`, which is freed once, after copying.
    unsafe {
        let value = text.to_string().unwrap_or_default();
        CoTaskMemFree(Some(text.0 as *const _));
        value
    }
}

fn display_name(item: &IShellItem, kind: SIGDN) -> Option<String> {
    // SAFETY: `GetDisplayName` returns a string for `take_string`.
    unsafe { item.GetDisplayName(kind) }.ok().map(take_string)
}

fn filetime_ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

/// One item of the Recycle Bin, with the shell item to act on it.
struct BinItem {
    shell: IShellItem,
    item: TrashedItem,
}

/// What a bin item is, from one `stat`: whether it is a folder, and its size when it is a file. A folder's size takes a walk of its whole tree, which must not happen on the main thread, so it is left 0 here and filled in by `fill_folder_sizes`.
fn item_kind(path: &Path) -> (u64, bool) {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => (0, true),
        Ok(metadata) => (metadata.len(), false),
        Err(_) => (0, false),
    }
}

/// The total size of the files under `path`, not following links.
fn tree_size(path: &Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => std::fs::read_dir(path)
            .map(|read| read.flatten().map(|entry| tree_size(&entry.path())).sum())
            .unwrap_or(0),
        Ok(metadata) => metadata.len(),
        Err(_) => 0,
    }
}

/// Fills in the size of every folder in `items` by walking it. It touches no COM object, so it runs on any thread, and `Platform::list` runs it on a blocking one instead of the main thread.
pub fn fill_folder_sizes(items: &mut [TrashedItem]) {
    for item in items.iter_mut().filter(|item| item.is_dir) {
        item.size = tree_size(Path::new(&item.receipt.trash_id));
    }
}

/// Every item in every drive's Recycle Bin.
fn bin_items() -> Result<Vec<BinItem>, TrashError> {
    // SAFETY: COM calls on interfaces this function owns, on a thread with COM initialised; every string is freed by `take_string`.
    unsafe {
        let folder: IShellItem =
            SHGetKnownFolderItem(&FOLDERID_RecycleBinFolder, KF_FLAG_DEFAULT, None)
                .map_err(|error| error_of(&error))?;
        let children: IEnumShellItems = folder
            .BindToHandler(None, &BHID_EnumItems)
            .map_err(|error| error_of(&error))?;
        let mut items = Vec::new();
        loop {
            let mut batch: [Option<IShellItem>; 16] = Default::default();
            let mut fetched = 0u32;
            children
                .Next(&mut batch, Some(&mut fetched))
                .map_err(|error| error_of(&error))?;
            if fetched == 0 {
                break;
            }
            for shell in batch.into_iter().take(fetched as usize).flatten() {
                if let Some(item) = read_item(&shell) {
                    items.push(BinItem { shell, item });
                }
            }
        }
        Ok(items)
    }
}

fn read_item(shell: &IShellItem) -> Option<TrashedItem> {
    let details: IShellItem2 = shell.cast().ok()?;
    // The item's file in the bin; without it the item has no stable id and is skipped.
    let bin_path = display_name(shell, SIGDN_FILESYSPATH)?;
    let display = display_name(shell, SIGDN_NORMALDISPLAY).unwrap_or_default();
    let name = winmap::original_name(&display, &bin_path);
    // SAFETY: property reads on a live item; the string is freed by `take_string`.
    let (location, deleted) = unsafe {
        let location = details
            .GetString(&PKEY_DELETED_FROM)
            .map(take_string)
            .unwrap_or_default();
        let deleted = details
            .GetFileTime(&PKEY_DATE_DELETED)
            .map(filetime_ticks)
            .map(winmap::filetime_to_unix)
            .unwrap_or(0);
        (location, deleted)
    };
    let original = winmap::join_original(&location, &name);
    let (size, is_dir) = item_kind(Path::new(&bin_path));
    Some(TrashedItem {
        receipt: TrashReceipt {
            trash_id: bin_path,
            original_path: PathBuf::from(&original),
            deleted_at: deleted,
        },
        name,
        original_path: PathBuf::from(original),
        deleted_at: deleted,
        size,
        is_dir,
    })
}

fn new_operation(
    flags: windows::Win32::UI::Shell::FILEOPERATION_FLAGS,
) -> Result<IFileOperation, TrashError> {
    // SAFETY: creates the shell's file operation object on a thread with COM initialised.
    unsafe {
        let operation: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|error| error_of(&error))?;
        operation
            .SetOperationFlags(flags)
            .map_err(|error| error_of(&error))?;
        Ok(operation)
    }
}

/// Runs the queued operations. An error `HRESULT` and an "aborted" result both fail the batch; the caller then looks at what actually happened to each item.
fn perform(operation: &IFileOperation) -> Result<(), TrashError> {
    // SAFETY: `PerformOperations` runs the queued operations synchronously on this thread.
    unsafe {
        operation
            .PerformOperations()
            .map_err(|error| error_of(&error))?;
        if operation
            .GetAnyOperationsAborted()
            .is_ok_and(|aborted| aborted.as_bool())
        {
            return Err(TrashError::io("the operation was cancelled"));
        }
    }
    Ok(())
}

/// A folder with its links and short names resolved, as the file system spells it.
fn resolve_folder(folder: &str) -> String {
    let cleaned = winmap::clean(folder);
    std::fs::canonicalize(&cleaned).map_or(cleaned, |real| {
        winmap::strip_verbatim(&real.to_string_lossy())
    })
}

/// The path to trash, resolved the way the Linux side does: `.` and `..` are resolved first, then the folders above the last part are resolved through links, but never the last part, so a link is trashed itself. The protected-path check then sees where the path really leads.
fn resolve_target(path: &Path) -> String {
    let cleaned = winmap::clean(&path.to_string_lossy());
    match winmap::split_parent(&cleaned) {
        Some((parent, name)) => winmap::join_original(&resolve_folder(&parent), &name),
        None => cleaned,
    }
}

fn exists(path: &str) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

// ------------------------------------------------------------------ operations

/// Moves the paths to the Recycle Bin in one operation. Each path gets its own outcome: what is still there afterwards failed, and what is gone has its receipt looked up in the bin.
pub fn trash_blocking(paths: &[PathBuf]) -> Vec<Result<TrashReceipt, TrashError>> {
    let _com = ComGuard::new();
    let profile = std::env::var("USERPROFILE")
        .ok()
        .map(|profile| resolve_folder(&profile));
    let mut results: Vec<Option<Result<TrashReceipt, TrashError>>> = vec![None; paths.len()];
    let mut queued: Vec<(usize, String, IShellItem)> = Vec::new();
    let operation = match new_operation(
        FOF_ALLOWUNDO | FOFX_RECYCLEONDELETE | FOF_SILENT | FOF_NOERRORUI | FOFX_EARLYFAILURE,
    ) {
        Ok(operation) => operation,
        Err(error) => return paths.iter().map(|_| Err(error.clone())).collect(),
    };

    for (index, path) in paths.iter().enumerate() {
        let text = resolve_target(path);
        if !path.is_absolute() {
            results[index] = Some(Err(TrashError::io("the path must be absolute")));
        } else if let Some(reason) = winmap::refusal(&text, profile.as_deref()) {
            results[index] = Some(Err(TrashError::Refused {
                reason: reason.to_string(),
            }));
        } else if !exists(&text) {
            results[index] = Some(Err(TrashError::NotFound));
        } else {
            // SAFETY: builds a shell item for an existing path and queues its deletion.
            let queued_item = unsafe {
                SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(text.as_str()), None)
                    .and_then(|item| operation.DeleteItem(&item, None).map(|()| item))
            };
            match queued_item {
                Ok(item) => queued.push((index, text, item)),
                Err(error) => results[index] = Some(Err(error_of(&error))),
            }
        }
    }

    let outcome = if queued.is_empty() {
        Ok(())
    } else {
        perform(&operation)
    };
    if !queued.is_empty() {
        let bin = bin_items().unwrap_or_default();
        for (index, text, _) in queued {
            results[index] = Some(if exists(&text) {
                // Still there: the batch failed before it, or the shell skipped it.
                Err(outcome
                    .clone()
                    .err()
                    .unwrap_or_else(|| TrashError::io("the Recycle Bin did not take the item")))
            } else {
                // Gone: find it in the bin, the newest match if the path was trashed before too.
                bin.iter()
                    .filter(|entry| {
                        winmap::same_path(&entry.item.original_path.to_string_lossy(), &text)
                    })
                    .max_by_key(|entry| entry.item.deleted_at)
                    .map(|entry| entry.item.receipt.clone())
                    .ok_or_else(|| {
                        TrashError::io("the item left its place but is not in the Recycle Bin")
                    })
            });
        }
    }
    results
        .into_iter()
        .map(|result| result.unwrap_or(Err(TrashError::io("the path was not processed"))))
        .collect()
}

/// The items of the bin, oldest first, with folder sizes still 0 (see `fill_folder_sizes`). This is the part that needs COM.
fn list_unsized_blocking() -> Result<Vec<TrashedItem>, TrashError> {
    let _com = ComGuard::new();
    let mut items: Vec<TrashedItem> = bin_items()?.into_iter().map(|entry| entry.item).collect();
    items.sort_by(|a, b| {
        a.deleted_at
            .cmp(&b.deleted_at)
            .then_with(|| a.receipt.trash_id.cmp(&b.receipt.trash_id))
    });
    Ok(items)
}

/// The items of the bin with every size filled in, on the calling thread (what the live test uses).
#[cfg(test)]
pub fn list_blocking() -> Result<Vec<TrashedItem>, TrashError> {
    let mut items = list_unsized_blocking()?;
    fill_folder_sizes(&mut items);
    Ok(items)
}

fn find(items: Vec<BinItem>, trash_id: &str) -> Result<BinItem, TrashError> {
    items
        .into_iter()
        .find(|entry| winmap::same_path(&entry.item.receipt.trash_id, trash_id))
        .ok_or(TrashError::NotFound)
}

pub fn restore_blocking(
    receipt: &TrashReceipt,
    target: &RestoreTarget,
) -> Result<TrashReceipt, TrashError> {
    let _com = ComGuard::new();
    let entry = find(bin_items()?, &receipt.trash_id)?;
    let destination = match target {
        RestoreTarget::Original => entry.item.original_path.clone(),
        RestoreTarget::Path { path } => path.clone(),
    };
    let text = winmap::normalise(&destination.to_string_lossy());
    if !destination.is_absolute() {
        return Err(TrashError::io("the destination must be an absolute path"));
    }
    let (parent, name) = winmap::split_parent(&text)
        .ok_or_else(|| TrashError::io("the destination has no folder"))?;
    if exists(&text) {
        return Err(TrashError::OriginExists { path: destination });
    }
    if !Path::new(&parent).is_dir() {
        return Err(TrashError::OriginMissingParent {
            path: PathBuf::from(parent),
        });
    }
    // SAFETY: shell calls on items this function owns.
    unsafe {
        let folder: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(parent.as_str()), None)
            .map_err(|error| error_of(&error))?;
        // Silent, with no "yes to all": a collision that appeared since the check above must ask, never overwrite.
        let operation = new_operation(FOF_SILENT | FOF_NOERRORUI | FOFX_EARLYFAILURE)?;
        operation
            .MoveItem(&entry.shell, &folder, &HSTRING::from(name.as_str()), None)
            .map_err(|error| error_of(&error))?;
        perform(&operation)?;
    }
    if !exists(&text) {
        return Err(TrashError::io("the item was not restored"));
    }
    Ok(TrashReceipt {
        trash_id: receipt.trash_id.clone(),
        original_path: destination,
        deleted_at: entry.item.deleted_at,
    })
}

/// Deletes the given bin items for good in one operation and reports which could not be removed.
fn delete_items(entries: &[BinItem]) -> Vec<Result<(), TrashError>> {
    let mut results: Vec<Result<(), TrashError>> = Vec::with_capacity(entries.len());
    let operation = match new_operation(FOF_NO_UI) {
        Ok(operation) => operation,
        Err(error) => return entries.iter().map(|_| Err(error.clone())).collect(),
    };
    let mut queued = false;
    for entry in entries {
        // SAFETY: queues the deletion of a live bin item.
        let queue = unsafe { operation.DeleteItem(&entry.shell, None) };
        results.push(queue.map_err(|error| error_of(&error)));
        queued |= results.last().is_some_and(Result::is_ok);
    }
    let outcome = if queued { perform(&operation) } else { Ok(()) };
    for (entry, result) in entries.iter().zip(results.iter_mut()) {
        if result.is_ok() && exists(&entry.item.receipt.trash_id) {
            *result = Err(outcome
                .clone()
                .err()
                .unwrap_or_else(|| TrashError::io("the item could not be removed")));
        }
    }
    results
}

pub fn delete_blocking(receipt: &TrashReceipt) -> Result<(), TrashError> {
    let _com = ComGuard::new();
    let entry = find(bin_items()?, &receipt.trash_id)?;
    delete_items(std::slice::from_ref(&entry)).remove(0)
}

pub fn empty_blocking(older_than_days: Option<u32>, now: i64) -> Result<EmptyReport, TrashError> {
    let _com = ComGuard::new();
    let items = bin_items()?;
    let mut report = EmptyReport::default();
    match older_than_days {
        None => {
            if items.is_empty() {
                return Ok(report);
            }
            // SAFETY: the documented call for emptying every bin, without prompts, progress or sound.
            let result = unsafe {
                SHEmptyRecycleBinW(
                    None,
                    PCWSTR::null(),
                    SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
                )
            };
            if let Err(error) = result {
                // An error code for "already empty" is not a failure.
                if !bin_items()?.is_empty() {
                    return Err(error_of(&error));
                }
            }
            let left = bin_items()?;
            for entry in &items {
                let still_there = left.iter().any(|other| {
                    winmap::same_path(&other.item.receipt.trash_id, &entry.item.receipt.trash_id)
                });
                if still_there {
                    report.failed.push(EmptyFailure {
                        trash_id: entry.item.receipt.trash_id.clone(),
                        error: TrashError::io("the item could not be removed"),
                    });
                } else {
                    report.removed += 1;
                }
            }
        }
        Some(days) => {
            let old: Vec<BinItem> = items
                .into_iter()
                .filter(|entry| winmap::is_expired(entry.item.deleted_at, now, days))
                .collect();
            for (entry, result) in old.iter().zip(delete_items(&old)) {
                match result {
                    Ok(()) => report.removed += 1,
                    Err(error) => report.failed.push(EmptyFailure {
                        trash_id: entry.item.receipt.trash_id.clone(),
                        error,
                    }),
                }
            }
        }
    }
    Ok(report)
}

// -------------------------------------------------------------------- platform

pub struct Platform;

fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

async fn on_main_thread<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, TrashError> {
    main_thread::run(app, job)
        .await
        .ok_or_else(|| TrashError::io("the application's event loop is not running"))
}

impl Platform {
    pub fn new() -> Self {
        Platform
    }

    pub fn status(&self) -> PluginStatus {
        PluginStatus::build(
            Flavour::Windows,
            [
                FEATURE_TRASH,
                FEATURE_LIST,
                FEATURE_RESTORE,
                FEATURE_EMPTY,
                FEATURE_EXPIRY,
                FEATURE_PER_VOLUME,
            ]
            .into_iter()
            .map(FeatureStatus::available)
            .collect(),
        )
    }

    pub async fn trash<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        paths: Vec<PathBuf>,
    ) -> Vec<Result<TrashReceipt, TrashError>> {
        let count = paths.len();
        match on_main_thread(app, move || trash_blocking(&paths)).await {
            Ok(results) => results,
            Err(error) => (0..count).map(|_| Err(error.clone())).collect(),
        }
    }

    pub async fn list<R: Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<Vec<TrashedItem>, TrashError> {
        // Only the COM calls run on the main thread; walking the folders for their sizes does not.
        let mut items = on_main_thread(app, list_unsized_blocking).await??;
        tauri::async_runtime::spawn_blocking(move || {
            fill_folder_sizes(&mut items);
            items
        })
        .await
        .map_err(|error| TrashError::io(format!("the size task failed: {error}")))
    }

    pub async fn restore<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        receipt: &TrashReceipt,
        target: RestoreTarget,
    ) -> Result<TrashReceipt, TrashError> {
        let receipt = receipt.clone();
        on_main_thread(app, move || restore_blocking(&receipt, &target)).await?
    }

    pub async fn delete<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        receipt: &TrashReceipt,
    ) -> Result<(), TrashError> {
        let receipt = receipt.clone();
        on_main_thread(app, move || delete_blocking(&receipt)).await?
    }

    pub async fn empty<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        older_than_days: Option<u32>,
    ) -> Result<EmptyReport, TrashError> {
        on_main_thread(app, move || empty_blocking(older_than_days, unix_now())).await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("trash-live-{stamp}-{name}"))
    }

    /// Drives the real Recycle Bin of the user running the test: trash, list, restore, conflict, delete and the batch outcomes. Set `TRASH_LIVE_EMPTY=1` to also empty the whole bin at the end, which removes the user's own items: only do that on a throwaway machine.
    #[test]
    #[ignore = "uses the real Recycle Bin; run on Windows with `cargo test -p tauri-plugin-trash live_recycle_bin -- --ignored --nocapture`"]
    fn live_recycle_bin() {
        let file = unique("a file.txt");
        let folder = unique("a folder");
        let missing = unique("missing.txt");
        std::fs::write(&file, "live content").unwrap();
        std::fs::create_dir_all(folder.join("inner")).unwrap();
        std::fs::write(folder.join("inner").join("f.bin"), [1u8; 2048]).unwrap();

        // A batch: two real paths, one that is not there, one that is refused.
        let results =
            trash_blocking(&[file.clone(), folder.clone(), missing, PathBuf::from("C:\\")]);
        println!("batch results: {results:#?}");
        assert!(results[0].is_ok() && results[1].is_ok());
        assert_eq!(results[2], Err(TrashError::NotFound));
        assert!(matches!(results[3], Err(TrashError::Refused { .. })));
        assert!(!file.exists() && !folder.exists());
        let file_receipt = results[0].clone().unwrap();
        let folder_receipt = results[1].clone().unwrap();
        println!("receipts: {file_receipt:?} {folder_receipt:?}");
        assert!(file_receipt.trash_id.contains("$Recycle.Bin"));
        assert!(winmap::same_path(
            &file_receipt.original_path.to_string_lossy(),
            &file.to_string_lossy()
        ));
        assert!(
            (file_receipt.deleted_at - unix_now()).abs() < 120,
            "deleted just now"
        );

        let listed = list_blocking().unwrap();
        println!("the bin holds {} items", listed.len());
        let listed_file = listed
            .iter()
            .find(|i| i.receipt.trash_id == file_receipt.trash_id)
            .unwrap();
        assert_eq!(listed_file.size, 12);
        assert!(!listed_file.is_dir);
        assert_eq!(
            listed_file.name,
            file.file_name().unwrap().to_string_lossy()
        );
        let listed_folder = listed
            .iter()
            .find(|i| i.receipt.trash_id == folder_receipt.trash_id)
            .unwrap();
        assert!(listed_folder.is_dir);
        assert_eq!(listed_folder.size, 2048);

        // Restore to the original place, and the conflict and missing-folder answers.
        let restored = restore_blocking(&file_receipt, &RestoreTarget::Original).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "live content");
        assert!(winmap::same_path(
            &restored.original_path.to_string_lossy(),
            &file.to_string_lossy()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        assert!(matches!(
            restore_blocking(&folder_receipt, &RestoreTarget::Original),
            Err(TrashError::OriginExists { .. })
        ));
        std::fs::remove_dir(&folder).unwrap();
        let elsewhere = unique("elsewhere");
        let missing_parent = RestoreTarget::Path {
            path: elsewhere.join("deeper").join("x"),
        };
        assert!(matches!(
            restore_blocking(&folder_receipt, &missing_parent),
            Err(TrashError::OriginMissingParent { .. })
        ));
        let again = restore_blocking(&folder_receipt, &RestoreTarget::Original).unwrap();
        println!("restored folder: {again:?}");
        assert_eq!(
            std::fs::read(folder.join("inner").join("f.bin"))
                .unwrap()
                .len(),
            2048
        );

        // Trash again, then delete for good; and a receipt that is gone.
        let second = trash_blocking(&[file.clone(), folder.clone()]);
        let second_file = second[0].clone().unwrap();
        delete_blocking(&second_file).unwrap();
        assert_eq!(delete_blocking(&second_file), Err(TrashError::NotFound));
        assert!(!list_blocking()
            .unwrap()
            .iter()
            .any(|i| i.receipt.trash_id == second_file.trash_id));
        let second_folder = second[1].clone().unwrap();

        // The expiry sweep leaves a fresh item alone and removes one at zero days.
        let report = empty_blocking(Some(36_500), unix_now()).unwrap();
        assert!(report.failed.is_empty());
        assert!(list_blocking()
            .unwrap()
            .iter()
            .any(|i| i.receipt.trash_id == second_folder.trash_id));
        let report = empty_blocking(Some(0), unix_now() + 60).unwrap();
        println!("sweep: {report:?}");
        assert!(report.removed >= 1);
        assert!(!list_blocking()
            .unwrap()
            .iter()
            .any(|i| i.receipt.trash_id == second_folder.trash_id));

        if std::env::var("TRASH_LIVE_EMPTY").as_deref() == Ok("1") {
            let leftover = unique("leftover.txt");
            std::fs::write(&leftover, "x").unwrap();
            trash_blocking(&[leftover]);
            let report = empty_blocking(None, unix_now()).unwrap();
            println!("empty: {report:?}");
            assert!(report.failed.is_empty());
            assert!(list_blocking().unwrap().is_empty());
        }
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir_all(&folder);
    }
}

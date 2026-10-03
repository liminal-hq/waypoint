// Exporting and importing the configuration files: the registry, the file picker seam and the files
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The bundle format and the planning are in `waypoint-settings`; this holds what needs the app:
// the configuration files that can be exported (the settings document here, any other the app
// registers, as the operations settings are), the native file picker the app supplies, and the
// plan that was just made. The picker and the paths stay in Rust: the page asks for "export" or
// "import" and hears the result, so it can neither name a file to write nor hand back a document
// to apply.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime};
use waypoint_settings::{
    apply_import, export_bundle, plan_import, plan_settings, BundleError, BundleKind, ConfigFile,
    ExportFile, ExportMeta, ExportReceipt, ExportedBundle, FilePlan, ImportPreview,
    SettingsDocument, MAX_TOTAL_BYTES, SETTINGS_FILE_ID,
};

use crate::error::Error;
use crate::state::SettingsStore;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The native dialogs the app shows for choosing a file. The window is the one the dialog belongs
/// to (it stays on top of it and blocks it); `None` is a dialog the person closed.
pub trait FilePicker: Send + Sync {
    /// Asks where to save, offering `suggested_name`.
    fn save(&self, window: &str, suggested_name: &str, kind: BundleKind) -> Option<PathBuf>;
    /// Asks which file to read.
    fn open(&self, window: &str) -> Option<PathBuf>;
}

struct Pending {
    id: u64,
    bytes: Vec<u8>,
}

/// The configuration files, the picker and the plan that was just made.
#[derive(Default)]
pub struct Transfers {
    files: Mutex<Vec<Arc<dyn ConfigFile>>>,
    picker: Mutex<Option<Arc<dyn FilePicker>>>,
    pending: Mutex<Option<Pending>>,
    next_plan: AtomicU64,
}

impl Transfers {
    /// Adds a configuration file to what is exported and imported. A file with the id of one that
    /// is there replaces it.
    pub fn register(&self, file: Arc<dyn ConfigFile>) {
        let mut files = locked(&self.files);
        files.retain(|existing| existing.id() != file.id());
        files.push(file);
    }

    /// Takes the configuration file with `id` out of what is exported and imported.
    pub fn unregister(&self, id: &str) {
        locked(&self.files).retain(|existing| existing.id() != id);
    }

    /// The native dialogs. Without one, export and import report that they are unavailable.
    pub fn set_picker(&self, picker: Arc<dyn FilePicker>) {
        *locked(&self.picker) = Some(picker);
    }

    pub fn picker(&self) -> Option<Arc<dyn FilePicker>> {
        locked(&self.picker).clone()
    }

    fn files(&self) -> Vec<Arc<dyn ConfigFile>> {
        locked(&self.files).clone()
    }

    /// Every file's document, packed as the bundle it is exported as.
    pub fn export(
        &self,
        app_version: &str,
        local_offset_minutes: i32,
    ) -> Result<ExportedBundle, Error> {
        let mut files = Vec::new();
        for file in self.files() {
            let document = file
                .export()
                .map_err(|e| Error::Io(format!("could not read `{}`: {e}", file.id())))?;
            files.push(ExportFile {
                id: file.id().to_owned(),
                document,
            });
        }
        let exported_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Ok(export_bundle(
            &files,
            &ExportMeta {
                app_version: app_version.to_owned(),
                exported_at_unix,
                local_offset_minutes,
            },
        )?)
    }

    /// Writes `bundle` to `path`, giving the path the bundle's extension when it has none.
    pub fn write(&self, path: &Path, bundle: &ExportedBundle) -> Result<ExportReceipt, Error> {
        let mut path = path.to_path_buf();
        if path.extension().is_none() {
            path.set_extension(bundle.kind.extension());
        }
        write_atomically(&path, &bundle.bytes)?;
        Ok(ExportReceipt {
            path: path.to_string_lossy().into_owned(),
            kind: bundle.kind,
            files: self.files().iter().map(|f| f.id().to_owned()).collect(),
        })
    }

    /// Plans importing `bytes` and keeps them as the file `apply` will use. A new plan replaces
    /// the one before.
    pub fn plan(&self, bytes: Vec<u8>) -> Result<ImportPreview, Error> {
        let planned = plan_import(&bytes, &self.files())?;
        let plan_id = self.next_plan.fetch_add(1, Ordering::AcqRel) + 1;
        *locked(&self.pending) = Some(Pending { id: plan_id, bytes });
        Ok(ImportPreview {
            plan_id,
            plan: planned.plan,
        })
    }

    /// Plans importing the file at `path`.
    pub fn plan_path(&self, path: &Path) -> Result<ImportPreview, Error> {
        self.plan(read_bounded(path)?)
    }

    /// Applies the plan `plan_id` names. The file is read again against what is in force now, so
    /// what is applied is what that file holds, checked again, and never anything the page sent.
    /// The plan is spent whether it works or not; a number that is not the plan's leaves it be.
    pub fn apply(&self, plan_id: u64) -> Result<(), Error> {
        let pending = {
            let mut slot = locked(&self.pending);
            if slot.as_ref().map(|p| p.id) != Some(plan_id) {
                return Err(Error::Stale);
            }
            slot.take()
        };
        let Some(pending) = pending else {
            return Err(Error::Stale);
        };
        let files = self.files();
        let planned = plan_import(&pending.bytes, &files)?;
        apply_import(&planned, &files)?;
        Ok(())
    }
}

/// Reads a file of at most `MAX_TOTAL_BYTES`: a larger one, or something that is not a file, is
/// refused before it is read.
fn read_bounded(path: &Path) -> Result<Vec<u8>, Error> {
    let io = |e: std::io::Error| Error::Io(format!("could not read the file: {e}"));
    // Looked at before it is opened: Windows refuses to open a folder at all.
    let meta = std::fs::metadata(path).map_err(io)?;
    if !meta.is_file() {
        return Err(BundleError::NotABundle.into());
    }
    let file = std::fs::File::open(path).map_err(io)?;
    if meta.len() > MAX_TOTAL_BYTES as u64 {
        return Err(BundleError::TooLarge {
            what: "the file".to_owned(),
            limit: MAX_TOTAL_BYTES,
        }
        .into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_TOTAL_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    Ok(bytes)
}

/// Writes through a temporary file beside `path` and renames it over, so a failure part way
/// leaves what was there.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let io = |e: std::io::Error| Error::Io(format!("could not write the file: {e}"));
    let name = path
        .file_name()
        .ok_or_else(|| Error::Io("that is not a file name".to_owned()))?;
    let mut temporary = name.to_os_string();
    temporary.push(format!(".{}.tmp", std::process::id()));
    let temporary = path.with_file_name(temporary);
    let written = std::fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(e) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(io(e));
    }
    std::fs::rename(&temporary, path).map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        io(e)
    })
}

/// The settings document as a configuration file, over the store that is its one writer.
pub struct SettingsConfigFile<R: Runtime>(pub AppHandle<R>);

impl<R: Runtime> SettingsConfigFile<R> {
    fn store(&self) -> tauri::State<'_, SettingsStore<R>> {
        self.0.state::<SettingsStore<R>>()
    }
}

impl<R: Runtime> ConfigFile for SettingsConfigFile<R> {
    fn id(&self) -> &str {
        SETTINGS_FILE_ID
    }

    fn export(&self) -> Result<Value, String> {
        serde_json::to_value(SettingsDocument::new(self.store().get())).map_err(|e| e.to_string())
    }

    fn plan(&self, incoming: &Value) -> Result<FilePlan, BundleError> {
        plan_settings(&self.store().get(), incoming)
    }

    fn apply(&self, document: &Value) -> Result<(), String> {
        let document: SettingsDocument =
            serde_json::from_value(document.clone()).map_err(|e| e.to_string())?;
        self.store()
            .import(document.body)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn restore(&self, document: &Value) -> Result<(), String> {
        let document: SettingsDocument =
            serde_json::from_value(document.clone()).map_err(|e| e.to_string())?;
        self.store()
            .set(document.body)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

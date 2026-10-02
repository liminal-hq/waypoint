// The Windows backend: the shell's association handlers to list and open, `SHOpenWithDialog` to choose, and `AssocQueryStringW` to read the default
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shell's COM objects want a single-threaded apartment, and a call here can wait on a dialog, so each one runs on a thread of its own with COM initialised for just that call. Handlers belong to an extension (`.png`), so a type is the extension's registered content type, or the extension itself when it has none.
//!
//! Windows does not let an application change the default silently: `set_default` is unavailable, and `open_default_apps_settings` opens the Settings page where the person does it. A handler's id is the name the shell gives it (`IAssocHandler::GetName`).

use std::path::PathBuf;

use tauri::{AppHandle, Runtime};
use windows::core::{HRESULT, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, IBindCtx, IDataObject, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    AssocQueryStringW, IAssocHandler, SHAssocEnumHandlers, SHCreateDataObject, SHOpenWithDialog,
    SHParseDisplayName, ShellExecuteW, ASSOCF_INIT_IGNOREUNKNOWN, ASSOCSTR, ASSOCSTR_CONTENTTYPE,
    ASSOCSTR_EXECUTABLE, ASSOCSTR_FRIENDLYAPPNAME, ASSOCSTR_FRIENDLYDOCNAME, ASSOC_FILTER,
    ASSOC_FILTER_NONE, ASSOC_FILTER_RECOMMENDED, OAIF_ALLOW_REGISTRATION, OAIF_EXEC, OPENASINFO,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::assoc::{self, RawDefault, RawHandler};
use crate::backend::{Backend, ParentWindow};
use crate::error::{MimeAppsError, Result};
use crate::models::{
    FeatureStatus, Flavour, Handlers, PluginStatus, Reason, TypeInfo, DIRECTORY_TYPE,
    FEATURE_APP_ICONS, FEATURE_CHOOSER, FEATURE_HANDLERS, FEATURE_OPEN_DEFAULT, FEATURE_OPEN_WITH,
    FEATURE_SET_DEFAULT, FEATURE_TYPE_INFO,
};
use crate::target::Target;

/// `HRESULT_FROM_WIN32(ERROR_CANCELLED)`: what `SHOpenWithDialog` returns when the person closes it.
const ERROR_CANCELLED: u32 = 1223;

const SETTINGS_PAGE: &str = "ms-settings:defaultapps";

pub struct Platform;

impl Default for Platform {
    fn default() -> Self {
        Self::new()
    }
}

impl Platform {
    pub fn new() -> Self {
        Platform
    }

    pub fn for_app<R: Runtime>(_app: &AppHandle<R>) -> Self {
        Platform
    }
}

/// Initialises COM for the calling thread, apartment-threaded, and undoes it when dropped.
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

/// Runs `job` on a new thread with COM initialised, and returns its result.
fn on_sta<T: Send + 'static>(job: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    std::thread::Builder::new()
        .name("mime-apps-sta".into())
        .spawn(move || {
            let _com = ComGuard::new();
            job()
        })
        .map_err(|error| MimeAppsError::failed(error.to_string()))?
        .join()
        .map_err(|_| MimeAppsError::failed("the shell call panicked"))?
}

fn failed(error: &windows::core::Error) -> MimeAppsError {
    if error.code() == HRESULT::from_win32(ERROR_CANCELLED) {
        MimeAppsError::Cancelled
    } else {
        MimeAppsError::failed(error.message())
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

/// One string of the association of an extension or a name (`.png`, `Directory`), when the registry has it.
fn assoc_string(kind: ASSOCSTR, association: &str) -> Option<String> {
    let association = HSTRING::from(association);
    let mut buffer = vec![0u16; 1024];
    let mut length = buffer.len() as u32;
    // SAFETY: the buffer outlives the call and `length` is its size in characters.
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_INIT_IGNOREUNKNOWN,
            kind,
            &association,
            PCWSTR::null(),
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
    };
    if result.is_err() {
        return None;
    }
    // The length counts the terminating NUL.
    let text = String::from_utf16_lossy(&buffer[..(length as usize).saturating_sub(1)]);
    (!text.is_empty()).then_some(text)
}

/// The extension with its dot (`.png`), or `None` for a name without one.
fn extension_of(target: &Target) -> Option<String> {
    target.extension().map(|extension| format!(".{extension}"))
}

fn enumerate(extension: &str, filter: ASSOC_FILTER) -> Result<Vec<IAssocHandler>> {
    // SAFETY: plain shell calls on an apartment-threaded thread.
    let handlers = unsafe { SHAssocEnumHandlers(&HSTRING::from(extension), filter) }
        .map_err(|error| failed(&error))?;
    let mut found = Vec::new();
    loop {
        let mut slot = [None];
        let mut fetched = 0u32;
        // SAFETY: `slot` and `fetched` are valid for the call.
        let result = unsafe { handlers.Next(&mut slot, Some(&mut fetched)) };
        if result.is_err() || fetched == 0 {
            break;
        }
        found.extend(slot[0].take());
    }
    Ok(found)
}

fn describe(handler: &IAssocHandler, recommended: bool) -> RawHandler {
    // SAFETY: each call returns a string for `take_string`; the icon location is two out-parameters.
    unsafe {
        let name = handler.GetName().map(take_string).unwrap_or_default();
        let ui_name = handler.GetUIName().map(take_string).unwrap_or_default();
        let mut path = PWSTR::null();
        let mut index = 0i32;
        let icon = handler
            .GetIconLocation(&mut path, &mut index)
            .ok()
            .filter(|_| !path.is_null())
            .map(|_| format!("{},{index}", take_string(path)));
        RawHandler {
            name,
            ui_name,
            recommended,
            icon,
        }
    }
}

fn raw_handlers(extension: &str) -> Result<Vec<RawHandler>> {
    let mut all: Vec<RawHandler> = enumerate(extension, ASSOC_FILTER_NONE)?
        .iter()
        .map(|handler| describe(handler, false))
        .collect();
    let recommended: Vec<String> = enumerate(extension, ASSOC_FILTER_RECOMMENDED)?
        .iter()
        .map(|handler| describe(handler, true).name)
        .collect();
    for handler in &mut all {
        handler.recommended = recommended.contains(&handler.name);
    }
    Ok(all)
}

fn raw_default(extension: &str) -> Option<RawDefault> {
    Some(RawDefault {
        executable: assoc_string(ASSOCSTR_EXECUTABLE, extension)?,
        friendly_name: assoc_string(ASSOCSTR_FRIENDLYAPPNAME, extension),
    })
}

fn local_paths(targets: &[Target]) -> Result<Vec<PathBuf>> {
    targets
        .iter()
        .map(|target| {
            target
                .path
                .clone()
                .ok_or_else(|| MimeAppsError::InvalidUri {
                    uri: target.uri.clone(),
                })
        })
        .collect()
}

/// A data object for the files, which is what a handler is invoked with.
fn data_object(paths: &[PathBuf]) -> Result<IDataObject> {
    let mut pidls: Vec<*mut ITEMIDLIST> = Vec::new();
    let free = |pidls: &[*mut ITEMIDLIST]| {
        for pidl in pidls {
            // SAFETY: each was allocated by `SHParseDisplayName`.
            unsafe { CoTaskMemFree(Some(*pidl as *const _)) };
        }
    };
    for path in paths {
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        // SAFETY: `pidl` receives an allocation owned by us.
        let parsed = unsafe {
            SHParseDisplayName(
                &HSTRING::from(path.as_os_str()),
                None::<&IBindCtx>,
                &mut pidl,
                0,
                None,
            )
        };
        if let Err(error) = parsed {
            free(&pidls);
            return Err(failed(&error));
        }
        pidls.push(pidl);
    }
    let absolute: Vec<*const ITEMIDLIST> = pidls.iter().map(|pidl| *pidl as *const _).collect();
    // SAFETY: with no parent folder the identifiers are absolute, which they are.
    let object = unsafe {
        SHCreateDataObject::<_, IDataObject>(None, Some(&absolute), None::<&IDataObject>)
    };
    free(&pidls);
    object.map_err(|error| failed(&error))
}

/// `ShellExecuteW` with the default verb, for a path or a URI.
fn shell_open(what: &str) -> Result<()> {
    // SAFETY: plain call with valid strings; the result is a code, not a handle to free.
    let result = unsafe {
        ShellExecuteW(
            None,
            &HSTRING::from("open"),
            &HSTRING::from(what),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(MimeAppsError::failed(format!(
            "the shell could not open it (code {})",
            result.0 as isize
        )))
    }
}

impl Backend for Platform {
    fn status(&self) -> PluginStatus {
        PluginStatus::build(
            Flavour::Windows,
            vec![
                FeatureStatus::available(FEATURE_TYPE_INFO),
                FeatureStatus::available(FEATURE_HANDLERS),
                FeatureStatus::available(FEATURE_OPEN_WITH),
                FeatureStatus::available(FEATURE_OPEN_DEFAULT),
                FeatureStatus::unavailable(
                    FEATURE_SET_DEFAULT,
                    Reason::ManagedBySystem,
                    "Windows does not allow an application to change the default silently; open the Default apps page in Settings instead",
                ),
                FeatureStatus::available(FEATURE_CHOOSER),
                FeatureStatus::unavailable(
                    FEATURE_APP_ICONS,
                    Reason::NotImplemented,
                    "application icons are not served on Windows yet",
                ),
            ],
        )
    }

    fn type_info(&self, target: &Target, _sniff: bool) -> Result<TypeInfo> {
        let target = target.clone();
        on_sta(move || {
            if target.is_directory() {
                return Ok(TypeInfo {
                    mime: DIRECTORY_TYPE.into(),
                    description: assoc_string(ASSOCSTR_FRIENDLYDOCNAME, "Directory")
                        .unwrap_or_else(|| DIRECTORY_TYPE.into()),
                    icon: None,
                });
            }
            let Some(extension) = extension_of(&target) else {
                return Ok(TypeInfo {
                    mime: "application/octet-stream".into(),
                    description: "File".into(),
                    icon: None,
                });
            };
            let mime =
                assoc_string(ASSOCSTR_CONTENTTYPE, &extension).unwrap_or_else(|| extension.clone());
            let description =
                assoc_string(ASSOCSTR_FRIENDLYDOCNAME, &extension).unwrap_or_else(|| mime.clone());
            Ok(TypeInfo {
                mime,
                description,
                icon: None,
            })
        })
    }

    fn handlers(&self, targets: &[Target]) -> Result<Handlers> {
        let first = targets.first().ok_or(MimeAppsError::Empty)?;
        if first.is_directory() {
            return Ok(Handlers {
                mime: DIRECTORY_TYPE.into(),
                mixed: false,
                default: None,
                recommended: Vec::new(),
                others: Vec::new(),
            });
        }
        let extensions: Vec<Option<String>> = targets.iter().map(extension_of).collect();
        let mixed = extensions
            .iter()
            .any(|extension| *extension != extensions[0]);
        let extension = extensions[0].clone().ok_or(MimeAppsError::NoHandler {
            mime: "application/octet-stream".into(),
        })?;
        on_sta(move || {
            let mime =
                assoc_string(ASSOCSTR_CONTENTTYPE, &extension).unwrap_or_else(|| extension.clone());
            let mut lists = assoc::build(
                &mime,
                &raw_handlers(&extension)?,
                raw_default(&extension).as_ref(),
            );
            lists.mixed = mixed;
            Ok(lists)
        })
    }

    fn open_with(&self, targets: &[Target], app_id: &str) -> Result<()> {
        let first = targets.first().ok_or(MimeAppsError::Empty)?;
        let extension = extension_of(first).ok_or(MimeAppsError::Unsupported)?;
        let paths = local_paths(targets)?;
        let app_id = app_id.to_string();
        on_sta(move || {
            let handlers = enumerate(&extension, ASSOC_FILTER_NONE)?;
            let handler = handlers
                .iter()
                .find(|handler| describe(handler, false).name == app_id)
                .ok_or(MimeAppsError::AppNotFound)?;
            let object = data_object(&paths)?;
            // SAFETY: the data object is valid for the call.
            unsafe { handler.Invoke(&object) }.map_err(|error| failed(&error))
        })
    }

    fn open_default(&self, targets: &[Target]) -> Result<()> {
        if targets.is_empty() {
            return Err(MimeAppsError::Empty);
        }
        let places: Vec<String> = targets
            .iter()
            .map(|target| match &target.path {
                Some(path) => path.display().to_string(),
                None => target.uri.clone(),
            })
            .collect();
        on_sta(move || places.iter().try_for_each(|place| shell_open(place)))
    }

    fn choose(&self, targets: &[Target], parent: ParentWindow) -> Result<()> {
        // The dialog is about one file; for several the front end draws its own list.
        let [target] = targets else {
            return Err(if targets.is_empty() {
                MimeAppsError::Empty
            } else {
                MimeAppsError::Unsupported
            });
        };
        let path = target
            .path
            .clone()
            .ok_or_else(|| MimeAppsError::InvalidUri {
                uri: target.uri.clone(),
            })?;
        on_sta(move || {
            let file = HSTRING::from(path.as_os_str());
            let info = OPENASINFO {
                pcszFile: PCWSTR(file.as_ptr()),
                pcszClass: PCWSTR::null(),
                oaifInFlags: OAIF_ALLOW_REGISTRATION | OAIF_EXEC,
            };
            let owner = parent.map(|raw| HWND(raw as *mut core::ffi::c_void));
            // SAFETY: `info` and the string it points into outlive the call, which blocks until the dialog closes.
            unsafe { SHOpenWithDialog(owner, &info) }.map_err(|error| failed(&error))
        })
    }

    fn set_default(&self, _mime: &str, _app_id: &str) -> Result<()> {
        Err(MimeAppsError::Unsupported)
    }

    fn open_default_apps_settings(&self) -> Result<()> {
        on_sta(|| shell_open(SETTINGS_PAGE))
    }

    fn knows_app(&self, _app_id: &str) -> bool {
        false
    }

    fn app_icon(&self, _app_id: &str, _size: u32) -> Option<Vec<u8>> {
        None
    }
}

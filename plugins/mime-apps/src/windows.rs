// The Windows backend: the shell's association handlers to list and open, `SHOpenWithDialog` to choose, and `AssocQueryStringW` to read the default
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The shell's COM objects want a single-threaded apartment, and a call here can wait on a dialog, so each one runs on a thread of its own with COM initialised for just that call. Handlers belong to an extension (`.png`), so a type is the extension's registered content type, or the extension itself when it has none.
//!
//! Windows does not let an application change the default silently: `set_default` is unavailable, and `open_default_apps_settings` opens the Settings page where the person does it. A handler's id is the name the shell gives it (`IAssocHandler::GetName`).

use std::path::PathBuf;

use tauri::{AppHandle, Runtime};
use windows::core::{GUID, HRESULT, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_FLAGS_AND_ATTRIBUTES,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, IBindCtx, IDataObject, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    AssocQueryStringW, FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music,
    FOLDERID_Pictures, FOLDERID_Profile, FOLDERID_Public, FOLDERID_Templates, FOLDERID_Videos,
    IAssocHandler, SHAssocEnumHandlers, SHCreateDataObject, SHGetFileInfoW, SHGetImageList,
    SHGetKnownFolderIDList, SHOpenWithDialog, SHParseDisplayName, ShellExecuteW,
    ASSOCF_INIT_IGNOREUNKNOWN, ASSOCSTR, ASSOCSTR_CONTENTTYPE, ASSOCSTR_EXECUTABLE,
    ASSOCSTR_FRIENDLYAPPNAME, ASSOCSTR_FRIENDLYDOCNAME, ASSOC_FILTER, ASSOC_FILTER_NONE,
    ASSOC_FILTER_RECOMMENDED, OAIF_ALLOW_REGISTRATION, OAIF_EXEC, OPENASINFO, SHFILEINFOW,
    SHGFI_OVERLAYINDEX, SHGFI_PIDL, SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetIconInfo, HICON, ICONINFO, SW_SHOWNORMAL,
};

use crate::assoc::{self, RawDefault, RawHandler};
use crate::backend::{Backend, ParentWindow};
use crate::error::{MimeAppsError, Result};
use crate::models::{
    FeatureStatus, Flavour, Handlers, PluginStatus, Reason, TypeInfo, DIRECTORY_TYPE,
    FEATURE_APP_ICONS, FEATURE_CHOOSER, FEATURE_FOLDER_ICONS, FEATURE_HANDLERS,
    FEATURE_OPEN_DEFAULT, FEATURE_OPEN_WITH, FEATURE_SET_DEFAULT, FEATURE_TYPE_ICONS,
    FEATURE_TYPE_INFO,
};
use crate::shellicon::{self, ImageList};
use crate::target::Target;
use crate::typeicons::{extension_for_mime, FolderKind, IconKind, IconRequest};

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

/// The known folder that has a folder kind's own icon in Explorer, or `None` for an ordinary folder.
fn known_folder(kind: FolderKind) -> Option<&'static GUID> {
    Some(match kind {
        FolderKind::Plain => return None,
        FolderKind::Home => &FOLDERID_Profile,
        FolderKind::Desktop => &FOLDERID_Desktop,
        FolderKind::Documents => &FOLDERID_Documents,
        FolderKind::Downloads => &FOLDERID_Downloads,
        FolderKind::Pictures => &FOLDERID_Pictures,
        FolderKind::Music => &FOLDERID_Music,
        FolderKind::Videos => &FOLDERID_Videos,
        FolderKind::Templates => &FOLDERID_Templates,
        FolderKind::Public => &FOLDERID_Public,
    })
}

/// The index of an icon in the shell's system image list: for a type, the one Explorer shows for a file with that extension (no file needs to exist: `SHGFI_USEFILEATTRIBUTES` makes the shell go by the name alone); for a folder, the folder icon, or a standard folder's own.
fn system_icon_index(kind: &IconKind) -> Option<i32> {
    let size = std::mem::size_of::<SHFILEINFOW>() as u32;
    let by_attributes = |name: &str, attributes: FILE_FLAGS_AND_ATTRIBUTES| {
        let mut info = SHFILEINFOW::default();
        let name = HSTRING::from(name);
        // SAFETY: `name` outlives the call, `info` is a valid out structure of the size passed, and with `SHGFI_USEFILEATTRIBUTES` the shell does not read the file system.
        let result = unsafe {
            SHGetFileInfoW(
                PCWSTR(name.as_ptr()),
                attributes,
                Some(&mut info),
                size,
                SHGFI_SYSICONINDEX | SHGFI_USEFILEATTRIBUTES,
            )
        };
        (result != 0).then_some(info.iIcon)
    };
    match kind {
        IconKind::Extension(extension) => {
            by_attributes(&format!(".{extension}"), FILE_ATTRIBUTE_NORMAL)
        }
        IconKind::Mime(mime) => by_attributes(extension_for_mime(mime), FILE_ATTRIBUTE_NORMAL),
        IconKind::Folder(folder) => known_folder(*folder)
            .and_then(known_folder_icon_index)
            .or_else(|| by_attributes("folder", FILE_ATTRIBUTE_DIRECTORY)),
        // Named by the caller, drawn by `file_icon_index`, which also gives the overlay.
        IconKind::File { path, .. } => file_icon_index(path).map(|(index, _)| index),
    }
}

/// The index in the shell's system image list of the icon stored in a file, with the overlay the shell draws over it (a shortcut's arrow, as the bits `GetIcon` takes; zero for none). The shell reads the file, so a cloud placeholder is left out, and only a shortcut asks for its overlay.
fn file_icon_index(path: &str) -> Option<(i32, u32)> {
    use std::os::windows::fs::MetadataExt;
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || shellicon::is_placeholder(metadata.file_attributes()) {
        return None;
    }
    let shortcut = path.to_ascii_lowercase().ends_with(".lnk");
    let flags = if shortcut {
        SHGFI_SYSICONINDEX | SHGFI_OVERLAYINDEX
    } else {
        SHGFI_SYSICONINDEX
    };
    let name = HSTRING::from(path);
    let mut info = SHFILEINFOW::default();
    // SAFETY: `name` outlives the call and `info` is a valid out structure of the size passed; without `SHGFI_USEFILEATTRIBUTES` the shell reads the file, which was checked above not to be a placeholder.
    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(name.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        )
    };
    (result != 0).then(|| shellicon::split_overlay(info.iIcon))
}

/// The icon index of a known folder, by its item identifier list.
fn known_folder_icon_index(id: &GUID) -> Option<i32> {
    // SAFETY: the list the shell allocates is used only for the call below and freed once; `info` is a valid out structure of the size passed.
    unsafe {
        let pidl = SHGetKnownFolderIDList(id, 0, None).ok()?;
        let mut info = SHFILEINFOW::default();
        let result = SHGetFileInfoW(
            PCWSTR(pidl as *const u16),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX | SHGFI_PIDL,
        );
        CoTaskMemFree(Some(pidl as *const _));
        (result != 0).then_some(info.iIcon)
    }
}

/// An icon's pixels as straight RGBA, with its width and height.
fn icon_pixels(icon: HICON) -> Option<(u32, u32, Vec<u8>)> {
    let mut info = ICONINFO::default();
    // SAFETY: `icon` is a live handle; on success the two bitmaps in `info` are ours to delete, which `release` below does once.
    unsafe { GetIconInfo(icon, &mut info) }.ok()?;
    let release = |info: &ICONINFO| unsafe {
        // SAFETY: each bitmap was created by `GetIconInfo` for us and is deleted once; a null one is skipped.
        for bitmap in [info.hbmColor, info.hbmMask] {
            if !bitmap.is_invalid() {
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
        }
    };
    let pixels = (|| {
        if info.hbmColor.is_invalid() {
            return None;
        }
        let mut bitmap = BITMAP::default();
        // SAFETY: `bitmap` is a valid out structure of the size passed, for a bitmap handle that is live.
        let got = unsafe {
            GetObjectW(
                HGDIOBJ(info.hbmColor.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bitmap as *mut BITMAP as *mut _),
            )
        };
        if got == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
            return None;
        }
        let (width, height) = (bitmap.bmWidth as u32, bitmap.bmHeight as u32);
        // SAFETY: a memory device context for the screen, deleted below.
        let dc = unsafe { CreateCompatibleDC(None) };
        let read = |bitmap: HBITMAP| -> Option<Vec<u8>> {
            let mut header = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    // Negative: rows from the top, as an image file wants them.
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut buffer = vec![0u8; (width * height * 4) as usize];
            // SAFETY: the buffer holds `width * height` 32-bit pixels, which is what the header asks for; the bitmap is not selected into another device context.
            let lines = unsafe {
                GetDIBits(
                    dc,
                    bitmap,
                    0,
                    height,
                    Some(buffer.as_mut_ptr() as *mut _),
                    &mut header,
                    DIB_RGB_COLORS,
                )
            };
            (lines != 0).then_some(buffer)
        };
        let mut colour = read(info.hbmColor);
        let mask = if info.hbmMask.is_invalid() {
            None
        } else {
            read(info.hbmMask)
        };
        // SAFETY: the context was created above and is deleted once.
        let _ = unsafe { DeleteDC(dc) };
        if let Some(buffer) = colour.as_mut() {
            shellicon::bgra_to_rgba(buffer, mask.as_deref());
        }
        colour.map(|buffer| (width, height, buffer))
    })();
    release(&info);
    pixels
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

/// The shell's icon for a type or a folder as PNG bytes, from the system image list whose icons are the smallest that are big enough. Needs a single-threaded apartment.
fn shell_icon(request: &IconRequest) -> Option<Vec<u8>> {
    let (index, overlay) = match &request.kind {
        IconKind::File { path, .. } => file_icon_index(path)?,
        kind => (system_icon_index(kind)?, 0),
    };
    let list = ImageList::for_pixels(request.pixels());
    // SAFETY: the shell hands back a reference-counted interface, released when it drops; the icon it gives is ours to destroy, which happens once below.
    unsafe {
        let images: IImageList = SHGetImageList(list.shil()).ok()?;
        let icon = images.GetIcon(index, ILD_TRANSPARENT.0 | overlay).ok()?;
        let pixels = icon_pixels(icon);
        let _ = DestroyIcon(icon);
        let (width, height, rgba) = pixels?;
        encode_png(width, height, &rgba)
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
                FeatureStatus::available(FEATURE_TYPE_ICONS),
                FeatureStatus::available(FEATURE_FOLDER_ICONS),
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

    fn type_icon(&self, request: &IconRequest) -> Option<Vec<u8>> {
        let request = request.clone();
        on_sta(move || shell_icon(&request).ok_or(MimeAppsError::Unsupported)).ok()
    }
}

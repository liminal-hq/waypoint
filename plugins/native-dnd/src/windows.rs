// Windows: outbound drags through OLE `DoDragDrop`, the OLE file clipboard, and modifiers read from the keyboard state
//
// A drag offers a hand-written `IDataObject` holding `CF_HDROP` and `Preferred DropEffect`, with an `IDropSource` that cancels on Escape and drops when the button that started the drag is released. `DoDragDrop` is a modal loop: it runs on the main thread and returns when the drag is over, so the command resolves then. Positions of inbound events are physical client-area pixels.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    cell::{Cell, RefCell},
    mem::ManuallyDrop,
    sync::OnceLock,
};

use log::{debug, warn};
use tauri::{AppHandle, Emitter, Runtime, Webview, WebviewWindow};
use windows::{
    core::{implement, w, BOOL, HRESULT, PCWSTR},
    Win32::{
        Foundation::{
            DATA_S_SAMEFORMATETC, DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS,
            DV_E_FORMATETC, DV_E_TYMED, E_NOTIMPL, HGLOBAL, HWND, LPARAM, LRESULT,
            OLE_E_ADVISENOTSUPPORTED, S_OK, WPARAM,
        },
        System::{
            Com::{
                IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA,
                DATADIR_GET, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
            },
            DataExchange::{AddClipboardFormatListener, RegisterClipboardFormatW},
            LibraryLoader::GetModuleHandleW,
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::{
                DoDragDrop, IDropSource, IDropSource_Impl, OleFlushClipboard, OleGetClipboard,
                OleInitialize, OleIsCurrentClipboard, OleSetClipboard, ReleaseStgMedium, CF_HDROP,
                DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_MOVE,
            },
            SystemServices::{MK_LBUTTON, MK_RBUTTON, MODIFIERKEYS_FLAGS},
        },
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LBUTTON, VK_MENU, VK_RBUTTON,
                VK_SHIFT,
            },
            Shell::{DragQueryFileW, SHCreateStdEnumFmtEtc, HDROP},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, RegisterClassW, HWND_MESSAGE, WINDOW_EX_STYLE,
                WINDOW_STYLE, WM_CLIPBOARDUPDATE, WNDCLASSW,
            },
        },
    },
};

use crate::{
    error::{Error, Result},
    inbound::PositionUnit,
    models::{ClipboardFiles, DisplayServer, Modifiers, CLIPBOARD_CHANGED_EVENT},
    outbound::{classify_ole, ole_effects, Begun, DragRequest, Finisher},
    platform::InboundExtras,
    uri,
};

/// Windows reports physical client-area pixels.
pub const POSITION_UNIT: PositionUnit = PositionUnit::Physical;

pub fn display_server() -> DisplayServer {
    DisplayServer::Windows
}

pub fn unavailable_reason() -> String {
    String::new()
}

fn key_down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: `GetAsyncKeyState` takes a virtual-key code and reads global input state.
    let state = unsafe { GetAsyncKeyState(i32::from(key.0)) };
    state < 0
}

/// Whether a mouse button is down. `GetAsyncKeyState` reports the physical buttons, so with swapped buttons the logical primary one is the right physical one, and either counts: the drag's source ends it by whichever button started it.
pub fn primary_button_down<R: Runtime>(_window: &WebviewWindow<R>) -> bool {
    key_down(VK_LBUTTON) || key_down(VK_RBUTTON)
}

pub fn modifiers_now() -> Modifiers {
    Modifiers {
        ctrl: key_down(VK_CONTROL),
        shift: key_down(VK_SHIFT),
        alt: key_down(VK_MENU),
    }
}

pub fn inbound_extras(_label: &str, _consume: bool) -> InboundExtras {
    InboundExtras::default()
}

pub fn on_webview_ready<R: Runtime>(_webview: &Webview<R>) {}

fn preferred_drop_effect_format() -> u16 {
    // SAFETY: registering a clipboard format by name has no preconditions.
    unsafe { RegisterClipboardFormatW(w!("Preferred DropEffect")) as u16 }
}

fn hdrop_format() -> FORMATETC {
    FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    }
}

fn effect_format() -> FORMATETC {
    FORMATETC {
        cfFormat: preferred_drop_effect_format(),
        ..hdrop_format()
    }
}

/// The bytes of a `CF_HDROP` global: a `DROPFILES` header (`pFiles` = its 20 bytes, no point, wide characters) and the paths, each ended by a NUL and the list by one more.
fn hdrop_bytes(paths: &[Vec<u16>]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&20u32.to_le_bytes()); // pFiles
    bytes.extend_from_slice(&0i32.to_le_bytes()); // pt.x
    bytes.extend_from_slice(&0i32.to_le_bytes()); // pt.y
    bytes.extend_from_slice(&0u32.to_le_bytes()); // fNC
    bytes.extend_from_slice(&1u32.to_le_bytes()); // fWide
    for path in paths {
        for unit in path {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());
    }
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes
}

/// A movable global holding `bytes`, owned by the caller (or the clipboard it is handed to).
fn global_with(bytes: &[u8]) -> windows::core::Result<HGLOBAL> {
    // SAFETY: the global is allocated with room for `bytes`, locked only while they are copied in, and returned unlocked.
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len())?;
        let target = GlobalLock(handle).cast::<u8>();
        if target.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
        let _ = GlobalUnlock(handle);
        Ok(handle)
    }
}

fn medium_with(bytes: &[u8]) -> windows::core::Result<STGMEDIUM> {
    Ok(STGMEDIUM {
        tymed: TYMED_HGLOBAL.0 as u32,
        u: STGMEDIUM_0 {
            hGlobal: global_with(bytes)?,
        },
        pUnkForRelease: ManuallyDrop::new(None),
    })
}

/// The files of a drag or of the clipboard, as an OLE data object.
#[implement(IDataObject)]
struct FileData {
    hdrop: Vec<u8>,
    effect: u32,
}

impl FileData {
    fn new(uris: &[String], effect: u32) -> Result<Self> {
        let paths: Vec<Vec<u16>> = uris
            .iter()
            .map(|u| {
                uri::uri_to_windows_path(u)
                    .map(|path| path.encode_utf16().collect())
                    .ok_or_else(|| Error::Invalid(format!("not a file URI Windows can open: {u}")))
            })
            .collect::<Result<_>>()?;
        Ok(FileData {
            hdrop: hdrop_bytes(&paths),
            effect,
        })
    }

    /// Whether `format` asks for one of the two formats offered, in a global.
    fn check(&self, format: *const FORMATETC) -> HRESULT {
        // SAFETY: OLE passes a valid `FORMATETC` pointer for the call.
        let Some(format) = (unsafe { format.as_ref() }) else {
            return DV_E_FORMATETC;
        };
        let known = format.cfFormat == CF_HDROP.0 || format.cfFormat == effect_format().cfFormat;
        if !known || format.dwAspect != DVASPECT_CONTENT.0 {
            return DV_E_FORMATETC;
        }
        if format.tymed & TYMED_HGLOBAL.0 as u32 == 0 {
            return DV_E_TYMED;
        }
        S_OK
    }
}

impl IDataObject_Impl for FileData_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        let checked = self.check(pformatetcin);
        if checked != S_OK {
            return Err(checked.into());
        }
        // SAFETY: `check` returned `S_OK`, so the pointer is non-null.
        let format = unsafe { &*pformatetcin };
        if format.cfFormat == CF_HDROP.0 {
            medium_with(&self.hdrop)
        } else {
            medium_with(&self.effect.to_le_bytes())
        }
    }

    fn GetDataHere(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *mut STGMEDIUM,
    ) -> windows::core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        self.check(pformatetc)
    }

    fn GetCanonicalFormatEtc(
        &self,
        _pformatectin: *const FORMATETC,
        pformatetcout: *mut FORMATETC,
    ) -> HRESULT {
        // SAFETY: OLE passes a writable pointer, or null.
        if let Some(out) = unsafe { pformatetcout.as_mut() } {
            out.ptd = std::ptr::null_mut();
        }
        DATA_S_SAMEFORMATETC
    }

    fn SetData(
        &self,
        _pformatetc: *const FORMATETC,
        _pmedium: *const STGMEDIUM,
        _frelease: BOOL,
    ) -> windows::core::Result<()> {
        // A paste or drop target tells the source what it did (`Performed DropEffect`); nothing here needs to know.
        Err(E_NOTIMPL.into())
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> windows::core::Result<IEnumFORMATETC> {
        if dwdirection != DATADIR_GET.0 as u32 {
            return Err(E_NOTIMPL.into());
        }
        // SAFETY: the shell copies the array.
        unsafe { SHCreateStdEnumFmtEtc(&[hdrop_format(), effect_format()]) }
    }

    fn DAdvise(
        &self,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _padvsink: windows::core::Ref<IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn DUnadvise(&self, _dwconnection: u32) -> windows::core::Result<()> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
}

/// Ends a drag when Escape is pressed (cancel) or when the mouse button that started it is released (drop).
#[implement(IDropSource)]
#[derive(Default)]
struct DropSource {
    /// The mouse buttons that were down when the drag began, learnt from the first query.
    held: Cell<Option<u32>>,
}

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, fescapepressed: BOOL, grfkeystate: MODIFIERKEYS_FLAGS) -> HRESULT {
        if fescapepressed.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        let buttons = grfkeystate.0 & (MK_LBUTTON.0 | MK_RBUTTON.0);
        let started_with = match self.held.get() {
            Some(held) => held,
            None => {
                self.held.set(Some(buttons));
                buttons
            }
        };
        if buttons & started_with == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

fn initialise_ole() {
    // SAFETY: OLE initialisation on the thread that owns the windows; an error means it was already initialised (another way), which is fine.
    unsafe {
        let _ = OleInitialize(None);
    }
}

/// Runs a drag to its end on the main thread. The drag image is drawn by Windows, so `request.icon` is not used.
pub fn begin_drag<R: Runtime>(
    _app: &AppHandle<R>,
    _window: &WebviewWindow<R>,
    _id: u32,
    request: &DragRequest,
    _finisher: Finisher,
) -> Result<Begun> {
    initialise_ole();
    let offered = ole_effects(request.actions);
    let preferred = if request.actions.copy {
        DROPEFFECT_COPY.0
    } else {
        DROPEFFECT_MOVE.0
    };
    let data: IDataObject = FileData::new(&request.uris, preferred)?.into();
    let source: IDropSource = DropSource::default().into();
    let mut effect = DROPEFFECT(0);
    // SAFETY: both objects are live COM objects for the call, and `effect` is a valid out pointer. The modal loop runs here and returns when the drag is over.
    let hr = unsafe { DoDragDrop(&data, &source, DROPEFFECT(offered), &mut effect) };
    let (outcome, reason) = classify_ole(hr.0, effect.0);
    Ok(Begun::Done(outcome, reason))
}

thread_local! {
    /// The data object this process last put on the clipboard, so it can be flushed at exit.
    static OWNED: RefCell<Option<IDataObject>> = const { RefCell::new(None) };
}

/// `OpenClipboard` fails with this when another process (a clipboard viewer, the VM's clipboard sync) has it open for a moment.
const CLIPBRD_E_CANT_OPEN: HRESULT = HRESULT(0x8004_01D0_u32 as i32);

/// Runs a clipboard call again for a short while while another process holds the clipboard open.
fn retry_while_busy<T>(
    mut call: impl FnMut() -> windows::core::Result<T>,
) -> windows::core::Result<T> {
    let mut attempts = 0;
    loop {
        match call() {
            Err(error) if error.code() == CLIPBRD_E_CANT_OPEN && attempts < 10 => {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            other => return other,
        }
    }
}

/// Puts `files` on the OLE clipboard as `CF_HDROP` with a `Preferred DropEffect` of copy or move. Must run on the main thread.
pub fn set_files(files: &ClipboardFiles) -> Result<()> {
    initialise_ole();
    let effect = if files.cut {
        DROPEFFECT_MOVE.0
    } else {
        DROPEFFECT_COPY.0
    };
    let data: IDataObject = FileData::new(&files.uris, effect)?.into();
    // SAFETY: `data` is a live COM object; OLE takes its own reference.
    retry_while_busy(|| unsafe { OleSetClipboard(&data) })
        .map_err(|error| Error::Failed(format!("OleSetClipboard failed: {error}")))?;
    OWNED.with(|owned| *owned.borrow_mut() = Some(data));
    Ok(())
}

/// The files on the OLE clipboard. Must run on the main thread.
pub fn get_files() -> Result<Option<ClipboardFiles>> {
    initialise_ole();
    // SAFETY: plain OLE calls on the thread that owns the windows; the medium is released before returning.
    unsafe {
        let data = retry_while_busy(|| OleGetClipboard())
            .map_err(|error| Error::Failed(format!("OleGetClipboard failed: {error}")))?;
        let hdrop = hdrop_format();
        if data.QueryGetData(&hdrop) != S_OK {
            return Ok(None);
        }
        let mut medium = data.GetData(&hdrop).map_err(|error| {
            Error::Failed(format!("the clipboard's file list is unreadable: {error}"))
        })?;
        let list = HDROP(medium.u.hGlobal.0);
        let count = DragQueryFileW(list, u32::MAX, None);
        let mut uris = Vec::with_capacity(count as usize);
        for index in 0..count {
            let length = DragQueryFileW(list, index, None) as usize;
            let mut buffer = vec![0u16; length + 1];
            let written = DragQueryFileW(list, index, Some(&mut buffer)) as usize;
            if let Some(uri) =
                uri::windows_path_to_uri(&String::from_utf16_lossy(&buffer[..written]))
            {
                uris.push(uri);
            }
        }
        ReleaseStgMedium(&mut medium);
        if uris.is_empty() {
            return Ok(None);
        }
        let cut = read_effect(&data).is_some_and(|effect| effect & DROPEFFECT_MOVE.0 != 0);
        Ok(Some(ClipboardFiles { uris, cut }))
    }
}

/// The `Preferred DropEffect` of a data object, if it has one.
///
/// # Safety
/// Calls into a live COM object and reads the global it returns.
unsafe fn read_effect(data: &IDataObject) -> Option<u32> {
    let format = effect_format();
    if data.QueryGetData(&format) != S_OK {
        return None;
    }
    let mut medium = data.GetData(&format).ok()?;
    let handle = medium.u.hGlobal;
    let pointer = GlobalLock(handle).cast::<u32>();
    let effect = (!pointer.is_null()).then(|| pointer.read_unaligned());
    let _ = GlobalUnlock(handle);
    ReleaseStgMedium(&mut medium);
    effect
}

static CLIPBOARD_CALLBACK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

unsafe extern "system" fn listener_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CLIPBOARDUPDATE {
        if let Some(callback) = CLIPBOARD_CALLBACK.get() {
            callback();
        }
        return LRESULT(0);
    }
    // SAFETY: forwards the window's own message with its own arguments.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// Listens for clipboard changes with a message-only window on the main thread, whose messages the event loop dispatches.
pub fn on_ready<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    let callback: Box<dyn Fn() + Send + Sync> = Box::new(move || {
        if let Err(error) = handle.emit(CLIPBOARD_CHANGED_EVENT, ()) {
            warn!("native-dnd: cannot send the clipboard change: {error}");
        }
    });
    if CLIPBOARD_CALLBACK.set(callback).is_err() {
        return;
    }
    // SAFETY: registers a window class and a message-only window of this module, then subscribes it to clipboard updates.
    unsafe {
        let Ok(instance) = GetModuleHandleW(PCWSTR::null()) else {
            return;
        };
        let class = w!("TauriPluginNativeDndClipboard");
        let description = WNDCLASSW {
            lpfnWndProc: Some(listener_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&description);
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        );
        match window {
            Ok(window) => {
                if let Err(error) = AddClipboardFormatListener(window) {
                    debug!("native-dnd: no clipboard listener: {error}");
                }
            }
            Err(error) => debug!("native-dnd: no clipboard listener window: {error}"),
        }
    }
}

/// Renders what this process put on the clipboard into the system, so a paste still works after the app has quit.
pub fn on_exit<R: Runtime>(_app: &AppHandle<R>) {
    OWNED.with(|owned| {
        if let Some(data) = owned.borrow_mut().take() {
            // SAFETY: plain OLE calls with a live data object.
            unsafe {
                if OleIsCurrentClipboard(&data).is_ok() {
                    let _ = OleFlushClipboard();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };

    #[test]
    fn the_file_list_has_a_dropfiles_header_and_double_nul() {
        let bytes = hdrop_bytes(&[vec![0x43, 0x3a], vec![0x44]]);
        assert_eq!(&bytes[..4], &20u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &1u32.to_le_bytes());
        // "C:" NUL "D" NUL NUL
        assert_eq!(bytes[20..], [0x43, 0, 0x3a, 0, 0, 0, 0x44, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn file_data_needs_windows_paths() {
        assert!(FileData::new(&["file:///C:/a%20b.txt".into()], 1).is_ok());
        assert!(FileData::new(&["https://x/y".into()], 1).is_err());
    }

    /// Needs a desktop session: run with `cargo test -p tauri-plugin-native-dnd --lib -- --ignored --nocapture`.
    #[test]
    #[ignore = "uses the real clipboard"]
    fn live_clipboard_round_trip() {
        let uris = vec![
            "file:///C:/Windows/notepad.exe".to_string(),
            "file:///C:/Windows/System32/cmd.exe".to_string(),
        ];
        for cut in [false, true] {
            set_files(&ClipboardFiles {
                uris: uris.clone(),
                cut,
            })
            .expect("set");
            let got = get_files().expect("get").expect("files on the clipboard");
            assert_eq!(got.cut, cut);
            assert_eq!(got.uris.len(), 2, "{got:?}");
            assert!(
                got.uris[0].to_lowercase().ends_with("windows/notepad.exe"),
                "{got:?}"
            );
        }
        println!("clipboard ready for another process to read");
        // OLE answers other processes' requests through this thread's message loop, as the app's event loop does.
        let hold = std::env::var("HOLD_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(hold);
        while std::time::Instant::now() < until {
            let mut message = MSG::default();
            // SAFETY: a plain message pump on the thread that owns the clipboard data object.
            unsafe {
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

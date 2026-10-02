// The Windows backend: drive letters and mapped drives through the Win32 drive APIs, live changes through a message-only window, eject through the shell
//
// Windows mounts every drive itself, so `mount`, `unmount` and `unlock` are reported unavailable: there is no drive-letter mount to ask for, a dismount without an eject leaves the volume half-removed, and BitLocker belongs to the system's own prompt. `eject` is the shell's own `Eject` verb on the drive, the same as the Safely Remove entry in Explorer.
//
// Every query that can block (the volume information of a drive with no disc, the name of a stalled mapped drive) runs on a worker thread with a timeout, so one dead drive never stalls the list.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows::core::{w, PCSTR, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WIN32_ERROR, WPARAM};
use windows::Win32::NetworkManagement::WNet::WNetGetConnectionW;
use windows::Win32::Storage::FileSystem::{
    GetDriveTypeW, GetLogicalDriveStringsW, GetLogicalDrives, GetVolumeInformationW,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Diagnostics::Debug::{SetThreadErrorMode, SEM_FAILCRITICALERRORS};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IContextMenu, IShellFolder, SHBindToParent, SHParseDisplayName, CMINVOKECOMMANDINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    GetMessageW, GetWindowLongPtrW, PostMessageW, PostQuitMessage, RegisterClassW,
    RegisterDeviceNotificationW, SetTimer, SetWindowLongPtrW, TranslateMessage,
    DEVICE_NOTIFY_WINDOW_HANDLE, DEV_BROADCAST_DEVICEINTERFACE_W, GWLP_USERDATA, HMENU,
    HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY, WM_DEVICECHANGE,
    WM_TIMER, WNDCLASSW,
};

use crate::backend::{Backend, BoxFuture, Notify};
use crate::error::{Result, VolumesError};
use crate::models::{
    FeatureStatus, Flavour, Passphrase, PluginStatus, Reason, Volume, VolumeKind, FEATURE_EJECT,
    FEATURE_LIST, FEATURE_MOUNT, FEATURE_UNLOCK, FEATURE_UNMOUNT, FEATURE_WATCH,
};
use crate::winmap::{
    display_label, is_system_drive, kind_for_drive_type, roots_from_mask, split_drive_strings,
    unc_to_uri,
};

const ID_PREFIX: &str = "drive:";

/// How long the details of one drive may take. A drive that does not answer is listed with what is known without asking.
const DETAILS_TIMEOUT: Duration = Duration::from_secs(2);

/// How often the set of drive letters is compared, to notice a mapped drive that no device notification announces.
const POLL_MILLISECONDS: u32 = 3000;

const DBT_DEVNODES_CHANGED: usize = 0x0007;
const DBT_DEVICEARRIVAL: usize = 0x8000;
const DBT_DEVICEREMOVECOMPLETE: usize = 0x8004;
const DBT_DEVTYP_DEVICEINTERFACE: u32 = 5;
/// `GUID_DEVINTERFACE_VOLUME`: every volume that comes or goes.
const GUID_DEVINTERFACE_VOLUME: windows::core::GUID =
    windows::core::GUID::from_u128(0x53f5630d_b6bf_11d0_94f2_00a0c91efb8b);

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn text_of(buffer: &[u16]) -> String {
    let end = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

/// What asking a drive for its details gave.
#[derive(Default)]
struct Details {
    label: String,
    file_system: Option<String>,
    remote: Option<String>,
}

/// Asks one drive for its volume label and file system, or a mapped drive for its remote name. Blocks on a drive that does not answer: call it on a worker thread.
fn drive_details(root: &str, kind: VolumeKind) -> Details {
    // SAFETY: plain call that makes a missing disc an error instead of a dialog, for this thread only.
    let _ = unsafe { SetThreadErrorMode(SEM_FAILCRITICALERRORS, None) };
    let mut details = Details::default();
    if kind == VolumeKind::Network {
        let local = wide(root.trim_end_matches('\\'));
        let mut remote = vec![0u16; 1024];
        let mut length = remote.len() as u32;
        // SAFETY: `local` is NUL-terminated, `remote` has `length` units of room.
        let status: WIN32_ERROR = unsafe {
            WNetGetConnectionW(
                PCWSTR(local.as_ptr()),
                Some(PWSTR(remote.as_mut_ptr())),
                &mut length,
            )
        };
        if status.0 == 0 {
            details.remote = Some(text_of(&remote));
        }
        return details;
    }
    let root_wide = wide(root);
    let mut label = vec![0u16; 261];
    let mut file_system = vec![0u16; 261];
    // SAFETY: `root_wide` is NUL-terminated and the buffers have the room they are given.
    let result = unsafe {
        GetVolumeInformationW(
            PCWSTR(root_wide.as_ptr()),
            Some(&mut label),
            None,
            None,
            None,
            Some(&mut file_system),
        )
    };
    if result.is_ok() {
        details.label = text_of(&label);
        let name = text_of(&file_system);
        details.file_system = (!name.is_empty()).then_some(name);
    }
    details
}

/// The drive roots and their types: quick calls that do not touch the media.
fn drive_roots() -> Vec<(String, VolumeKind)> {
    let mut buffer = vec![0u16; 512];
    // SAFETY: the buffer is writable for its whole length.
    let length = unsafe { GetLogicalDriveStringsW(Some(&mut buffer)) } as usize;
    if length == 0 || length > buffer.len() {
        return Vec::new();
    }
    split_drive_strings(&buffer[..length])
        .into_iter()
        .filter_map(|root| {
            let root_wide = wide(&root);
            // SAFETY: NUL-terminated string that outlives the call.
            let drive_type = unsafe { GetDriveTypeW(PCWSTR(root_wide.as_ptr())) };
            Some((root, kind_for_drive_type(drive_type)?))
        })
        .collect()
}

fn volume_for(
    root: &str,
    kind: VolumeKind,
    details: Details,
    system_drive: Option<&str>,
) -> Volume {
    let letter = root.trim_end_matches(['\\', ':']);
    let (label, uri, device) = match (&details.remote, kind) {
        (Some(remote), VolumeKind::Network) => (
            display_label(remote.rsplit('\\').next().unwrap_or(""), kind, root),
            unc_to_uri(remote),
            Some(remote.clone()),
        ),
        _ => (display_label(&details.label, kind, root), None, None),
    };
    Volume {
        id: format!("{ID_PREFIX}{letter}"),
        label,
        kind,
        file_system: details.file_system,
        mount_point: Some(root.to_string()),
        uri,
        total: None,
        free: None,
        can_mount: false,
        can_unmount: false,
        can_eject: matches!(kind, VolumeKind::Removable | VolumeKind::Optical),
        can_power_off: false,
        locked: false,
        is_system: kind == VolumeKind::Internal && is_system_drive(root, system_drive),
        device,
    }
}

/// Lists the drives, asking each for its details on a worker thread of its own within one shared deadline.
fn list_volumes() -> Vec<Volume> {
    let system_drive = std::env::var("SystemDrive").ok();
    let deadline = Instant::now() + DETAILS_TIMEOUT;
    let pending: Vec<_> = drive_roots()
        .into_iter()
        .map(|(root, kind)| {
            let (sender, receiver) = mpsc::channel();
            let asked = root.clone();
            let spawned = std::thread::Builder::new()
                .name("volumes-drive".into())
                .spawn(move || {
                    let _ = sender.send(drive_details(&asked, kind));
                });
            (root, kind, spawned.is_ok().then_some(receiver))
        })
        .collect();
    pending
        .into_iter()
        .map(|(root, kind, receiver)| {
            let details = receiver
                .and_then(|receiver| {
                    receiver
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .ok()
                })
                .unwrap_or_default();
            volume_for(&root, kind, details, system_drive.as_deref())
        })
        .collect()
}

/// Ejects the drive with the shell's `Eject` verb, as Explorer does. Runs on a single-threaded apartment of its own.
fn eject_blocking(root: &str) -> Result<()> {
    // SAFETY: COM is initialised for this thread and uninitialised before it ends, both below.
    let initialised = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = invoke_eject(root);
    if initialised {
        // SAFETY: balances the successful `CoInitializeEx` above.
        unsafe { CoUninitialize() };
    }
    result
}

fn shell_error(error: windows::core::Error) -> VolumesError {
    // E_ACCESSDENIED
    if error.code().0 as u32 == 0x8007_0005 {
        VolumesError::NotAuthorised
    } else {
        VolumesError::io(error.message())
    }
}

fn invoke_eject(root: &str) -> Result<()> {
    let name = wide(root);
    let mut item: *mut ITEMIDLIST = std::ptr::null_mut();
    // SAFETY: `name` is NUL-terminated; `item` receives a list this function frees.
    unsafe { SHParseDisplayName(PCWSTR(name.as_ptr()), None, &mut item, 0, None) }
        .map_err(|_| VolumesError::NotFound)?;
    let outcome = (|| -> Result<()> {
        let mut child: *mut ITEMIDLIST = std::ptr::null_mut();
        // SAFETY: `item` is the list parsed above; `child` points into it.
        let parent: IShellFolder =
            unsafe { SHBindToParent(item, Some(&mut child)) }.map_err(shell_error)?;
        // SAFETY: `child` is a valid relative item of `parent`.
        let menu: IContextMenu =
            unsafe { parent.GetUIObjectOf(HWND::default(), &[child as *const _], None) }
                .map_err(shell_error)?;
        // SAFETY: a plain popup menu the shell fills with the item's verbs and that is destroyed below.
        let popup: HMENU = unsafe { CreatePopupMenu() }.map_err(shell_error)?;
        let invoked = (|| {
            // SAFETY: `popup` is a valid menu; the ids are the range the shell may use.
            unsafe { menu.QueryContextMenu(popup, 0, 1, 0x7fff, 0) }.ok()?;
            let info = CMINVOKECOMMANDINFO {
                cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
                lpVerb: PCSTR(c"eject".as_ptr().cast()),
                ..Default::default()
            };
            // SAFETY: `info` is fully initialised and its verb is NUL-terminated.
            unsafe { menu.InvokeCommand(&info) }
        })();
        // SAFETY: `popup` is the menu made above and is not used again.
        let _ = unsafe { DestroyMenu(popup) };
        // A drive with no `Eject` verb (a fixed disk) fails here.
        invoked.map_err(|_| VolumesError::Unsupported)
    })();
    // SAFETY: `item` was allocated by `SHParseDisplayName`.
    unsafe { CoTaskMemFree(Some(item as *const c_void)) };
    outcome
}

/// What the watcher's window procedure needs: whom to tell, and the drive letters last seen.
struct WatchState {
    notify: Notify,
    letters: std::cell::Cell<u32>,
}

unsafe extern "system" fn watch_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: the pointer is the `WatchState` leaked in `watch_thread`, or null before it is set and after it is freed.
    let state = unsafe { (GetWindowLongPtrW(window, GWLP_USERDATA) as *const WatchState).as_ref() };
    match message {
        WM_DEVICECHANGE => {
            if let Some(state) = state {
                if matches!(
                    wparam.0,
                    DBT_DEVNODES_CHANGED | DBT_DEVICEARRIVAL | DBT_DEVICEREMOVECOMPLETE
                ) {
                    (state.notify)();
                }
            }
            LRESULT(1)
        }
        WM_TIMER => {
            if let Some(state) = state {
                // SAFETY: no arguments.
                let letters = unsafe { GetLogicalDrives() };
                if state.letters.replace(letters) != letters {
                    (state.notify)();
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            // SAFETY: the window belongs to this thread.
            let _ = unsafe { DestroyWindow(window) };
            LRESULT(0)
        }
        WM_DESTROY => {
            // SAFETY: ends the loop of the thread that owns the window.
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        // SAFETY: the default handling for everything else, with the arguments as received.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

/// Runs the message-only window and its loop until the window is closed. `window` receives the handle once it exists.
fn watch_thread(notify: Notify, window: Arc<AtomicIsize>) {
    // SAFETY: the usual window-class and message-only-window sequence; every pointer passed outlives its call.
    unsafe {
        let Ok(instance) = GetModuleHandleW(None) else {
            return;
        };
        let class = WNDCLASSW {
            lpfnWndProc: Some(watch_proc),
            hInstance: instance.into(),
            lpszClassName: w!("TauriPluginVolumesWatcher"),
            ..Default::default()
        };
        // Registering twice (a second plugin instance) fails harmlessly: the class exists.
        RegisterClassW(&class);
        let Ok(handle) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("TauriPluginVolumesWatcher"),
            w!("volumes"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        ) else {
            return;
        };
        let state = Box::into_raw(Box::new(WatchState {
            notify,
            letters: std::cell::Cell::new(GetLogicalDrives()),
        }));
        SetWindowLongPtrW(handle, GWLP_USERDATA, state as isize);

        let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
            dbcc_size: std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
            dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE,
            dbcc_classguid: GUID_DEVINTERFACE_VOLUME,
            ..Default::default()
        };
        // Volumes are announced to a window that registers for their interface, message-only or not.
        let _ = RegisterDeviceNotificationW(
            windows::Win32::Foundation::HANDLE(handle.0),
            &filter as *const _ as *const c_void,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        );
        SetTimer(Some(handle), 1, POLL_MILLISECONDS, None);
        window.store(handle.0 as isize, Ordering::SeqCst);

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        window.store(0, Ordering::SeqCst);
        SetWindowLongPtrW(handle, GWLP_USERDATA, 0);
        drop(Box::from_raw(state));
    }
}

pub struct Platform {
    window: Arc<AtomicIsize>,
}

impl Platform {
    pub fn new() -> Self {
        Platform {
            window: Arc::new(AtomicIsize::new(0)),
        }
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        let handle = self.window.load(Ordering::SeqCst);
        if handle != 0 {
            // SAFETY: posting to a window of this process is always allowed; a stale handle only fails.
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(handle as *mut c_void)),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
    }
}

const NO_MOUNT: &str =
    "Windows mounts every drive by itself and gives it a letter; there is nothing to mount";
const NO_UNMOUNT: &str = "Windows has no unmount apart from eject; use Eject for removable drives";
const NO_UNLOCK: &str = "BitLocker volumes are unlocked through Windows' own prompt";

impl Backend for Platform {
    fn status(&self) -> BoxFuture<'_, PluginStatus> {
        Box::pin(async {
            PluginStatus::build(
                Flavour::Windows,
                vec![
                    FeatureStatus::available(FEATURE_LIST),
                    FeatureStatus::unavailable(FEATURE_MOUNT, Reason::NotSupported, NO_MOUNT),
                    FeatureStatus::unavailable(FEATURE_UNMOUNT, Reason::NotSupported, NO_UNMOUNT),
                    FeatureStatus::available(FEATURE_EJECT),
                    FeatureStatus::unavailable(FEATURE_UNLOCK, Reason::NotSupported, NO_UNLOCK),
                    FeatureStatus::available(FEATURE_WATCH),
                ],
            )
        })
    }

    fn volumes(&self) -> BoxFuture<'_, Result<Vec<Volume>>> {
        Box::pin(async {
            tokio::task::spawn_blocking(list_volumes)
                .await
                .map_err(|error| VolumesError::io(error.to_string()))
        })
    }

    fn mount(&self, _id: String) -> BoxFuture<'_, Result<String>> {
        Box::pin(async { Err(VolumesError::Unsupported) })
    }

    fn unmount(&self, _id: String) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Err(VolumesError::Unsupported) })
    }

    fn eject(&self, id: String) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let letter = id
                .strip_prefix(ID_PREFIX)
                .filter(|letter| letter.len() == 1 && letter.as_bytes()[0].is_ascii_alphabetic())
                .ok_or(VolumesError::NotFound)?;
            let root = format!("{}:\\", letter.to_ascii_uppercase());
            if !roots_from_mask(unsafe { GetLogicalDrives() }).contains(&root) {
                return Err(VolumesError::NotFound);
            }
            tokio::task::spawn_blocking(move || eject_blocking(&root))
                .await
                .map_err(|error| VolumesError::io(error.to_string()))?
        })
    }

    fn unlock(&self, _id: String, _passphrase: Passphrase) -> BoxFuture<'_, Result<String>> {
        Box::pin(async { Err(VolumesError::Unsupported) })
    }

    fn watch(&self, notify: Notify) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let window = Arc::clone(&self.window);
            std::thread::Builder::new()
                .name("volumes-watch".into())
                .spawn(move || watch_thread(notify, window))
                .map(|_| ())
                .map_err(|error| VolumesError::io(error.to_string()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints this machine's drives and watches for a change for a few seconds. Read-only; run it by hand on a Windows machine: `cargo test -p tauri-plugin-volumes live_volumes -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads the real drives"]
    fn live_volumes() {
        let volumes = list_volumes();
        for volume in &volumes {
            println!("{volume:#?}");
        }
        assert!(
            volumes
                .iter()
                .any(|volume| volume.kind == VolumeKind::Internal),
            "a Windows machine has a fixed drive"
        );
        let space = crate::space::system_space(volumes[0].mount_point.as_deref().unwrap());
        println!("space of the first drive: {space:?}");

        let (sender, receiver) = mpsc::channel();
        let platform = Platform::new();
        let notify: Notify = Arc::new(move || {
            let _ = sender.send(());
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(platform.watch(notify)).unwrap();
        println!("watching for 10 seconds: plug in or remove a drive, or map one");
        println!(
            "change seen: {}",
            receiver.recv_timeout(Duration::from_secs(10)).is_ok()
        );
    }
}

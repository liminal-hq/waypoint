// The Win32 calls behind the Windows launcher: random bytes, the user's SID, the two pipes, the runas launch and the checks of who connected
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Each `unsafe` block wraps one Win32 call and says why it is sound. Nothing here logs or reports a path.

use std::ffi::c_void;
use std::fs::File;
use std::io::{self, Read};
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HLOCAL, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
    TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, GetProcessId, OpenProcess,
    OpenProcessToken, QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::IO::CancelSynchronousIo;
use windows::Win32::UI::Shell::{
    FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX64, FOLDERID_ProgramFilesX86,
    SHGetKnownFolderPath, ShellExecuteExW, KF_FLAG_DEFAULT, SEE_MASK_NOASYNC,
    SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

use crate::args::helper_arguments;
use crate::launch::{map_shell_error, ElevatedStream, LaunchError};
use crate::location::same_path;
use crate::pipe_names::{is_handshake, max_handshake_len, pipe_names, token, Rng};
use crate::sddl::pipe_dacl;
use crate::Config;

/// How often a wait checks whether the caller gave up and whether the helper ended.
const POLL: Duration = Duration::from_millis(50);
/// How long the helper has, once the person has consented, to connect to both pipes and present the token.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// How long an abandoned wait keeps trying to interrupt the thread that is blocked in a pipe call.
const ABORT_TIMEOUT: Duration = Duration::from_secs(1);
/// The size of each pipe's buffer.
const PIPE_BUFFER: u32 = 64 * 1024;
/// `ERROR_PIPE_CONNECTED`: the client connected between the pipe's creation and `ConnectNamedPipe`.
const ERROR_PIPE_CONNECTED: i32 = 535;
/// `ERROR_OPERATION_ABORTED`: the blocked call was interrupted by `CancelSynchronousIo`.
const ERROR_OPERATION_ABORTED: i32 = 995;
/// `FILE_ATTRIBUTE_REPARSE_POINT`.
const REPARSE_POINT: u32 = 0x400;

fn io_error(error: &io::Error) -> LaunchError {
    LaunchError::Io { kind: error.kind() }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A Win32 handle that is closed when dropped.
pub struct Owned(HANDLE);

// SAFETY: a kernel handle is a process-wide reference; any thread may use or close it.
unsafe impl Send for Owned {}
// SAFETY: as above; the calls made through `&Owned` (wait, query) are thread-safe.
unsafe impl Sync for Owned {}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle is owned by this value and closed exactly once, here.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Random bytes from the operating system's preferred generator.
pub struct OsRng;

impl Rng for OsRng {
    fn fill(&mut self, buf: &mut [u8]) -> io::Result<()> {
        // SAFETY: `buf` is a valid writable slice; the system-preferred generator takes no algorithm handle.
        let status = unsafe { BCryptGenRandom(None, buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
        if status.is_ok() {
            Ok(())
        } else {
            Err(io::Error::other("no random bytes"))
        }
    }
}

/// The current process's access token, for reading.
fn process_token() -> io::Result<Owned> {
    let mut handle = HANDLE::default();
    // SAFETY: the pseudo handle from `GetCurrentProcess` needs no closing, and `handle` is a valid out pointer.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) }
        .map_err(|_| io::Error::last_os_error())?;
    Ok(Owned(handle))
}

/// Reads one kind of token information into a buffer of 8-byte units, so that the structures in it are aligned.
fn token_information(
    token: &Owned,
    class: windows::Win32::Security::TOKEN_INFORMATION_CLASS,
) -> io::Result<Vec<u64>> {
    let mut length = 0u32;
    // SAFETY: a null buffer of length zero asks only for the needed length.
    let _ = unsafe { GetTokenInformation(token.0, class, None, 0, &mut length) };
    if length == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
    // SAFETY: the buffer is at least `length` bytes and writable.
    unsafe {
        GetTokenInformation(
            token.0,
            class,
            Some(buffer.as_mut_ptr().cast::<c_void>()),
            length,
            &mut length,
        )
    }
    .map_err(|_| io::Error::last_os_error())?;
    Ok(buffer)
}

/// The SID of the user this process runs as, in string form (`S-1-5-21-...`).
pub fn current_user_sid_string() -> io::Result<String> {
    let token = process_token()?;
    let buffer = token_information(&token, TokenUser)?;
    // SAFETY: `GetTokenInformation(TokenUser)` filled the aligned buffer with a `TOKEN_USER` whose SID points into the same buffer.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut text = PWSTR::null();
    // SAFETY: the SID is valid for the buffer's life; `text` receives a `LocalAlloc`ed string.
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) }
        .map_err(|_| io::Error::last_os_error())?;
    // SAFETY: on success `text` is a valid null-terminated wide string.
    let sid = unsafe { text.to_string() };
    // SAFETY: the string was allocated by `ConvertSidToStringSidW` for the caller to free with `LocalFree`.
    unsafe {
        let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(text.0.cast())));
    }
    sid.map_err(|_| io::Error::other("bad SID text"))
}

/// Whether this process runs with an elevated token, when that can be read.
pub fn is_elevated() -> Option<bool> {
    let token = process_token().ok()?;
    let buffer = token_information(&token, TokenElevation).ok()?;
    // SAFETY: `GetTokenInformation(TokenElevation)` filled the aligned buffer with a `TOKEN_ELEVATION`.
    let elevation = unsafe { &*buffer.as_ptr().cast::<TOKEN_ELEVATION>() };
    Some(elevation.TokenIsElevated != 0)
}

/// The Program Files folders Windows reports, as text: the native one and the 32-bit and 64-bit ones where they exist.
pub fn program_files() -> Vec<String> {
    let mut folders = Vec::new();
    for id in [
        &FOLDERID_ProgramFiles,
        &FOLDERID_ProgramFilesX86,
        &FOLDERID_ProgramFilesX64,
    ] {
        // SAFETY: `id` is a valid folder id; the returned string is freed below with `CoTaskMemFree`.
        if let Ok(path) = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) } {
            // SAFETY: on success `path` is a valid null-terminated wide string owned by the caller.
            if let Ok(text) = unsafe { path.to_string() } {
                if !folders.contains(&text) {
                    folders.push(text);
                }
            }
            // SAFETY: the string came from the shell's allocator, which `CoTaskMemFree` releases.
            unsafe { CoTaskMemFree(Some(path.0.cast())) };
        }
    }
    folders
}

/// Whether a path is a link or other reparse point, read from its attributes without following it.
pub fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & REPARSE_POINT != 0
}

enum Direction {
    /// The app writes, the helper reads.
    Outbound,
    /// The helper writes, the app reads.
    Inbound,
}

/// Creates one byte-mode pipe with the one instance its name allows: the first instance of that name (nobody else can have made it), closed to remote clients, and open only as `dacl` says. The handle is not inheritable.
fn create_pipe(name: &str, direction: Direction, dacl: &str) -> io::Result<File> {
    let dacl_text = wide(dacl);
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: `dacl_text` is a null-terminated wide string; `descriptor` receives a `LocalAlloc`ed descriptor freed below.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(dacl_text.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(|_| io::Error::last_os_error())?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let access = match direction {
        Direction::Outbound => PIPE_ACCESS_OUTBOUND,
        Direction::Inbound => PIPE_ACCESS_INBOUND,
    };
    let name = wide(name);
    // SAFETY: the name is null-terminated, and `attributes` and the descriptor it points to outlive the call.
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            access | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            PIPE_BUFFER,
            PIPE_BUFFER,
            0,
            Some(&attributes),
        )
    };
    let failure = io::Error::last_os_error();
    // SAFETY: the descriptor was allocated by the conversion for the caller to free with `LocalFree`, and the pipe keeps its own copy.
    unsafe {
        let _ = windows::Win32::Foundation::LocalFree(Some(HLOCAL(descriptor.0)));
    }
    if handle == INVALID_HANDLE_VALUE {
        return Err(failure);
    }
    // SAFETY: the handle is a new pipe handle that nothing else owns; the `File` closes it.
    Ok(unsafe { File::from_raw_handle(handle.0 as RawHandle) })
}

fn raw(file: &File) -> HANDLE {
    HANDLE(file.as_raw_handle())
}

/// Waits for a client to connect; blocks until one does or `CancelSynchronousIo` interrupts it.
fn connect(pipe: &File) -> io::Result<()> {
    // SAFETY: the handle is a valid pipe server handle owned by `pipe`; no overlapped structure is used.
    match unsafe { ConnectNamedPipe(raw(pipe), None) } {
        Ok(()) => Ok(()),
        Err(_) => {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_PIPE_CONNECTED) {
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

fn disconnect(pipe: &File) {
    // SAFETY: the handle is a valid pipe server handle owned by `pipe`; a failure changes nothing, because the pipe is closed next.
    unsafe {
        let _ = DisconnectNamedPipe(raw(pipe));
    }
}

fn client_pid(pipe: &File) -> io::Result<u32> {
    let mut pid = 0u32;
    // SAFETY: the handle is a valid pipe server handle and `pid` a valid out pointer.
    unsafe { GetNamedPipeClientProcessId(raw(pipe), &mut pid) }
        .map_err(|_| io::Error::last_os_error())?;
    Ok(pid)
}

/// The full path of the program a process was started from.
fn image_path(process: HANDLE) -> io::Result<String> {
    let mut buffer = vec![0u16; 32 * 1024];
    let mut length = buffer.len() as u32;
    // SAFETY: the buffer is writable for `length` characters, and `length` is updated to the characters written.
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    }
    .map_err(|_| io::Error::last_os_error())?;
    Ok(String::from_utf16_lossy(&buffer[..length as usize]))
}

/// The image path of process `pid`, from the launch's own handle when it can be queried and otherwise from a handle opened by id.
fn process_image_path(process: &Owned, pid: u32) -> io::Result<String> {
    if let Ok(path) = image_path(process.0) {
        return Ok(path);
    }
    // SAFETY: opens a handle for query only; it is closed below.
    let opened = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map_err(|_| io::Error::last_os_error())?;
    let opened = Owned(opened);
    image_path(opened.0)
}

/// True when the process on the other end of `pipe` is the one the launch started, and was started from the helper program.
fn client_is_the_helper(pipe: &File, process: &Owned, helper: &Path) -> bool {
    let Ok(pid) = client_pid(pipe) else {
        return false;
    };
    // SAFETY: the handle is a valid process handle owned by `process`.
    let launched = unsafe { GetProcessId(process.0) };
    if launched == 0 || launched != pid {
        return false;
    }
    match process_image_path(process, pid) {
        Ok(path) => same_path(&path, &helper.to_string_lossy()),
        Err(_) => false,
    }
}

/// Reads the helper's first line from the pipe, a byte at a time so that nothing past it is consumed, and compares it with the ready line and the token.
fn read_handshake(pipe: &File, ready_line: &str, token: &str) -> bool {
    let limit = max_handshake_len(ready_line);
    let mut line = Vec::with_capacity(limit);
    let mut reader = pipe;
    let mut byte = [0u8; 1];
    while line.len() < limit {
        match reader.read(&mut byte) {
            Ok(1) => {
                if byte[0] == b'\n' {
                    return is_handshake(&line, ready_line, token);
                }
                line.push(byte[0]);
            }
            _ => return false,
        }
    }
    false
}

fn exit_code(process: &Owned) -> Option<u32> {
    // SAFETY: the handle is a valid process handle; waiting with a zero timeout only tests whether it has ended.
    if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_OBJECT_0 {
        return None;
    }
    let mut code = 0u32;
    // SAFETY: the handle is valid and `code` is a valid out pointer.
    unsafe { GetExitCodeProcess(process.0, &mut code) }.ok()?;
    Some(code)
}

/// `ShellExecuteExW` with the `runas` verb. It blocks while the UAC prompt is open, so it runs on its own thread. `Ok(None)` is a launch that gave no process handle.
fn shell_execute(file: &str, parameters: &str, directory: &str) -> Result<Option<Owned>, u32> {
    // SAFETY: initialises COM for this thread, which the shell's verb handling may use; undone below when it succeeded.
    let com = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let verb = wide("runas");
    let file = wide(file);
    let parameters = wide(parameters);
    let directory = wide(directory);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        lpDirectory: PCWSTR(directory.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised and every string it points to outlives the call.
    let result = unsafe { ShellExecuteExW(&mut info) };
    let outcome = match result {
        Ok(()) if info.hProcess.is_invalid() || info.hProcess.0.is_null() => Ok(None),
        Ok(()) => Ok(Some(Owned(info.hProcess))),
        Err(_) => Err(io::Error::last_os_error().raw_os_error().unwrap_or(0) as u32),
    };
    if com {
        // SAFETY: balances the successful `CoInitializeEx` above, on the same thread.
        unsafe { CoUninitialize() };
    }
    outcome
}

/// A thread that is blocked in a pipe call, and the means to interrupt it.
struct Worker {
    thread: std::thread::JoinHandle<()>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    /// Asks the thread to stop and interrupts the pipe call it may be blocked in, again and again until it ends: an interrupt that arrives before the call starts does nothing, and the stop flag catches that case.
    fn abort(self) {
        self.stop.store(true, Ordering::SeqCst);
        let started = Instant::now();
        while !self.thread.is_finished() && started.elapsed() < ABORT_TIMEOUT {
            // SAFETY: the handle is the live thread's own, valid while `self.thread` exists.
            unsafe {
                let _ = CancelSynchronousIo(HANDLE(self.thread.as_raw_handle()));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

/// Waits for the helper on both pipes, checks who it is and reads its handshake. Runs on a worker thread.
fn establish(
    to: File,
    from: File,
    process: &Owned,
    helper: &Path,
    ready_line: &str,
    token: &str,
    stop: &AtomicBool,
) -> Result<(File, File), LaunchError> {
    for pipe in [&to, &from] {
        if stop.load(Ordering::SeqCst) {
            return Err(LaunchError::Cancelled);
        }
        connect(pipe).map_err(|error| {
            if error.raw_os_error() == Some(ERROR_OPERATION_ABORTED) {
                LaunchError::Cancelled
            } else {
                io_error(&error)
            }
        })?;
    }
    let trusted = client_is_the_helper(&to, process, helper)
        && client_is_the_helper(&from, process, helper)
        && read_handshake(&from, ready_line, token);
    if !trusted {
        disconnect(&to);
        disconnect(&from);
        return Err(if stop.load(Ordering::SeqCst) {
            LaunchError::Cancelled
        } else {
            LaunchError::Failed { code: None }
        });
    }
    Ok((to, from))
}

/// Starts the helper through UAC and returns the stream to it. See the module's plugin documentation for the checks that make the stream trusted.
pub fn launch(
    config: &Config,
    cancelled: &dyn Fn() -> bool,
) -> Result<ElevatedStream, LaunchError> {
    let Some(prefix) = config.pipe_prefix.as_deref() else {
        return Err(LaunchError::Unavailable {
            reason: crate::win_check::REASON_NOT_CONFIGURED.to_string(),
        });
    };
    let mut rng = OsRng;
    let names = pipe_names(prefix, &mut rng).map_err(|error| io_error(&error))?;
    let token = token(&mut rng).map_err(|error| io_error(&error))?;
    let sid = current_user_sid_string().map_err(|error| io_error(&error))?;
    let dacl = pipe_dacl(&sid).ok_or(LaunchError::Failed { code: None })?;
    let to =
        create_pipe(&names.to, Direction::Outbound, &dacl).map_err(|error| io_error(&error))?;
    let from =
        create_pipe(&names.from, Direction::Inbound, &dacl).map_err(|error| io_error(&error))?;

    let helper = config.helper.clone();
    let directory = helper
        .parent()
        .map(|folder| folder.to_string_lossy().into_owned())
        .unwrap_or_default();
    // SAFETY: reads this process's own id.
    let parameters = helper_arguments(&names, &token, unsafe { GetCurrentProcessId() });

    // The prompt blocks the call for as long as the person likes, so it gets a thread. If this wait is abandoned the thread still ends when the prompt does, and closes the handle it gets.
    let (shell_result, shell_inbox) = mpsc::channel();
    let file = helper.to_string_lossy().into_owned();
    std::thread::Builder::new()
        .name("elevate-runas".into())
        .spawn(move || {
            let _ = shell_result.send(shell_execute(&file, &parameters, &directory));
        })
        .map_err(|error| io_error(&error))?;

    let process = loop {
        if cancelled() {
            return Err(LaunchError::Cancelled);
        }
        match shell_inbox.recv_timeout(POLL) {
            Ok(Ok(Some(process))) => break Arc::new(process),
            // No process to check the connection against: fail closed.
            Ok(Ok(None)) => return Err(LaunchError::Failed { code: None }),
            Ok(Err(code)) => return Err(map_shell_error(code)),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(LaunchError::Failed { code: None })
            }
        }
    };

    // Consent has been given: from here the helper has a bounded time to connect.
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let stop = Arc::new(AtomicBool::new(false));
    let (established, inbox) = mpsc::channel();
    let thread = {
        let process = Arc::clone(&process);
        let stop = Arc::clone(&stop);
        let ready_line = config.ready_line.clone();
        std::thread::Builder::new()
            .name("elevate-connect".into())
            .spawn(move || {
                let result = establish(to, from, &process, &helper, &ready_line, &token, &stop);
                let _ = established.send(result);
            })
            .map_err(|error| io_error(&error))?
    };
    let worker = Worker { thread, stop };

    let result = loop {
        if cancelled() {
            worker.abort();
            return Err(LaunchError::Cancelled);
        }
        match inbox.recv_timeout(POLL) {
            Ok(result) => break result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(LaunchError::Failed { code: None })
            }
        }
        if let Some(code) = exit_code(&process) {
            worker.abort();
            return Err(LaunchError::Failed {
                code: i32::try_from(code).ok(),
            });
        }
        if Instant::now() >= deadline {
            worker.abort();
            return Err(LaunchError::Failed { code: None });
        }
    };
    let (to, from) = result?;
    Ok(ElevatedStream::from_parts(
        Box::new(from),
        Box::new(to),
        Box::new(process),
    ))
}

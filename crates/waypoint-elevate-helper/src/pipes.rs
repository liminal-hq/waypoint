// Opens the two pipes the app made, checks who made them and says the helper is running, for the Windows launch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;

use crate::cli::PipeArgs;

/// How long the helper keeps trying to open the pipes: the app makes them before it starts the helper, so this only covers a slow start, and an app that gave up (and closed them) is not waited for longer.
const CONNECT_WINDOW: Duration = Duration::from_secs(15);
const RETRY: Duration = Duration::from_millis(100);
/// `ERROR_FILE_NOT_FOUND` and `ERROR_PIPE_BUSY`: the pipe is not there (yet) or is taken for the moment.
const NOT_THERE: [i32; 2] = [2, 231];

/// Why the helper could not get its pipes. No variant carries a name or a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeError {
    /// The pipes did not appear in time, or could not be opened.
    Connect,
    /// The pipes were made by a process other than the one named on the command line.
    WrongServer,
    /// The handshake could not be written.
    Handshake,
}

/// Opens a pipe by name, retrying while it is not there yet, until the window closes.
fn open(name: &str, read: bool, started: Instant) -> Result<File, PipeError> {
    loop {
        let mut options = OpenOptions::new();
        if read {
            options.read(true);
        } else {
            options.write(true);
        }
        match options.open(name) {
            Ok(file) => return Ok(file),
            Err(error)
                if error
                    .raw_os_error()
                    .is_some_and(|code| NOT_THERE.contains(&code))
                    && started.elapsed() < CONNECT_WINDOW =>
            {
                std::thread::sleep(RETRY);
            }
            Err(_) => return Err(PipeError::Connect),
        }
    }
}

fn server_pid(pipe: &File) -> io::Result<u32> {
    let mut pid = 0u32;
    // SAFETY: the handle is a valid pipe client handle owned by `pipe`, and `pid` a valid out pointer.
    unsafe { GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut pid) }
        .map_err(|_| io::Error::last_os_error())?;
    Ok(pid)
}

/// Opens both pipes (reading `to`, writing `from`), checks that the app named by `parent` made them, and writes the handshake line `<ready line> <token>`. Returns what to read requests from and write responses to.
pub fn connect(args: &PipeArgs, ready_line: &str) -> Result<(File, File), PipeError> {
    let started = Instant::now();
    let reader = open(&args.to, true, started)?;
    let mut writer = open(&args.from, false, started)?;
    for pipe in [&reader, &writer] {
        match server_pid(pipe) {
            Ok(pid) if pid == args.parent => {}
            _ => return Err(PipeError::WrongServer),
        }
    }
    writeln!(writer, "{ready_line} {}", args.token).map_err(|_| PipeError::Handshake)?;
    writer.flush().map_err(|_| PipeError::Handshake)?;
    Ok((reader, writer))
}

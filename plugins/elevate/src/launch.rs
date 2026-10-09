// Starts the helper process and waits until it says it is running, however long the system's prompt takes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How often a wait for the helper checks whether the caller gave up.
const POLL: Duration = Duration::from_millis(50);
/// How long to wait for the exit status once the helper's error stream has closed.
const EXIT_GRACE: Duration = Duration::from_secs(2);
/// The longest line of the helper's error stream that is kept to be compared with the ready line.
const MAX_LINE: usize = 4096;

/// `pkexec`'s exit status when the person dismissed the authentication dialog.
pub const EXIT_DISMISSED: i32 = 126;
/// `pkexec`'s exit status when the person was not authorised, authentication failed, or `pkexec` itself failed.
pub const EXIT_NOT_AUTHORISED: i32 = 127;

/// Why the helper could not be started. No message names a path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    /// Elevation cannot work on this system; the reason is for the person.
    #[error("{reason}")]
    Unavailable { reason: String },
    /// The person could not be authenticated, or is not allowed to act as an administrator.
    #[error("Not authorised to act as an administrator")]
    NotAuthorised,
    /// The person dismissed the authentication dialog.
    #[error("The authentication dialog was dismissed")]
    Dismissed,
    /// The caller stopped waiting before the helper was running.
    #[error("Cancelled before the helper started")]
    Cancelled,
    /// The helper ended before it was running, for some other reason. `code` is its exit status, absent when a signal ended it or it closed its output and gave none.
    #[error("The administrator helper failed to start{}", code.map(|code| format!(" (exit status {code})")).unwrap_or_default())]
    Failed { code: Option<i32> },
    /// The process could not be started or watched.
    #[error("Could not start the administrator helper ({kind})")]
    Io { kind: io::ErrorKind },
}

impl LaunchError {
    fn io(error: &io::Error) -> Self {
        LaunchError::Io { kind: error.kind() }
    }
}

/// Maps the exit status of a helper that ended before it was running to the reason. `pkexec` gives 126 for a dismissed dialog and 127 for not authorised (or its own failure); anything else, or no status, is a helper failure.
pub fn map_exit(code: Option<i32>) -> LaunchError {
    match code {
        Some(EXIT_DISMISSED) => LaunchError::Dismissed,
        Some(EXIT_NOT_AUTHORISED) => LaunchError::NotAuthorised,
        code => LaunchError::Failed { code },
    }
}

/// Windows' `ERROR_CANCELLED`: the person declined the UAC prompt.
pub const ERROR_CANCELLED: u32 = 1223;

/// Maps the error code of a failed `ShellExecuteExW` to the reason: a declined prompt is a dismissal, anything else is an I/O failure that names no path.
pub fn map_shell_error(code: u32) -> LaunchError {
    if code == ERROR_CANCELLED {
        LaunchError::Dismissed
    } else {
        LaunchError::Io {
            kind: io::Error::from_raw_os_error(code as i32).kind(),
        }
    }
}

/// True when `line` (without its line ending) is the ready line.
pub fn is_ready_line(line: &[u8], ready_line: &str) -> bool {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    line == ready_line.as_bytes()
}

/// A process that was started: its three pipes and its life.
pub trait Spawned: Send {
    fn take_stdin(&mut self) -> Option<Box<dyn Write + Send>>;
    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>>;
    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>>;
    /// The exit status if the process has ended: `Some(None)` when a signal ended it.
    fn try_wait(&mut self) -> io::Result<Option<Option<i32>>>;
    /// Asks the process to end, as far as this user is allowed to. Never blocks, and a failure is not reported: once `pkexec` has handed over to the helper the process belongs to root and cannot be signalled, which is why closing its standard input is what ends it.
    fn kill(&mut self);
    /// Waits for the process to end, to release it.
    fn wait(&mut self) -> io::Result<Option<i32>>;
}

/// Starts the process. `pkexec <helper>` in production; a script in tests.
pub trait Spawner: Send + Sync {
    fn spawn(&self) -> io::Result<Box<dyn Spawned>>;
}

/// Starts a program with piped standard streams.
pub struct CommandSpawner {
    pub program: PathBuf,
    pub args: Vec<String>,
}

struct ChildProcess(Child);

impl Spawner for CommandSpawner {
    fn spawn(&self) -> io::Result<Box<dyn Spawned>> {
        let child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Ok(Box::new(ChildProcess(child)))
    }
}

impl Spawned for ChildProcess {
    fn take_stdin(&mut self) -> Option<Box<dyn Write + Send>> {
        self.0.stdin.take().map(|pipe| Box::new(pipe) as _)
    }
    fn take_stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.0.stdout.take().map(|pipe| Box::new(pipe) as _)
    }
    fn take_stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        self.0.stderr.take().map(|pipe| Box::new(pipe) as _)
    }
    fn try_wait(&mut self) -> io::Result<Option<Option<i32>>> {
        Ok(self.0.try_wait()?.map(|status| status.code()))
    }
    fn kill(&mut self) {
        let _ = self.0.kill();
    }
    fn wait(&mut self) -> io::Result<Option<i32>> {
        Ok(self.0.wait()?.code())
    }
}

/// Ends a process when dropped: signals it if it still can be, then reaps it on a thread, so dropping never blocks on a helper that is still shutting down.
struct ProcessGuard(Option<Box<dyn Spawned>>);

impl ProcessGuard {
    fn try_wait(&mut self) -> io::Result<Option<Option<i32>>> {
        match self.0.as_mut() {
            Some(process) => process.try_wait(),
            None => Ok(Some(None)),
        }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if let Some(mut process) = self.0.take() {
            process.kill();
            let _ = std::thread::Builder::new()
                .name("elevate-reap".into())
                .spawn(move || {
                    let _ = process.wait();
                });
        }
    }
}

/// A running helper: the pipes to speak the helper's protocol over, and what keeps the process they belong to. Dropping it closes the pipes first, which ends the helper, then releases the process.
pub struct ElevatedStream {
    /// What the helper writes: its standard output on Linux, a pipe on Windows.
    pub reader: Box<dyn Read + Send>,
    /// What the helper reads: its standard input on Linux, a pipe on Windows.
    pub writer: Box<dyn Write + Send>,
    // Declared last so the pipes close before the process is signalled or released.
    _keeper: Box<dyn Send>,
}

impl ElevatedStream {
    /// A stream over two pipes and a `keeper` that is dropped after them: it holds the process, and releases it when dropped.
    pub fn from_parts(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        keeper: Box<dyn Send>,
    ) -> Self {
        ElevatedStream {
            reader,
            writer,
            _keeper: keeper,
        }
    }
}

enum Event {
    Ready,
    Closed,
}

/// Reads the helper's error stream: sends `Ready` at the first line equal to `ready_line`, `Closed` at the end of the stream, and then keeps draining so the helper never blocks on a full pipe. Nothing read is kept or logged; before the ready line it is the prompt program's own words, which can name paths.
fn watch_stderr(mut stderr: Box<dyn Read + Send>, ready_line: String, events: mpsc::Sender<Event>) {
    let mut line: Vec<u8> = Vec::new();
    let mut overlong = false;
    let mut ready = false;
    let mut chunk = [0u8; 512];
    loop {
        let read = match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        if ready {
            continue;
        }
        for &byte in &chunk[..read] {
            if byte == b'\n' {
                if !overlong && is_ready_line(&line, &ready_line) {
                    ready = true;
                    let _ = events.send(Event::Ready);
                    break;
                }
                line.clear();
                overlong = false;
            } else if line.len() < MAX_LINE {
                line.push(byte);
            } else {
                overlong = true;
            }
        }
    }
    let _ = events.send(Event::Closed);
}

/// Starts the process and blocks until the helper is really running: until it writes `ready_line` to its error stream, until it ends (mapped through [`map_exit`]), or until `cancelled` returns true (the process is signalled and its pipes closed, and the result is [`LaunchError::Cancelled`]). There is no time limit, because the system's prompt may wait for the person as long as they like; a caller's own timeout for the protocol should start only when this returns.
pub fn launch_with(
    spawner: &dyn Spawner,
    ready_line: &str,
    cancelled: &dyn Fn() -> bool,
) -> Result<ElevatedStream, LaunchError> {
    let mut process = spawner.spawn().map_err(|error| LaunchError::io(&error))?;
    let (Some(writer), Some(reader), Some(stderr)) = (
        process.take_stdin(),
        process.take_stdout(),
        process.take_stderr(),
    ) else {
        let _guard = ProcessGuard(Some(process));
        return Err(LaunchError::Io {
            kind: io::ErrorKind::BrokenPipe,
        });
    };
    let mut guard = ProcessGuard(Some(process));

    let (events, inbox) = mpsc::channel();
    let ready_line_owned = ready_line.to_string();
    std::thread::Builder::new()
        .name("elevate-stderr".into())
        .spawn(move || watch_stderr(stderr, ready_line_owned, events))
        .map_err(|error| LaunchError::io(&error))?;

    let mut closed_at: Option<Instant> = None;
    loop {
        if cancelled() {
            // Closing the pipes ends a helper that `kill` cannot reach; the guard signals the rest.
            drop((writer, reader));
            return Err(LaunchError::Cancelled);
        }
        match inbox.recv_timeout(POLL) {
            Ok(Event::Ready) => {
                return Ok(ElevatedStream::from_parts(reader, writer, Box::new(guard)));
            }
            Ok(Event::Closed) | Err(RecvTimeoutError::Disconnected) => {
                closed_at.get_or_insert_with(Instant::now);
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        // A ready line that arrived together with the end of the process still counts.
        let status = guard.try_wait().map_err(|error| LaunchError::io(&error))?;
        if let Some(code) = status {
            if let Ok(Event::Ready) = inbox.try_recv() {
                return Ok(ElevatedStream::from_parts(reader, writer, Box::new(guard)));
            }
            return Err(map_exit(code));
        }
        if closed_at.is_some_and(|at| at.elapsed() >= EXIT_GRACE) {
            // The stream closed but the process lives on without saying it is ready.
            drop((writer, reader));
            return Err(LaunchError::Failed { code: None });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkexec_exit_codes_map_to_their_reasons() {
        assert_eq!(map_exit(Some(126)), LaunchError::Dismissed);
        assert_eq!(map_exit(Some(127)), LaunchError::NotAuthorised);
        assert_eq!(map_exit(Some(3)), LaunchError::Failed { code: Some(3) });
        assert_eq!(map_exit(Some(0)), LaunchError::Failed { code: Some(0) });
        assert_eq!(map_exit(None), LaunchError::Failed { code: None });
    }

    #[test]
    fn a_declined_uac_prompt_is_a_dismissal_and_other_codes_are_io() {
        assert_eq!(map_shell_error(1223), LaunchError::Dismissed);
        assert!(matches!(map_shell_error(5), LaunchError::Io { .. }));
        assert!(matches!(map_shell_error(2), LaunchError::Io { .. }));
    }

    #[test]
    fn the_ready_line_is_matched_exactly_up_to_its_line_ending() {
        assert!(is_ready_line(b"helper ready", "helper ready"));
        assert!(is_ready_line(b"helper ready\n", "helper ready"));
        assert!(is_ready_line(b"helper ready\r\n", "helper ready"));
        assert!(!is_ready_line(b"helper ready ", "helper ready"));
        assert!(!is_ready_line(b" helper ready", "helper ready"));
        assert!(!is_ready_line(b"not helper ready", "helper ready"));
        assert!(!is_ready_line(b"helper", "helper ready"));
        assert!(!is_ready_line(b"", "helper ready"));
    }

    #[test]
    fn no_error_message_names_a_path() {
        let errors = [
            LaunchError::Unavailable { reason: "x".into() },
            LaunchError::NotAuthorised,
            LaunchError::Dismissed,
            LaunchError::Cancelled,
            LaunchError::Failed { code: Some(3) },
            LaunchError::Failed { code: None },
            LaunchError::Io {
                kind: io::ErrorKind::NotFound,
            },
        ];
        for error in errors {
            assert!(!error.to_string().contains('/'), "{error}");
        }
    }
}

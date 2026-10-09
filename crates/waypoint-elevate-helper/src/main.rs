// The privileged helper of Open as Administrator: serves file operations until its input ends, over
// its standard input and output (started by `pkexec` on Linux) or over two named pipes (started
// through UAC on Windows). It has no privilege logic of its own; whatever starts it decides who it
// runs as.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod cli;
#[cfg(windows)]
mod pipes;

use std::io::{stderr, stdin, stdout, Write};
use std::process::ExitCode;
use std::sync::Arc;

use waypoint_elevated::{serve, ServeConfig, ServeEnd, READY_LINE};
use waypoint_vfs::LocalProvider;

/// The input ended or the helper sat idle: the ordinary ways to finish.
const EXIT_OK: u8 = 0;
/// The client broke the protocol.
const EXIT_PROTOCOL: u8 = 3;
/// The output failed.
const EXIT_OUTPUT: u8 = 4;
/// A thread could not be started.
const EXIT_START: u8 = 5;
/// The command line was not understood, or asked for pipes where there are none.
const EXIT_USAGE: u8 = 2;
/// The pipes did not appear in time, or could not be opened.
#[cfg(windows)]
const EXIT_PIPES: u8 = 6;
/// The pipes were made by a process other than the app that started the helper.
#[cfg(windows)]
const EXIT_WRONG_SERVER: u8 = 7;
/// The handshake could not be written.
#[cfg(windows)]
const EXIT_HANDSHAKE: u8 = 8;

fn main() -> ExitCode {
    match cli::parse(std::env::args().skip(1)) {
        Ok(cli::Mode::Stdio) => {
            // The first thing the helper does is say it is running, so whatever started it (and
            // waited through a prompt) knows the connection can begin. A failed write is not
            // fatal: the reader may be gone, and then serving ends on its own.
            announce_ready();
            // Nothing is written to the output but protocol frames, and nothing to the error
            // stream but the ready line and the reason for a fatal end, which never names a path.
            run(stdin(), stdout())
        }
        Ok(cli::Mode::Pipes(args)) => run_pipes(&args),
        Err(cli::UsageError) => {
            eprintln!("waypoint-elevate-helper: unrecognised arguments");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Over the two pipes the app made: the handshake (the ready line and the token) is the first
/// thing written to the output, where standard error has no reader.
#[cfg(windows)]
fn run_pipes(args: &cli::PipeArgs) -> ExitCode {
    match pipes::connect(args, READY_LINE) {
        Ok((reader, writer)) => run(reader, writer),
        Err(error) => {
            eprintln!("waypoint-elevate-helper: could not connect ({error:?})");
            ExitCode::from(match error {
                pipes::PipeError::Connect => EXIT_PIPES,
                pipes::PipeError::WrongServer => EXIT_WRONG_SERVER,
                pipes::PipeError::Handshake => EXIT_HANDSHAKE,
            })
        }
    }
}

/// Pipes exist only where the Windows launcher does.
#[cfg(not(windows))]
fn run_pipes(_args: &cli::PipeArgs) -> ExitCode {
    eprintln!("waypoint-elevate-helper: pipes are not supported on this platform");
    ExitCode::from(EXIT_USAGE)
}

fn run(
    input: impl std::io::Read + Send + 'static,
    output: impl std::io::Write + Send + 'static,
) -> ExitCode {
    let end = serve(
        input,
        output,
        Arc::new(LocalProvider::new()),
        ServeConfig::default(),
    );
    match end {
        ServeEnd::EndOfInput | ServeEnd::Idle => ExitCode::from(EXIT_OK),
        ServeEnd::ProtocolError(_) => fatal(&end, EXIT_PROTOCOL),
        ServeEnd::WriteFailed => fatal(&end, EXIT_OUTPUT),
        ServeEnd::StartFailed => fatal(&end, EXIT_START),
    }
}

fn announce_ready() {
    let mut error = stderr().lock();
    let _ = writeln!(error, "{READY_LINE}");
    let _ = error.flush();
}

fn fatal(end: &ServeEnd, code: u8) -> ExitCode {
    eprintln!("waypoint-elevate-helper: {end}");
    ExitCode::from(code)
}

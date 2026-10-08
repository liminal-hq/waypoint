// The privileged helper of Open as Administrator: serves file operations over its standard input and
// output until the input ends. It has no privilege logic of its own; whatever starts it decides
// who it runs as.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

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

fn main() -> ExitCode {
    // The first thing the helper does is say it is running, so whatever started it (and waited
    // through a prompt) knows the connection can begin. A failed write is not fatal: the reader
    // may be gone, and then serving ends on its own.
    announce_ready();
    // Nothing is written to the output but protocol frames, and nothing to the error stream but
    // the ready line and the reason for a fatal end, which never names a path.
    let end = serve(
        stdin(),
        stdout(),
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

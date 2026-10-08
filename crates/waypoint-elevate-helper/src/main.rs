// The privileged helper of Open as Administrator: serves file operations over its standard input and
// output until the input ends. It has no privilege logic of its own; whatever starts it decides
// who it runs as.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{stdin, stdout};
use std::process::ExitCode;
use std::sync::Arc;

use waypoint_elevated::{serve, ServeConfig, ServeEnd};
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
    // Nothing is written to the output but protocol frames, and nothing to the error stream but
    // the reason for a fatal end, which never names a path.
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

fn fatal(end: &ServeEnd, code: u8) -> ExitCode {
    eprintln!("waypoint-elevate-helper: {end}");
    ExitCode::from(code)
}

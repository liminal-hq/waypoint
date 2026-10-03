// Runs an external thumbnailer with a timeout and a low priority, in its own process group so a cancel or a timeout kills all of it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(unix)]

use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How a run ended other than by success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    /// The program could not be started.
    Spawn(String),
    /// It ran longer than the timeout and was killed.
    TimedOut,
    /// The request was cancelled and the program was killed.
    Cancelled,
    /// It exited with a failure status (or was killed by a signal, which is `None`).
    Exited(Option<i32>),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Spawn(message) => write!(f, "could not start the thumbnailer: {message}"),
            RunError::TimedOut => write!(f, "the thumbnailer timed out"),
            RunError::Cancelled => write!(f, "cancelled"),
            RunError::Exited(Some(code)) => write!(f, "the thumbnailer exited with status {code}"),
            RunError::Exited(None) => write!(f, "the thumbnailer was killed by a signal"),
        }
    }
}

/// Runs `argv` (a program and its arguments) to completion at a lower priority (`nice` 10), with no input and its output discarded. Polls `cancel` and the `timeout`; either kills the whole process group, so a thumbnailer that started helpers leaves none behind.
pub fn run(argv: &[String], timeout: Duration, cancel: &AtomicBool) -> Result<(), RunError> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| RunError::Spawn("empty command".to_string()))?;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    // SAFETY: `nice` is a plain system call, which is allowed between `fork` and `exec`; its result is deliberately ignored, as a thumbnailer that keeps its priority is still correct.
    unsafe {
        command.pre_exec(|| {
            libc::nice(10);
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|error| RunError::Spawn(error.to_string()))?;
    let deadline = Instant::now() + timeout;
    let mut pause = Duration::from_millis(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // A helper the thumbnailer left running goes with it.
                kill_group(&child);
                return if status.success() {
                    Ok(())
                } else {
                    Err(RunError::Exited(status.code()))
                };
            }
            Ok(None) => {}
            Err(error) => {
                stop(&mut child);
                return Err(RunError::Spawn(error.to_string()));
            }
        }
        if cancel.load(Ordering::Relaxed) {
            stop(&mut child);
            return Err(RunError::Cancelled);
        }
        if Instant::now() >= deadline {
            stop(&mut child);
            return Err(RunError::TimedOut);
        }
        std::thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_millis(20));
    }
}

fn kill_group(child: &Child) {
    // SAFETY: `kill` with a negative process id signals a process group; the group is the child's own (`process_group(0)`), and a stale id only makes the call fail.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
}

fn stop(child: &mut Child) {
    kill_group(child);
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests;

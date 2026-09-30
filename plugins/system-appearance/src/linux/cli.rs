// Runs command-line tools to read a value or stream change notifications
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    thread,
};

use tokio::sync::mpsc::UnboundedSender;

/// Runs `program` and returns its trimmed stdout, or a description of why it failed.
pub fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("{program}: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{program} failed: {}", stderr.trim()))
    }
}

/// A long-running child process that is killed when this guard is dropped.
pub struct Monitor {
    child: Child,
}

impl Monitor {
    /// Spawns `program` and sends a notification for every line it prints.
    pub fn spawn(
        program: &str,
        args: &[&str],
        changed: UnboundedSender<()>,
    ) -> std::io::Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                for _line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if changed.send(()).is_err() {
                        break;
                    }
                }
            });
        }
        Ok(Self { child })
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawns a monitor and boxes it as a watcher guard, logging when the tool is unavailable.
pub fn monitor_guard(
    program: &str,
    args: &[&str],
    changed: UnboundedSender<()>,
) -> Vec<Box<dyn Send>> {
    match Monitor::spawn(program, args, changed) {
        Ok(monitor) => vec![Box::new(monitor)],
        Err(error) => {
            log::warn!("system-appearance: cannot start {program}: {error}");
            Vec::new()
        }
    }
}

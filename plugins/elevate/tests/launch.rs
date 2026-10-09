// Starts fake helpers (shell scripts) through the launch machinery and checks readiness, pkexec's exit codes, cancelling and cleanup
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg(unix)]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri_plugin_elevate::launch::{launch_with, CommandSpawner};
use tauri_plugin_elevate::LaunchError;

/// The line the fake helpers print; the real one is the app's to name.
const READY: &str = "fake-helper ready";

fn shell(script: &str) -> CommandSpawner {
    CommandSpawner {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".into(), script.into()],
    }
}

fn never() -> bool {
    false
}

fn gone(pid: u32) -> bool {
    !PathBuf::from(format!("/proc/{pid}")).exists()
}

fn wait_gone(pid: u32) {
    let started = Instant::now();
    while !gone(pid) {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "process {pid} lived"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn read_pid(file: &std::path::Path) -> u32 {
    let started = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(file) {
            if let Ok(pid) = text.trim().parse() {
                return pid;
            }
        }
        assert!(started.elapsed() < Duration::from_secs(10), "no pid file");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn failure(result: Result<tauri_plugin_elevate::ElevatedStream, LaunchError>) -> LaunchError {
    match result {
        Ok(_) => panic!("the launch should have failed"),
        Err(error) => error,
    }
}

#[test]
fn ready_then_the_streams_carry_data_both_ways() {
    let spawner = shell(&format!("echo noise >&2; echo '{READY}' >&2; exec cat"));
    let mut stream = launch_with(&spawner, READY, &never).unwrap();
    stream.writer.write_all(b"ping").unwrap();
    stream.writer.flush().unwrap();
    let mut back = [0u8; 4];
    stream.reader.read_exact(&mut back).unwrap();
    assert_eq!(&back, b"ping");
}

#[test]
fn a_slow_ready_line_still_succeeds() {
    // A person at a prompt: nothing for longer than any protocol timeout would allow.
    let spawner = shell(&format!("sleep 1; echo '{READY}' >&2; exec cat"));
    let started = Instant::now();
    let stream = launch_with(&spawner, READY, &never);
    assert!(stream.is_ok());
    assert!(started.elapsed() >= Duration::from_secs(1));
}

#[test]
fn the_ready_line_may_arrive_in_pieces() {
    let spawner_split = shell(
        "printf 'fake-helper' >&2; sleep 0.2; printf ' ready' >&2; sleep 0.2; printf '\\n' >&2; exec cat",
    );
    assert!(launch_with(&spawner_split, READY, &never).is_ok());
}

#[test]
fn exit_126_is_a_dismissed_dialog() {
    let spawner = shell("echo 'Request dismissed' >&2; exit 126");
    assert_eq!(
        failure(launch_with(&spawner, READY, &never)),
        LaunchError::Dismissed
    );
}

#[test]
fn exit_127_is_not_authorised() {
    let spawner = shell("echo 'Not authorized' >&2; exit 127");
    assert_eq!(
        failure(launch_with(&spawner, READY, &never)),
        LaunchError::NotAuthorised
    );
}

#[test]
fn any_other_exit_before_ready_is_a_helper_failure() {
    let spawner = shell("exit 3");
    assert_eq!(
        failure(launch_with(&spawner, READY, &never)),
        LaunchError::Failed { code: Some(3) }
    );
    // Words that merely contain the ready line are not it.
    let spawner = shell(&format!("echo 'not {READY}' >&2; exit 5"));
    assert_eq!(
        failure(launch_with(&spawner, READY, &never)),
        LaunchError::Failed { code: Some(5) }
    );
}

#[test]
fn the_end_of_the_error_stream_before_ready_is_a_failure() {
    let spawner = shell("echo hello >&2; exit 0");
    assert_eq!(
        failure(launch_with(&spawner, READY, &never)),
        LaunchError::Failed { code: Some(0) }
    );
    // The stream closes but the process lingers: not waited on for ever.
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let spawner = shell(&format!(
        "echo $$ > '{}'; exec 2>&-; exec sleep 60",
        pid_file.display()
    ));
    let started = Instant::now();
    assert_eq!(
        failure(launch_with(&spawner, READY, &never)),
        LaunchError::Failed { code: None }
    );
    assert!(started.elapsed() < Duration::from_secs(20));
    wait_gone(read_pid(&pid_file));
}

#[test]
fn cancelling_while_waiting_kills_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let spawner = shell(&format!(
        "echo $$ > '{}'; exec sleep 60",
        pid_file.display()
    ));
    let flag = Arc::new(AtomicBool::new(false));
    let setter = {
        let flag = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::SeqCst);
        })
    };
    let started = Instant::now();
    let cancelled = {
        let flag = flag.clone();
        move || flag.load(Ordering::SeqCst)
    };
    assert_eq!(
        failure(launch_with(&spawner, READY, &cancelled)),
        LaunchError::Cancelled
    );
    assert!(started.elapsed() < Duration::from_secs(5));
    setter.join().unwrap();
    wait_gone(read_pid(&pid_file));
}

#[test]
fn cancelling_before_anything_starts_still_cleans_up() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let spawner = shell(&format!(
        "echo $$ > '{}'; exec sleep 60",
        pid_file.display()
    ));
    assert_eq!(
        failure(launch_with(&spawner, READY, &|| true)),
        LaunchError::Cancelled
    );
    // The shell may be killed before it writes its pid; if it got that far, it must be gone.
    std::thread::sleep(Duration::from_millis(300));
    if let Ok(text) = std::fs::read_to_string(&pid_file) {
        if let Ok(pid) = text.trim().parse() {
            wait_gone(pid);
        }
    }
}

#[test]
fn dropping_the_stream_ends_the_helper() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    let spawner = shell(&format!(
        "echo $$ > '{}'; echo '{READY}' >&2; exec cat",
        pid_file.display()
    ));
    let stream = launch_with(&spawner, READY, &never).unwrap();
    let pid = read_pid(&pid_file);
    assert!(!gone(pid));
    drop(stream);
    wait_gone(pid);
}

#[test]
fn a_program_that_cannot_start_is_an_io_error_without_a_path() {
    let spawner = CommandSpawner {
        program: PathBuf::from("/nonexistent/launcher"),
        args: vec![],
    };
    let error = failure(launch_with(&spawner, READY, &never));
    assert_eq!(
        error,
        LaunchError::Io {
            kind: std::io::ErrorKind::NotFound
        }
    );
    assert!(!error.to_string().contains('/'));
}

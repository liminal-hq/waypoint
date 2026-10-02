// Tests the external runner with fake thumbnailer scripts: success, failure, the timeout and the kill
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::*;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn argv(path: &Path, rest: &[&str]) -> Vec<String> {
    std::iter::once(path.to_string_lossy().into_owned())
        .chain(rest.iter().map(|s| s.to_string()))
        .collect()
}

fn alive(pid: i32) -> bool {
    // A zombie is dead for this purpose: it only waits to be reaped.
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .rsplit(')')
            .next()
            .unwrap_or("")
            .trim_start()
            .starts_with('Z'),
        Err(_) => false,
    }
}

fn read_pid(path: &Path) -> i32 {
    for _ in 0..200 {
        if let Ok(text) = fs::read_to_string(path) {
            if let Ok(pid) = text.trim().parse() {
                return pid;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the script never wrote {}", path.display());
}

fn wait_dead(pid: i32) {
    for _ in 0..300 {
        if !alive(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("process {pid} is still alive");
}

#[test]
fn a_thumbnailer_that_succeeds_leaves_its_output() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = script(tmp.path(), "ok", "echo made > \"$1\"");
    let out = tmp.path().join("out.txt");
    run(
        &argv(&tool, &[out.to_str().unwrap()]),
        Duration::from_secs(5),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(fs::read_to_string(out).unwrap().trim(), "made");
}

#[test]
fn a_failing_or_missing_program_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = script(tmp.path(), "bad", "exit 3");
    assert_eq!(
        run(
            &argv(&tool, &[]),
            Duration::from_secs(5),
            &AtomicBool::new(false)
        ),
        Err(RunError::Exited(Some(3)))
    );
    assert!(matches!(
        run(
            &["/no/such/program".to_string()],
            Duration::from_secs(5),
            &AtomicBool::new(false)
        ),
        Err(RunError::Spawn(_))
    ));
    assert!(matches!(
        run(&[], Duration::from_secs(1), &AtomicBool::new(false)),
        Err(RunError::Spawn(_))
    ));
}

#[test]
fn a_thumbnailer_that_sleeps_is_killed_at_the_timeout_with_its_helpers() {
    let tmp = tempfile::tempdir().unwrap();
    let pids = tmp.path().join("pids");
    // A shell that starts a helper and waits for it: both must die.
    let tool = script(
        tmp.path(),
        "slow",
        &format!(
            "echo $$ > {p}.sh\nsleep 30 &\necho $! > {p}.sleep\nwait",
            p = pids.display()
        ),
    );
    let started = Instant::now();
    let result = run(
        &argv(&tool, &[]),
        Duration::from_millis(300),
        &AtomicBool::new(false),
    );
    assert_eq!(result, Err(RunError::TimedOut));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the timeout must not wait for the script"
    );
    wait_dead(read_pid(&tmp.path().join("pids.sh")));
    wait_dead(read_pid(&tmp.path().join("pids.sleep")));
}

#[test]
fn a_cancel_kills_a_running_thumbnailer_promptly() {
    let tmp = tempfile::tempdir().unwrap();
    let pid_file = tmp.path().join("pid");
    let tool = script(
        tmp.path(),
        "slow",
        &format!("echo $$ > {}\nsleep 30", pid_file.display()),
    );
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let args = argv(&tool, &[]);
    let handle = std::thread::spawn(move || run(&args, Duration::from_secs(30), &flag));
    let pid = read_pid(&pid_file);
    assert!(alive(pid));
    let started = Instant::now();
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(handle.join().unwrap(), Err(RunError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(2));
    wait_dead(pid);
}

#[test]
fn the_thumbnailer_runs_at_a_lower_priority() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("nice");
    // Field 19 of /proc/self/stat is the nice value; `$$` is the script's shell.
    let tool = script(
        tmp.path(),
        "n",
        &format!("cut -d' ' -f19 /proc/$$/stat > {}", out.display()),
    );
    run(
        &argv(&tool, &[]),
        Duration::from_secs(5),
        &AtomicBool::new(false),
    )
    .unwrap();
    let nice: i32 = fs::read_to_string(out).unwrap().trim().parse().unwrap();
    assert!(nice >= 10, "nice was {nice}");
}

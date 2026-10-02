// Finds the programs that hold a mount open, for the `busy` error
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::fs;
use std::path::Path;

/// How many names `busy` carries at most; the rest would only be noise in a dialog.
const MAX_NAMES: usize = 3;

/// The names of the processes under `proc_root` (`/proc`) whose working folder or open files are inside `mount_point`, in process order, without repeats. Reading another user's process is refused by the system and such a process is skipped, so the answer can be missing a holder but never names one wrongly. `None` when none was found.
pub fn find_holders(proc_root: &Path, mount_point: &str) -> Option<String> {
    let mount = Path::new(mount_point);
    if mount_point.is_empty() || mount == Path::new("/") {
        return None;
    }
    let mut pids: Vec<u32> = fs::read_dir(proc_root)
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    pids.sort_unstable();
    let mut names: Vec<String> = Vec::new();
    for pid in pids {
        let dir = proc_root.join(pid.to_string());
        if !holds(&dir, mount) {
            continue;
        }
        let name = fs::read_to_string(dir.join("comm"))
            .map(|text| text.trim().to_string())
            .unwrap_or_default();
        let name = if name.is_empty() {
            format!("process {pid}")
        } else {
            name
        };
        if !names.contains(&name) {
            names.push(name);
            if names.len() == MAX_NAMES {
                break;
            }
        }
    }
    (!names.is_empty()).then(|| names.join(", "))
}

fn holds(process: &Path, mount: &Path) -> bool {
    if fs::read_link(process.join("cwd")).is_ok_and(|target| target.starts_with(mount)) {
        return true;
    }
    let Ok(fds) = fs::read_dir(process.join("fd")) else {
        return false;
    };
    fds.filter_map(|fd| fd.ok())
        .any(|fd| fs::read_link(fd.path()).is_ok_and(|target| target.starts_with(mount)))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn process(root: &Path, pid: u32, name: &str, cwd: &str, fds: &[&str]) {
        let dir = root.join(pid.to_string());
        fs::create_dir_all(dir.join("fd")).unwrap();
        fs::write(dir.join("comm"), format!("{name}\n")).unwrap();
        symlink(cwd, dir.join("cwd")).unwrap();
        for (n, target) in fds.iter().enumerate() {
            symlink(target, dir.join("fd").join(n.to_string())).unwrap();
        }
    }

    #[test]
    fn a_process_in_the_mount_is_named() {
        let tmp = tempfile::tempdir().unwrap();
        process(tmp.path(), 100, "bash", "/run/media/me/USB/docs", &[]);
        process(tmp.path(), 101, "other", "/home/me", &["/dev/null"]);
        assert_eq!(
            find_holders(tmp.path(), "/run/media/me/USB").as_deref(),
            Some("bash")
        );
    }

    #[test]
    fn an_open_file_counts_and_names_are_listed_once_in_process_order() {
        let tmp = tempfile::tempdir().unwrap();
        process(tmp.path(), 300, "vim", "/home", &["/mnt/x/a.txt"]);
        process(tmp.path(), 200, "less", "/home", &["/mnt/x/b.txt"]);
        process(tmp.path(), 250, "less", "/home", &["/mnt/x/c.txt"]);
        assert_eq!(
            find_holders(tmp.path(), "/mnt/x").as_deref(),
            Some("less, vim")
        );
    }

    #[test]
    fn a_folder_that_only_shares_a_name_prefix_does_not_count() {
        let tmp = tempfile::tempdir().unwrap();
        process(tmp.path(), 1, "bash", "/mnt/xylophone", &["/mnt/x2/file"]);
        assert_eq!(find_holders(tmp.path(), "/mnt/x"), None);
    }

    #[test]
    fn nothing_is_found_for_the_root_an_empty_mount_or_an_unreadable_proc() {
        let tmp = tempfile::tempdir().unwrap();
        process(tmp.path(), 1, "init", "/", &[]);
        assert_eq!(find_holders(tmp.path(), "/"), None);
        assert_eq!(find_holders(tmp.path(), ""), None);
        assert_eq!(find_holders(&tmp.path().join("missing"), "/mnt/x"), None);
    }

    #[test]
    fn at_most_three_names_are_given() {
        let tmp = tempfile::tempdir().unwrap();
        for (pid, name) in [(1, "a"), (2, "b"), (3, "c"), (4, "d")] {
            process(tmp.path(), pid, name, "/mnt/x", &[]);
        }
        assert_eq!(
            find_holders(tmp.path(), "/mnt/x").as_deref(),
            Some("a, b, c")
        );
    }
}

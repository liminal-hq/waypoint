// Plugin-level tests: the commands and the overlay run against a mock Tauri app and real repositories
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
use tauri::{Listener, Manager, WebviewUrl, WebviewWindowBuilder};
use waypoint_path::{FilePath, VfsPath};
use waypoint_protocol::{GitChanged, GitHeadKind, Location};
use waypoint_vfs::{FolderMarks, GitChange};

use crate::commands;
use crate::{init, Git, CHANGED_EVENT};

fn have_git() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_AUTHOR_NAME", "Ada")
        .env("GIT_AUTHOR_EMAIL", "ada@example.test")
        .env("GIT_COMMITTER_NAME", "Ada")
        .env("GIT_COMMITTER_EMAIL", "ada@example.test")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A repository with `a.txt` and `src/lib.rs` committed; `None` where `git` is missing.
fn repo() -> Option<tempfile::TempDir> {
    if !have_git() {
        assert!(
            std::env::var_os("WAYPOINT_GIT_REQUIRE").is_none(),
            "WAYPOINT_GIT_REQUIRE is set but `git` is missing"
        );
        eprintln!("skipped: `git` is not installed");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    std::fs::write(dir.path().join("a.txt"), "a").unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "lib").unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "base"]);
    Some(dir)
}

fn app() -> tauri::App<MockRuntime> {
    let app = mock_builder()
        .plugin(init())
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    for label in ["main", "other"] {
        WebviewWindowBuilder::new(&app, label, WebviewUrl::App("index.html".into()))
            .build()
            .expect("the mock window opens");
    }
    app
}

fn window(app: &tauri::App<MockRuntime>, label: &str) -> tauri::Window<MockRuntime> {
    app.get_webview_window(label)
        .expect("the window exists")
        .as_ref()
        .window()
}

fn location(path: &Path) -> Location {
    FilePath::from_path(path).unwrap().to_location()
}

fn changes(window: &tauri::Window<MockRuntime>) -> Receiver<GitChanged> {
    let (sender, receiver) = channel();
    window.listen(CHANGED_EVENT, move |event| {
        let _ = sender.send(serde_json::from_str(event.payload()).expect("a Git event"));
    });
    receiver
}

fn watch(
    app: &tauri::App<MockRuntime>,
    label: &str,
    folder: &Path,
) -> Option<waypoint_protocol::GitWatch> {
    tauri::async_runtime::block_on(commands::git_watch(
        window(app, label),
        app.state::<Git>(),
        location(folder),
    ))
    .unwrap()
}

fn wait_for(receiver: &Receiver<GitChanged>, accept: impl Fn(&GitChanged) -> bool) -> GitChanged {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(left) {
            Ok(event) if accept(&event) => return event,
            Ok(_) => {}
            Err(_) => panic!("the awaited Git event never came"),
        }
    }
}

#[test]
fn watching_a_folder_in_a_repository_replies_and_then_reports_changes() {
    let Some(dir) = repo() else { return };
    let app = app();
    let events = changes(&window(&app, "main"));
    let watch = watch(&app, "main", &dir.path().join("src")).expect("a repository");
    assert_eq!(watch.root, location(dir.path()));
    assert_eq!(
        watch.name,
        dir.path().file_name().unwrap().to_string_lossy()
    );
    // The first status may or may not be done: either way an event gives it.
    let first = wait_for(&events, |event| event.id == watch.id);
    assert_eq!(first.summary.head, "main");
    assert_eq!(first.summary.head_kind, GitHeadKind::Branch);
    assert!(!first.summary.is_dirty());

    std::fs::write(dir.path().join("a.txt"), "changed").unwrap();
    let dirty = wait_for(&events, |event| event.summary.unstaged == 1);
    assert!(dirty.revision > first.revision);
    assert!(dirty.summary.is_dirty());
}

/// The page draws the branch from the reply or from the first event for the watch's id. A status
/// that finished before the id was known must still reach it: through the reply.
#[test]
fn a_watch_always_reaches_the_page_through_its_reply_or_an_event() {
    let Some(dir) = repo() else { return };
    let app = app();
    let events = changes(&window(&app, "main"));
    for round in 0..40 {
        let watch = watch(&app, "main", dir.path()).expect("a repository");
        if watch.summary.is_none() {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match events.recv_timeout(left) {
                    Ok(event) if event.id == watch.id => break,
                    Ok(_) => {}
                    Err(_) => panic!("round {round}: neither the reply nor an event had a status"),
                }
            }
        }
        tauri::async_runtime::block_on(commands::git_unwatch(
            window(&app, "main"),
            app.state::<Git>(),
            watch.id,
        ))
        .unwrap();
    }
}

#[test]
fn a_folder_outside_a_repository_has_nothing_to_watch() {
    let Some(_repo) = repo() else { return };
    let elsewhere = tempfile::tempdir().unwrap();
    let app = app();
    assert!(watch(&app, "main", elsewhere.path()).is_none());
    // A revision location is not a local folder.
    let revision = Location::new("repo @ main", "git+file:///tmp/none!/?rev=main");
    let reply = tauri::async_runtime::block_on(commands::git_watch(
        window(&app, "main"),
        app.state::<Git>(),
        revision,
    ))
    .unwrap();
    assert!(reply.is_none());
}

#[test]
fn unwatching_and_closing_a_window_end_the_watches() {
    let Some(dir) = repo() else { return };
    let app = app();
    let first = watch(&app, "main", dir.path()).unwrap();
    let second = watch(&app, "other", dir.path()).unwrap();
    assert_ne!(first.id, second.id);
    let git = app.state::<Git>();
    assert_eq!(git.watching(), 2);
    // One window cannot stop another's watch.
    tauri::async_runtime::block_on(commands::git_unwatch(
        window(&app, "other"),
        app.state::<Git>(),
        first.id,
    ))
    .unwrap();
    assert_eq!(git.watching(), 2);
    tauri::async_runtime::block_on(commands::git_unwatch(
        window(&app, "main"),
        app.state::<Git>(),
        first.id,
    ))
    .unwrap();
    assert_eq!(git.watching(), 1);
    assert_eq!(git.forget_window("other"), 1);
    assert_eq!(git.watching(), 0);
    assert_eq!(
        git.service.tracked(),
        0,
        "the tracker stops with its last watch"
    );
}

#[test]
fn the_switch_stops_everything_and_turns_back_on() {
    let Some(dir) = repo() else { return };
    let app = app();
    let git = app.state::<Git>();
    let status = tauri::async_runtime::block_on(commands::get_status(app.state::<Git>())).unwrap();
    assert!(status.available);
    assert!(status.features.iter().any(|f| f == "watch"));
    assert!(watch(&app, "main", dir.path()).is_some());

    git.set_enabled(false);
    assert_eq!(git.watching(), 0, "turning it off ends every watch");
    assert!(watch(&app, "main", dir.path()).is_none());
    let off = tauri::async_runtime::block_on(commands::get_status(app.state::<Git>())).unwrap();
    assert!(!off.available);
    assert!(off.reason.unwrap().contains("Settings"));

    git.set_enabled(true);
    assert!(watch(&app, "main", dir.path()).is_some());
}

#[test]
fn badges_count_what_changed_inside_each_folder() {
    let Some(dir) = repo() else { return };
    std::fs::write(dir.path().join("src/lib.rs"), "changed").unwrap();
    std::fs::write(dir.path().join("src/new.rs"), "new").unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let app = app();
    let badges = tauri::async_runtime::block_on(commands::git_badges(
        app.state::<Git>(),
        vec![
            location(dir.path()),
            location(&dir.path().join("src")),
            location(elsewhere.path()),
        ],
    ))
    .unwrap();
    assert_eq!(badges.len(), 2, "the folder outside a repository has none");
    assert_eq!(badges[0].changed, 2);
    assert_eq!(badges[0].uri, location(dir.path()).uri);
    assert_eq!(badges[1].changed, 2);
    // Clean repositories have no badge.
    let clean = repo().unwrap();
    let none = tauri::async_runtime::block_on(commands::git_badges(
        app.state::<Git>(),
        vec![location(clean.path())],
    ))
    .unwrap();
    assert!(none.is_empty());
    app.state::<Git>().set_enabled(false);
    let off = tauri::async_runtime::block_on(commands::git_badges(
        app.state::<Git>(),
        vec![location(dir.path())],
    ))
    .unwrap();
    assert!(off.is_empty());
}

#[test]
fn the_overlay_sends_marks_for_a_folder_and_follows_the_switch() {
    let Some(dir) = repo() else { return };
    std::fs::write(dir.path().join("a.txt"), "changed").unwrap();
    let git = Git::default();
    let overlay = git.overlay();
    let received: Arc<Mutex<Vec<FolderMarks>>> = Arc::default();
    let sink: waypoint_vfs::MarkSink = {
        let received = received.clone();
        Arc::new(move |marks| received.lock().unwrap().push(marks))
    };
    let folder = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    let guard = overlay.attach(&folder, sink).expect("a repository");

    let until = |what: &str, accept: &dyn Fn(&FolderMarks) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if received.lock().unwrap().iter().any(accept) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("never received: {what}");
    };
    until("the changed file", &|marks| {
        marks
            .names
            .get(std::ffi::OsStr::new("a.txt"))
            .is_some_and(|mark| mark.unstaged == Some(GitChange::Modified))
    });

    // Off: the last thing sent is an empty set, so the listing loses its marks.
    git.set_enabled(false);
    assert!(received.lock().unwrap().last().unwrap().is_empty());
    let count = received.lock().unwrap().len();
    std::fs::write(dir.path().join("src/lib.rs"), "more").unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        received.lock().unwrap().len(),
        count,
        "nothing is sent while off"
    );

    // On again: the same listing is decorated again.
    git.set_enabled(true);
    until("the marks again", &|marks| {
        marks
            .names
            .get(std::ffi::OsStr::new("src"))
            .is_some_and(|mark| mark.inside == 1)
    });
    drop(guard);
    assert_eq!(
        git.service.tracked(),
        0,
        "closing the listing stops watching"
    );
}

#[test]
fn a_folder_of_repositories_marks_each_one_and_a_plain_folder_gets_nothing() {
    let Some(first) = repo() else { return };
    let parent = tempfile::tempdir().unwrap();
    // `repo` made its folder in the temporary directory; make two repositories under one parent.
    drop(first);
    for name in ["one", "two"] {
        let path = parent.path().join(name);
        std::fs::create_dir(&path).unwrap();
        git(&path, &["init", "-q", "-b", "main"]);
    }
    std::fs::create_dir(parent.path().join("plain")).unwrap();
    let git_state = Git::default();
    let received: Arc<Mutex<Vec<FolderMarks>>> = Arc::default();
    let sink: waypoint_vfs::MarkSink = {
        let received = received.clone();
        Arc::new(move |marks| received.lock().unwrap().push(marks))
    };
    let folder = VfsPath::File(FilePath::from_path(parent.path()).unwrap());
    let guard = git_state
        .overlay()
        .attach(&folder, sink)
        .expect("two repositories");
    let marks = received.lock().unwrap().last().unwrap().clone();
    assert_eq!(marks.names.len(), 2);
    assert!(marks.names[std::ffi::OsStr::new("one")].repository);
    assert!(marks.names[std::ffi::OsStr::new("two")].repository);
    assert_eq!(
        git_state.service.tracked(),
        0,
        "nothing is watched outside a working tree"
    );
    drop(guard);
    let plain = VfsPath::File(FilePath::from_path(parent.path().join("plain")).unwrap());
    assert!(git_state
        .overlay()
        .attach(&plain, Arc::new(|_| {}))
        .is_none());
}

#[test]
fn a_subfolder_that_is_a_repository_is_marked_inside_a_working_tree_too() {
    let Some(dir) = repo() else { return };
    let nested = dir.path().join("vendor");
    std::fs::create_dir(&nested).unwrap();
    git(&nested, &["init", "-q", "-b", "main"]);
    let git_state = Git::default();
    let received: Arc<Mutex<Vec<FolderMarks>>> = Arc::default();
    let sink: waypoint_vfs::MarkSink = {
        let received = received.clone();
        Arc::new(move |marks| received.lock().unwrap().push(marks))
    };
    let folder = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    let _guard = git_state.overlay().attach(&folder, sink).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if received.lock().unwrap().iter().any(|marks| {
            marks
                .names
                .get(std::ffi::OsStr::new("vendor"))
                .is_some_and(|m| m.repository)
        }) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the nested repository was never marked");
}

#[test]
fn a_folder_that_is_not_in_a_repository_is_not_decorated_and_neither_is_a_remote_one() {
    let elsewhere = tempfile::tempdir().unwrap();
    let git = Git::default();
    let sink: waypoint_vfs::MarkSink = Arc::new(|_| {});
    let folder = VfsPath::File(FilePath::from_path(elsewhere.path()).unwrap());
    assert!(git.overlay().attach(&folder, sink.clone()).is_none());
    let revision = VfsPath::from_uri("git+file:///tmp/x!/?rev=main").unwrap();
    assert!(git.overlay().attach(&revision, sink).is_none());
}

#[test]
fn path_info_lists_the_commits_that_changed_a_path_and_counts_what_changed_since() {
    let Some(dir) = repo() else { return };
    std::fs::write(dir.path().join("a.txt"), "a\nmore\n").unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "grow a"]);
    std::fs::write(dir.path().join("a.txt"), "a\nmore\nand more\n").unwrap();
    let app = app();
    let info = tauri::async_runtime::block_on(commands::git_path_info(
        app.state::<Git>(),
        location(&dir.path().join("a.txt")),
        Some(5),
    ))
    .unwrap()
    .expect("a repository");
    let summaries: Vec<&str> = info.commits.iter().map(|c| c.summary.as_str()).collect();
    assert_eq!(summaries, ["grow a", "base"]);
    assert_eq!(info.commits[0].short.len(), 8);
    assert_eq!(info.commits[0].author, "Ada");
    assert!(!info.truncated);
    assert_eq!(
        (info.diff.files, info.diff.added, info.diff.removed),
        (1, 1, 0)
    );
    // A folder asks the same question of everything in it, and the limit holds.
    let folder = tauri::async_runtime::block_on(commands::git_path_info(
        app.state::<Git>(),
        location(dir.path()),
        Some(1),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(folder.commits.len(), 1);
    // Outside a repository, or with Git off, there is nothing.
    let elsewhere = tempfile::tempdir().unwrap();
    assert!(tauri::async_runtime::block_on(commands::git_path_info(
        app.state::<Git>(),
        location(elsewhere.path()),
        None
    ))
    .unwrap()
    .is_none());
    app.state::<Git>().set_enabled(false);
    assert!(tauri::async_runtime::block_on(commands::git_path_info(
        app.state::<Git>(),
        location(dir.path()),
        None
    ))
    .unwrap()
    .is_none());
}

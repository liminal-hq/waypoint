// A 50 000-file repository: browsing a revision and the first status stay within budget, and the
// status never holds up a listing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Not run by default (it writes 50 000 files and timings need a quiet machine):
//!
//!   WAYPOINT_TEST_TMP=/some/disk/folder cargo test -p waypoint-provider-git --release \
//!     --test budget -- --ignored --nocapture
//!
//! `WAYPOINT_TEST_TMP` names a folder on a disk (not a small tmpfs); the fixture is deleted
//! afterwards. The budgets are asserted in release builds only. They are the milestone 2 listing
//! budget (a folder of 100 000 entries within half a second, the remote one A85 reuses) scaled to
//! the count, and a first status within two seconds, which `git status` itself needs a fraction of
//! on the same files.

mod support;

use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use support::*;
use waypoint_provider_git::{
    compute, GitProvider, StatusOptions, TrackEvent, Tracker, TrackerOptions,
};
use waypoint_vfs::{CancelToken, Provider};

const FOLDERS: usize = 10;
const PER_FOLDER: usize = 5_000;
const ENFORCE: bool = !cfg!(debug_assertions);

#[test]
#[ignore = "a benchmark: run it with --ignored and --release"]
fn fifty_thousand_files_list_within_budget_and_status_arrives_after() {
    if !have_git() {
        assert!(
            !required(),
            "WAYPOINT_GIT_REQUIRE is set but `git` is missing"
        );
        return;
    }
    let base = std::env::var_os("WAYPOINT_TEST_TMP").map(std::path::PathBuf::from);
    let dir = match &base {
        Some(base) => tempfile::tempdir_in(base).unwrap(),
        None => tempfile::tempdir().unwrap(),
    };
    let repo = Repo { dir };
    repo.git(&["init", "-q", "-b", "main"]);
    for folder in 0..FOLDERS {
        for file in 0..PER_FOLDER {
            repo.write(
                &format!("dir{folder}/file{file}.txt"),
                &format!("{folder} {file}\n"),
            );
        }
    }
    repo.commit_all("fifty thousand files");
    // A clone arrives packed; so does a repository that has been repacked. Loose objects are
    // measured too, below, since a fresh commit of this size is all loose.
    let loose = {
        let started = Instant::now();
        let provider = GitProvider::new();
        for folder in 0..FOLDERS {
            provider
                .list(
                    &repo.at_inner(None, &format!("dir{folder}")),
                    &CancelToken::new(),
                    0,
                    &mut |_| {},
                )
                .unwrap();
        }
        started.elapsed()
    };
    repo.git(&["repack", "-a", "-d", "-q"]);
    // Some changes, so the status has something to report.
    repo.write("dir3/file7.txt", "edited");
    repo.write("untracked.txt", "new");

    let provider = GitProvider::new();
    let cancel = CancelToken::new();

    // The tracker starts at once: it never makes the caller wait for the status.
    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    let tracker = Tracker::start(
        repo.path().to_path_buf(),
        TrackerOptions::default(),
        Arc::new(move |event| {
            if let TrackEvent::Updated(snapshot) = event {
                let _ = tx.send((Instant::now(), snapshot));
            }
        }),
    );
    let start_took = started.elapsed();

    // Browse while the status is being computed.
    let listing_started = Instant::now();
    let root = provider
        .list(&repo.at(None), &cancel, 0, &mut |_| {})
        .unwrap();
    let mut total = 0;
    for folder in 0..FOLDERS {
        total += provider
            .list(
                &repo.at_inner(None, &format!("dir{folder}")),
                &cancel,
                0,
                &mut |_| {},
            )
            .unwrap()
            .len();
    }
    let listing_took = listing_started.elapsed();
    assert_eq!(root.len(), FOLDERS);
    assert_eq!(total, FOLDERS * PER_FOLDER);

    let (arrived, snapshot) = rx.recv_timeout(Duration::from_secs(30)).unwrap();
    let status_took = arrived.duration_since(started);

    let mut warm = Vec::new();
    for _ in 0..3 {
        let started = Instant::now();
        compute(repo.path(), &StatusOptions::default(), &cancel, None).unwrap();
        warm.push(started.elapsed());
    }
    println!(
        "loose objects {loose:?}; tracker start {start_took:?}; {total} entries listed in {listing_took:?} (scale: 100 000 in 500 ms); first status {status_took:?}; warm status {warm:?}"
    );
    assert_eq!(snapshot.status.len(), 2);
    drop(tracker);

    if ENFORCE {
        assert!(
            start_took < Duration::from_millis(50),
            "starting the tracker took {start_took:?}"
        );
        assert!(
            listing_took < Duration::from_millis(250),
            "listing 50 000 entries took {listing_took:?}"
        );
        assert!(
            status_took < Duration::from_secs(2),
            "the first status took {status_took:?}"
        );
    }
}

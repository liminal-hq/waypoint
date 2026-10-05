// Times the provider on a repository: the figures in the library decision (D161) come from here.
//
//     cargo run --release -p waypoint-provider-git --example measure -- /path/to/repo [inner/folder]
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::time::Instant;

use waypoint_path::{FilePath, GitPath, VfsPath};
use waypoint_provider_git::{compute, summarize, GitProvider, StatusOptions};
use waypoint_vfs::{CancelToken, Provider};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().expect("the repository folder");
    let inner = args.next();
    let root = std::fs::canonicalize(&root).unwrap();
    let root = root.as_path();
    let cancel = CancelToken::new();

    for run in 1..=3 {
        let started = Instant::now();
        let status = compute(root, &StatusOptions::default(), &cancel, None).unwrap();
        let took = started.elapsed();
        println!(
            "status #{run}: {:>8.1} ms, {} entries",
            took.as_secs_f64() * 1000.0,
            status.len()
        );
        if run == 3 {
            let started = Instant::now();
            let summary = summarize(root, &status).unwrap();
            println!(
                "summary:    {:>8.1} ms ({:?}, {} staged, {} unstaged, {} untracked)",
                started.elapsed().as_secs_f64() * 1000.0,
                summary.head,
                summary.counts.staged,
                summary.counts.unstaged,
                summary.counts.untracked
            );
        }
    }

    if let Some(inner) = &inner {
        let scope = [inner.as_bytes().to_vec()];
        for run in 1..=3 {
            let started = Instant::now();
            let status = compute(root, &StatusOptions::default(), &cancel, Some(&scope)).unwrap();
            println!(
                "scoped #{run}: {:>8.1} ms for `{inner}`, {} entries",
                started.elapsed().as_secs_f64() * 1000.0,
                status.len()
            );
        }
    }

    let provider = GitProvider::new();
    let mut path = VfsPath::Git(GitPath::new(FilePath::from_path(root).unwrap(), None).unwrap());
    for part in inner
        .iter()
        .flat_map(|i| i.split('/'))
        .filter(|p| !p.is_empty())
    {
        path = path.join(part).unwrap();
    }
    for run in 1..=3 {
        let started = Instant::now();
        let mut first_rows = None;
        let entries = provider
            .list_batches(&path, &cancel, 0, &mut |batch| {
                first_rows.get_or_insert_with(|| started.elapsed());
                let _ = batch;
            })
            .map(|_| ())
            .and_then(|_| provider.list(&path, &cancel, 0, &mut |_| {}))
            .unwrap();
        println!(
            "list #{run}:   {:>8.1} ms for {} entries of {}",
            started.elapsed().as_secs_f64() * 1000.0,
            entries.len(),
            path.display()
        );
    }
}

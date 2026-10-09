// Cancelling a call that is under way reaches the helper's work.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use support::*;
use waypoint_elevated::ServeConfig;
use waypoint_protocol::VfsError;
use waypoint_vfs::{CancelToken, LocalProvider, Provider};

fn cancel_after_start(provider: &TestProvider, token: &CancelToken) {
    wait_until("the call to start", || {
        provider.started.load(Ordering::SeqCst) >= 1
    });
    std::thread::sleep(Duration::from_millis(50));
    token.cancel();
}

#[test]
fn a_cancelled_list_returns_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(TestProvider::new());
    let rig = Rig::connected(provider.clone(), ServeConfig::default());
    let token = CancelToken::new();
    let result = std::thread::scope(|scope| {
        let call = scope.spawn(|| rig.client.list(&admin(dir.path()), &token, 0, &mut |_| {}));
        cancel_after_start(&provider, &token);
        call.join().unwrap()
    });
    assert!(matches!(result, Err(VfsError::Cancelled)), "{result:?}");
    // The connection is still good.
    provider.release.store(true, Ordering::SeqCst);
    rig.client.stat(&admin(dir.path())).unwrap();
}

#[test]
fn a_cancelled_batched_list_returns_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(TestProvider::new());
    let rig = Rig::connected(provider.clone(), ServeConfig::default());
    let token = CancelToken::new();
    let result = std::thread::scope(|scope| {
        let call = scope.spawn(|| {
            rig.client
                .list_batches(&admin(dir.path()), &token, 0, &mut |_| {})
        });
        cancel_after_start(&provider, &token);
        call.join().unwrap()
    });
    assert!(matches!(result, Err(VfsError::Cancelled)), "{result:?}");
}

#[test]
fn a_cancelled_folder_size_reports_that_it_was_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(TestProvider::new());
    let rig = Rig::connected(provider.clone(), ServeConfig::default());
    let token = CancelToken::new();
    let run = std::thread::scope(|scope| {
        let call = scope.spawn(|| {
            rig.client
                .folder_size(&admin(dir.path()), &token, &mut |_| {})
        });
        cancel_after_start(&provider, &token);
        call.join().unwrap()
    })
    .unwrap();
    assert!(run.cancelled);
}

#[test]
fn a_token_that_is_cancelled_before_the_call_never_asks_the_helper() {
    let dir = tempfile::tempdir().unwrap();
    let rig = Rig::connected(Arc::new(LocalProvider::new()), ServeConfig::default());
    let token = CancelToken::new();
    token.cancel();
    let path = admin(dir.path());
    assert!(matches!(
        rig.client.list(&path, &token, 0, &mut |_| {}),
        Err(VfsError::Cancelled)
    ));
    assert!(matches!(
        rig.client.list_batches(&path, &token, 0, &mut |_| {}),
        Err(VfsError::Cancelled)
    ));
}

#[test]
fn progress_and_folder_totals_come_back() {
    let dir = tempfile::tempdir().unwrap();
    for n in 0..20 {
        std::fs::write(dir.path().join(format!("f{n}")), vec![0u8; 100]).unwrap();
    }
    let rig = Rig::connected(Arc::new(LocalProvider::new()), ServeConfig::default());
    let path = admin(dir.path());
    let entries = rig
        .client
        .list(&path, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(entry_names(&entries).len(), 20);
    let mut batches = 0;
    let mut total = 0;
    rig.client
        .list_batches(&path, &CancelToken::new(), 0, &mut |batch| {
            batches += 1;
            total += batch.len();
        })
        .unwrap();
    assert!(batches >= 1);
    assert_eq!(total, 20);
    let run = rig
        .client
        .folder_size(&path, &CancelToken::new(), &mut |_| {})
        .unwrap();
    assert_eq!(
        (run.totals.files, run.totals.bytes, run.cancelled),
        (20, 2000, false)
    );
    assert!(rig.client.free_space(&path).is_some());
    assert!(rig.client.volume_id(&path).is_some());
    assert!(rig.client.details(&path).is_ok());
}

#[test]
fn a_large_folder_is_listed_in_pieces_that_fit_a_frame() {
    let dir = tempfile::tempdir().unwrap();
    for n in 0..1500 {
        std::fs::write(dir.path().join(format!("{n:04}-{}", "x".repeat(200))), b"").unwrap();
    }
    let rig = Rig::connected(Arc::new(LocalProvider::new()), ServeConfig::default());
    let path = admin(dir.path());
    let entries = rig
        .client
        .list(&path, &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(entries.len(), 1500);
}

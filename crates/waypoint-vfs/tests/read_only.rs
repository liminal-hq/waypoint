// A listing says whether its provider writes anything, so the views can hide the commands that need it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;

use waypoint_path::{CaseRule, FilePath, VfsPath};
use waypoint_vfs::{
    Filter, Listing, ListingHandle, ListingOptions, LocalProvider, MemoryProvider, Provider,
    SortSpec,
};

fn open(provider: Arc<dyn Provider>, path: VfsPath) -> Arc<Listing> {
    Listing::open(
        ListingHandle(1),
        path,
        provider,
        SortSpec::default(),
        Filter::default(),
        ListingOptions {
            watch: false,
            ..ListingOptions::default()
        },
        Arc::new(|_| {}),
    )
    .unwrap()
}

#[test]
fn a_provider_that_writes_gives_a_listing_that_is_not_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = VfsPath::File(FilePath::from_path(dir.path()).unwrap());
    let listing = open(Arc::new(LocalProvider::new()), path);
    assert!(!listing.snapshot().read_only);
}

#[test]
fn a_read_only_provider_marks_the_snapshot_of_a_listing_and_of_each_re_sort() {
    let dir = tempfile::tempdir().unwrap();
    let root = FilePath::from_path(dir.path()).unwrap();
    let provider = MemoryProvider::new(root.clone(), CaseRule::Sensitive);
    provider.set_read_only(true);
    let listing = open(Arc::new(provider.clone()), VfsPath::File(root));
    assert!(listing.snapshot().read_only);
    assert!(listing.set_sort(SortSpec::default()).read_only);
    assert!(provider.read_only());
    provider.set_read_only(false);
    assert!(!listing.snapshot().read_only);
}

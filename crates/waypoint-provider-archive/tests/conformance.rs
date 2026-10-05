// The shared provider conformance suite, run against an archive.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use support::*;
use waypoint_vfs::conformance::{self, Subject};

/// The suite starts from an empty folder, which for an archive is an empty zip.
#[test]
fn an_archive_passes_the_conformance_suite() {
    let dir = scratch();
    let zip = RawZip::new().write(&dir.path().join("empty.zip"));
    let p = provider();
    let subject = Subject {
        name: "archive",
        provider: &*p,
        root: top(&zip),
    };
    conformance::run(&subject);
}

#[test]
fn the_capabilities_are_honest() {
    let p = provider();
    let caps = p.capabilities();
    assert!(!caps.write && !caps.watch && !caps.remote);
    assert!(caps.symlinks);
    assert_eq!(caps.case_rule, waypoint_path::CaseRule::Sensitive);
    assert_eq!(caps.permissions, waypoint_vfs::PermissionModel::Unix);
    assert_eq!(caps.rename, waypoint_vfs::RenameSupport::None);
    assert!(
        !caps.range_read,
        "a deflated entry is read past, not seeked into"
    );
}

use waypoint_vfs::Provider;

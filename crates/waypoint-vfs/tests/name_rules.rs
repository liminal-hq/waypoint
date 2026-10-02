// The table of names `validate_name` accepts and refuses, which the frontend's TypeScript mirror
// (`apps/waypoint/src/browse/nameRules.ts`) is tested against too, so the two cannot drift.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsStr;

use serde::Deserialize;
use waypoint_path::CaseRule;
use waypoint_vfs::validate_name;

#[derive(Deserialize)]
struct Case {
    name: String,
    sensitive: bool,
    insensitive: bool,
}

#[test]
fn every_name_in_the_shared_table_is_judged_as_the_table_says() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/name_rules.json")).unwrap();
    assert!(cases.len() > 20);
    for case in cases {
        for (rule, valid) in [
            (CaseRule::Sensitive, case.sensitive),
            (CaseRule::Insensitive, case.insensitive),
        ] {
            assert_eq!(
                validate_name(OsStr::new(&case.name), rule).is_ok(),
                valid,
                "{:?} under {rule:?}",
                case.name
            );
        }
    }
}

#[test]
fn the_length_limit_is_bytes_on_linux_and_utf16_units_on_windows() {
    // The mirror's tests repeat these three cases by hand.
    let long = "x".repeat(256);
    assert!(validate_name(OsStr::new(&long), CaseRule::Sensitive).is_err());
    assert!(validate_name(OsStr::new(&long), CaseRule::Insensitive).is_err());
    let wide = "é".repeat(128);
    assert!(validate_name(OsStr::new(&wide), CaseRule::Sensitive).is_err());
    assert!(validate_name(OsStr::new(&wide), CaseRule::Insensitive).is_ok());
}

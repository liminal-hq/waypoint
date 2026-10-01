// The Link job: a symbolic link in the destination to each source, which stays where it is.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;
mod xfer;

use xfer::*;

fn dst_tree<P: Provider + 'static>(h: &Harness<P>) -> Tree {
    tree_of(h.provider.as_ref(), &h.path("dst"))
}

fn link_to<P: Provider + 'static>(h: &Harness<P>, relative: &str) -> Node {
    Node::Link(h.path(relative).display())
}

#[test]
fn a_link_is_made_to_each_source_and_nothing_is_read_or_changed() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        if !links {
            return;
        }
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/f", "file"),
                ("src/d/", ""),
                ("src/d/x", "x"),
                ("dst/", ""),
            ]),
        );
        h.provider.symlink(&h.path("src/ln"), "f".as_ref()).unwrap();
        let before = tree_of(h.provider.as_ref(), &h.path("src"));
        h.provider.reset();
        let result = go(
            &mut h,
            JobKind::Link,
            &["src/f", "src/d", "src/ln"],
            "dst",
            None,
        );
        done(&result);
        assert_eq!(h.provider.calls_of(Op::OpenRead), 0);
        assert_eq!(h.provider.calls_of(Op::CopyWithin), 0);
        // Links to the absolute places, each a link and none followed.
        let mut expected = Tree::new();
        for name in ["f", "d", "ln"] {
            expected.insert(name.to_owned(), link_to(&h, &format!("src/{name}")));
        }
        assert_eq!(dst_tree(&h), expected);
        assert_eq!(tree_of(h.provider.as_ref(), &h.path("src")), before);
        let report = result.report.unwrap();
        assert_eq!(
            report.created,
            vec![h.loc("dst/f"), h.loc("dst/d"), h.loc("dst/ln")]
        );
        // A link counts as one item and no bytes, whatever it points at.
        assert_eq!(report.progress.items_total, 3);
        assert_eq!(report.progress.items_done, 3);
        assert_eq!(report.progress.bytes_total, 0);
        assert!(leftovers(&work_tree(&h)).is_empty());
    });
}

#[test]
fn a_link_to_a_folder_can_be_made_inside_that_folder_and_beside_its_source() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(&h, &tree(&[("d/", ""), ("d/x", "x")]));
    // Inside the folder it points at, which a copy would refuse.
    let result = go(&mut h, JobKind::Link, &["d"], "d", None);
    done(&result);
    assert_eq!(
        tree_of(h.provider.as_ref(), &h.path("d")).get("d"),
        Some(&link_to(&h, "d"))
    );
    // Beside its source, in the folder that holds it: a free name, again and again.
    for expected in ["Link to x", "Link to x (2)", "Link to x (3)"] {
        let result = go(&mut h, JobKind::Link, &["d/x"], "d", None);
        done(&result);
        assert!(
            tree_of(h.provider.as_ref(), &h.path("d")).contains_key(expected),
            "{expected}"
        );
    }
}

#[test]
fn a_link_that_meets_a_name_asks_and_replaces_only_what_is_not_a_folder() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        if !links {
            return;
        }
        build(
            &h,
            &tree(&[
                ("src/", ""),
                ("src/a", "A"),
                ("src/d/", ""),
                ("src/d/x", "x"),
                ("dst/", ""),
                ("dst/a", "old file"),
                ("dst/d/", ""),
                ("dst/d/keep", "keep"),
            ]),
        );
        h.provider.reset();
        // Nothing is decided: it waits and writes nothing.
        let result = go(&mut h, JobKind::Link, &["src/a", "src/d"], "dst", None);
        let JobState::Waiting {
            reason: WaitReason::Conflicts { conflicts },
        } = &result.state
        else {
            panic!("{:?}", result.state);
        };
        // A link to a folder meets a folder as a link meets a folder: not like with like.
        assert_eq!(conflicts[0].kind, ConflictKind::FileOverFile);
        assert_eq!(conflicts[1].kind, ConflictKind::FileOverFolder);
        assert_eq!(h.provider.write_calls(), 0);

        // Replace swaps the file for the link and refuses the folder, deleting nothing.
        let mut answers = Answers::always(Some(ConflictPolicy::Replace), Some(Decision::Skip));
        let request = req(&h, JobKind::Link, &["src/a", "src/d"], "dst", None);
        let result = run(&mut h, request, &mut answers);
        done(&result);
        let now = dst_tree(&h);
        assert_eq!(now.get("a"), Some(&link_to(&h, "src/a")));
        assert_eq!(now.get("d"), Some(&Node::Dir));
        assert_eq!(now.get("d/keep"), Some(&file("keep")));
        assert_eq!(result.report.unwrap().counts.failed, 1);
        assert!(leftovers(&work_tree(&h)).is_empty());

        // Keep both puts the links beside what is there.
        let result = go(
            &mut h,
            JobKind::Link,
            &["src/a", "src/d"],
            "dst",
            Some(ConflictPolicy::KeepBoth),
        );
        done(&result);
        let now = dst_tree(&h);
        assert_eq!(now.get("a (2)"), Some(&link_to(&h, "src/a")));
        assert_eq!(now.get("d (2)"), Some(&link_to(&h, "src/d")));
    });
}

#[test]
fn a_link_is_not_blocked_by_room_or_by_what_it_points_at() {
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(&h, &tree(&[("src/", ""), ("dst/", "")]));
    put_bytes(&h, "src/big", &pattern(50_000, 1));
    memory(&h).set_space(
        VolumeId(1),
        VolumeSpace {
            free_bytes: 10,
            total_bytes: 1_000_000,
        },
    );
    h.provider.reset();
    let mut request = req(&h, JobKind::Link, &["src/big"], "dst", None);
    request.options.verify = Some(true);
    let result = run(&mut h, request, &mut Answers::default());
    done(&result);
    // Nothing was hashed: there were no bytes to check.
    assert_eq!(result.report.unwrap().transfer.verified, None);
}

#[test]
fn a_fault_or_a_cancel_at_any_step_leaves_a_whole_link_or_nothing() {
    for kind in FaultKind::ALL {
        let make = || {
            let (h, d) = memory_harness(CaseRule::Sensitive);
            build(
                &h,
                &tree(&[("src/", ""), ("src/a", "A"), ("src/b", "B"), ("dst/", "")]),
            );
            (h, d)
        };
        let (mut h, _d) = make();
        done(&go(&mut h, JobKind::Link, &["src/a", "src/b"], "dst", None));
        let calls = h.provider.calls();
        for step in 1..=calls {
            let (mut h, _d) = make();
            h.provider.fail_at(step, kind);
            let result = go(&mut h, JobKind::Link, &["src/a", "src/b"], "dst", None);
            h.provider.reset();
            let now = dst_tree(&h);
            assert!(leftovers(&now).is_empty(), "{kind:?} {step}: {now:?}");
            for (name, node) in &now {
                assert_eq!(
                    node,
                    &link_to(&h, &format!("src/{name}")),
                    "{kind:?} {step}"
                );
            }
            assert!(matches!(
                result.state,
                JobState::Done | JobState::Failed { .. }
            ));
            // The sources are untouched.
            assert_eq!(
                tree_of(h.provider.as_ref(), &h.path("src")),
                tree(&[("a", "A"), ("b", "B")])
            );
        }
    }
    let make = || {
        let (h, d) = memory_harness(CaseRule::Sensitive);
        build(
            &h,
            &tree(&[("src/", ""), ("src/a", "A"), ("src/b", "B"), ("dst/", "")]),
        );
        (h, d)
    };
    let (mut h, _d) = make();
    done(&go(&mut h, JobKind::Link, &["src/a", "src/b"], "dst", None));
    let calls = h.provider.calls();
    for step in 1..=calls {
        let (mut h, _d) = make();
        let request = req(&h, JobKind::Link, &["src/a", "src/b"], "dst", None);
        let result = run_transfer(
            &mut h,
            request,
            &mut Answers::default(),
            &small(),
            &mut |h, token| h.provider.cancel_at(step, token),
        );
        h.provider.reset();
        let now = dst_tree(&h);
        assert!(leftovers(&now).is_empty(), "cancel {step}: {now:?}");
        assert!(matches!(result.state, JobState::Done | JobState::Cancelled));
    }
}

#[test]
fn the_kind_has_its_own_name_and_title() {
    assert_eq!(
        serde_json::to_string(&JobKind::Link).unwrap(),
        r#"{"kind":"link"}"#
    );
    let (mut h, _dir) = memory_harness(CaseRule::Sensitive);
    build(&h, &tree(&[("src/", ""), ("src/a", "A"), ("dst/", "")]));
    let result = go(&mut h, JobKind::Link, &["src/a"], "dst", None);
    assert_eq!(
        h.store.job(result.id).unwrap().title,
        "Link \u{201c}a\u{201d}"
    );
}

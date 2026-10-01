// The sandbox that every other test stands on: it panics on a path outside its root, whether the
// path says so (`..`, an absolute path) or only a symlink does, and the operations never trigger it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};

use common::*;
use waypoint_vfs::{CancelToken, WriteOptions};

fn panics(what: &str, call: impl FnOnce()) {
    let outcome = catch_unwind(AssertUnwindSafe(call));
    let message = match outcome {
        Ok(()) => panic!("{what}: the sandbox let it through"),
        Err(payload) => payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default(),
    };
    assert!(message.starts_with("sandbox:"), "{what}: {message}");
}

fn outside(h: &Harness<impl Provider + 'static>) -> VfsPath {
    h.base.parent().expect("the sandbox is not the root")
}

#[test]
fn a_path_outside_the_root_panics_whatever_the_call() {
    each_provider!(|h, rule, links| {
        let _ = (rule, links);
        let out = outside(&h);
        let cancel = CancelToken::new();
        panics("stat", || drop(h.provider.stat(&out)));
        panics("list", || {
            drop(h.provider.list(&out, &cancel, 0, &mut |_| {}))
        });
        panics("create_dir", || {
            drop(h.provider.create_dir(&out.join("new").unwrap()))
        });
        panics("create_file", || {
            drop(h.provider.create_file(&out.join("new").unwrap()))
        });
        panics("remove_file", || {
            drop(h.provider.remove_file(&out.join("x").unwrap()))
        });
        panics("rename out", || {
            drop(
                h.provider
                    .rename(&h.work, &out.join("elsewhere").unwrap(), false),
            )
        });
        panics("rename in", || {
            drop(
                h.provider
                    .rename(&out.join("elsewhere").unwrap(), &h.work, false),
            )
        });
        panics("open_read", || {
            drop(h.provider.open_read(&out.join("x").unwrap()))
        });
        panics("create_write", || {
            drop(
                h.provider
                    .create_write(&out.join("x").unwrap(), WriteOptions::exclusive()),
            )
        });
        panics("symlink", || {
            drop(h.provider.symlink(&out.join("l").unwrap(), "x".as_ref()))
        });
        // A `..` is folded by the path type, so it shows up as the outside path it names.
        let climbing = h.work.join("../../escape").unwrap();
        panics("..", || drop(h.provider.stat(&climbing)));
        // Inside the root all of it works.
        h.provider
            .create_dir(&h.work.join("fine").unwrap())
            .unwrap();
        assert!(h.provider.stat(&h.work.join("fine").unwrap()).is_ok());
    });
}

#[test]
fn a_symlink_that_leads_out_panics_when_it_is_followed_and_not_when_it_is_acted_on() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        if !links {
            return;
        }
        let out = outside(&h);
        h.provider.symlink(&h.path("up"), "../..".as_ref()).unwrap();
        h.provider
            .symlink(&h.path("abs"), out.display().as_ref())
            .unwrap();
        let cancel = CancelToken::new();
        // The link itself is fair game: it is the thing renamed, read and removed.
        assert!(h.provider.stat(&h.path("up")).is_ok());
        assert!(h.provider.read_link(&h.path("abs")).is_ok());
        h.provider
            .rename(&h.path("up"), &h.path("up2"), false)
            .unwrap();
        h.provider
            .rename(&h.path("up2"), &h.path("up"), false)
            .unwrap();
        // Going through it is not.
        panics("through a relative link", || {
            drop(h.provider.stat(&h.path("up/anything")))
        });
        panics("through an absolute link", || {
            drop(h.provider.list(&h.path("abs"), &cancel, 0, &mut |_| {}))
        });
        panics("opening a link", || {
            drop(h.provider.open_read(&h.path("abs")))
        });
        panics("creating through a link", || {
            drop(h.provider.create_file(&h.path("up/new")))
        });
        // A truncating write follows a link in the last component; an exclusive one does not.
        panics("truncating through a link", || {
            drop(
                h.provider
                    .create_write(&h.path("abs"), WriteOptions::truncate()),
            )
        });
        h.provider.remove_file(&h.path("up")).unwrap();
        h.provider.remove_file(&h.path("abs")).unwrap();
    });
}

#[test]
fn a_link_that_stays_inside_is_followed_freely_and_a_loop_is_caught() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        if !links {
            return;
        }
        build(&h, &tree(&[("real/", ""), ("real/f", "x")]));
        h.provider.symlink(&h.path("ok"), "real".as_ref()).unwrap();
        // (The in-memory provider does not resolve paths through links; the guard does, and it
        // must not object to one that stays inside.)
        let _ = h.provider.stat(&h.path("ok/f"));
        h.provider
            .symlink(&h.path("loop1"), "loop2".as_ref())
            .unwrap();
        h.provider
            .symlink(&h.path("loop2"), "loop1".as_ref())
            .unwrap();
        panics("a loop", || drop(h.provider.stat(&h.path("loop1/x"))));
    });
}

#[test]
fn operations_never_follow_a_link_out_of_the_sandbox() {
    each_provider!(|h, rule, links| {
        let _ = rule;
        if !links {
            return;
        }
        let out = outside(&h);
        let mut source = tree(&[("d/", ""), ("d/f", "x")]);
        source.insert("d/up".to_owned(), Node::Link("../..".to_owned()));
        source.insert("d/abs".to_owned(), Node::Link(out.display()));
        build(&h, &source);
        // Planning walks past the links, a duplicate recreates them, and a delete removes them as
        // links: none of it enters what they point at (which would panic).
        let planned = h
            .plan(&h.request(JobKind::Duplicate, &["d"], None, None))
            .unwrap();
        assert_eq!(planned.total_items, 4);
        let result = h.run(h.request(JobKind::Duplicate, &["d"], None, None));
        assert_eq!(result.state, JobState::Done);
        let t = work_tree(&h);
        assert_eq!(t.get("d (2)/up"), Some(&Node::Link("../..".to_owned())));
        assert_eq!(
            h.run(h.request(JobKind::Delete, &["d", "d (2)"], None, None))
                .state,
            JobState::Done
        );
        assert!(work_tree(&h).is_empty());
        let _ = out;
    });
}

// The listing budget of the remote contract (A83): first rows early, 100 000 entries within a
// second or so on loopback, and bounded memory. `hundred_thousand_entries` is `#[ignore]`d, as the
// SFTP provider's is, and run by hand: `cargo nextest run -p waypoint-provider-webdav
// --run-ignored only --release listing_budget`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::time::Instant;

use support::mock::{entry_xml, Body, Mock, Reply};
use support::{config, options};
use waypoint_path::VfsPath;
use waypoint_provider_webdav::WebDavProvider;
use waypoint_vfs::{CancelToken, Provider};

fn serve(count: usize) -> Mock {
    Mock::start(move |_, _| {
        let mut reply = Reply::new(207).header("Content-Type", "application/xml");
        reply.body = Body::Stream(Box::new(move |out| {
            out.write_all(b"<?xml version=\"1.0\"?><D:multistatus xmlns:D=\"DAV:\">")?;
            out.write_all(
                b"<D:response><D:href>/many/</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>",
            )?;
            let mut chunk = String::new();
            for n in 0..count {
                chunk.push_str(&entry_xml(
                    "/many/",
                    &format!("a file with a longish name {n:07}.txt"),
                ));
                if chunk.len() > 64 * 1024 {
                    out.write_all(chunk.as_bytes())?;
                    chunk.clear();
                }
            }
            out.write_all(chunk.as_bytes())?;
            out.write_all(b"</D:multistatus>")
        }));
        reply
    })
}

fn run(count: usize) -> (usize, f64, f64) {
    let mock = serve(count);
    let provider = WebDavProvider::dav(config(None).with_options(options()));
    let path = VfsPath::from_uri(&format!("dav://127.0.0.1:{}/many", mock.port)).unwrap();
    let started = Instant::now();
    let mut first = None;
    let mut total = 0;
    provider
        .list_batches(&path, &CancelToken::new(), 0, &mut |batch| {
            first.get_or_insert_with(|| started.elapsed().as_secs_f64());
            total += batch.len();
        })
        .unwrap();
    (
        total,
        first.unwrap_or_default(),
        started.elapsed().as_secs_f64(),
    )
}

/// The peak resident memory of this process so far, in megabytes (Linux).
fn peak_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    Some(line.split_whitespace().nth(1)?.parse::<f64>().ok()? / 1024.0)
}

#[test]
fn twenty_thousand_entries_arrive_in_batches() {
    let (total, first, all) = run(20_000);
    assert_eq!(total, 20_000);
    assert!(first <= all);
}

#[test]
#[ignore = "a measurement: run by hand with --release"]
fn hundred_thousand_entries() {
    let before = peak_mb();
    let (total, first, all) = run(100_000);
    let after = peak_mb();
    println!(
        "100 000 entries: first batch {first:.3} s, all {all:.3} s, peak memory {} MB over {} MB",
        after.zip(before).map_or(f64::NAN, |(a, b)| a - b).round(),
        before.unwrap_or_default().round()
    );
    assert_eq!(total, 100_000);
    assert!(all < 2.0, "100 000 entries took {all:.2} s");
    if let Some((after, before)) = after.zip(before) {
        assert!(
            after - before < 50.0,
            "memory grew by {:.0} MB",
            after - before
        );
    }
}

/// The same measurement against Apache, which has to read 100 000 files' attributes and write the
/// answer itself, so its time is the server's more than the provider's.
#[test]
#[ignore = "a measurement: run by hand with --release"]
fn hundred_thousand_entries_from_apache() {
    let Some(server) = support::apache(support::Login::Anonymous) else {
        return;
    };
    for n in 0..100_000 {
        std::fs::write(
            server
                .data
                .join(format!("a file with a longish name {n:07}.txt")),
            "",
        )
        .unwrap();
    }
    let provider = server.provider();
    let before = peak_mb();
    let started = Instant::now();
    let (mut first, mut total) = (None, 0);
    provider
        .list_batches(&server.root(), &CancelToken::new(), 0, &mut |batch| {
            first.get_or_insert_with(|| started.elapsed().as_secs_f64());
            total += batch.len();
        })
        .unwrap();
    println!(
        "Apache, 100 000 entries: first batch {:.3} s, all {:.3} s, peak memory {:.0} MB over {:.0} MB",
        first.unwrap_or_default(),
        started.elapsed().as_secs_f64(),
        peak_mb().zip(before).map_or(f64::NAN, |(a, b)| a - b),
        before.unwrap_or_default()
    );
    assert_eq!(total, 100_000);
}

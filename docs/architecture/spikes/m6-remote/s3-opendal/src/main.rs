// Throwaway spike: opendal lister over S3 and over WebDAV.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use futures::TryStreamExt;
use opendal::{services, Operator};
use std::time::Instant;

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let op = match a[1].as_str() {
        "s3" => Operator::new(services::S3::default().bucket(&a[3]).endpoint(&format!("http://127.0.0.1:{}", a[2])).region("us-east-1")
            .access_key_id("AKIASPIKE").secret_access_key("secretspike")).unwrap(),
        "dav" => Operator::new(services::Webdav::default().endpoint(&format!("http://127.0.0.1:{}", a[2])).username("scott").password("spikepw")).unwrap(),
        _ => panic!(),
    };
    let dir = a.get(4).cloned().unwrap_or_else(|| "/".into());
    let t0 = Instant::now();
    let mut l = op.lister(&dir).await.unwrap();
    let (mut n, mut first) = (0, None);
    while let Some(_e) = l.try_next().await.unwrap() { first.get_or_insert_with(|| t0.elapsed()); n += 1; }
    println!("opendal {} first_ms={:.1} total_ms={:.0} entries={n} peak_rss_mb={:.0}", a[1], first.unwrap().as_secs_f64()*1e3, t0.elapsed().as_secs_f64()*1e3, rss_mb());
}

// Throwaway spike: aws-sdk-s3 against an S3-compatible endpoint (path-style, custom endpoint).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use std::time::Instant;

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cfg = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .endpoint_url(format!("http://127.0.0.1:{}", a[1]))
        .region(Region::new("us-east-1"))
        .credentials_provider(Credentials::new("AKIASPIKE", "secretspike", None, None, "spike"))
        .force_path_style(true)
        .build();
    let c = aws_sdk_s3::Client::from_conf(cfg);
    match a[2].as_str() {
        "list" => {
            let t0 = Instant::now();
            let mut p = c.list_objects_v2().bucket(&a[3]).into_paginator().send();
            let (mut n, mut first, mut pages) = (0, None, 0);
            while let Some(page) = p.next().await {
                let page = page.unwrap();
                first.get_or_insert_with(|| t0.elapsed());
                pages += 1;
                n += page.contents().len();
            }
            println!("aws-sdk-s3 first_page_ms={:.1} total_ms={:.0} entries={n} pages={pages} peak_rss_mb={:.0}", first.unwrap().as_secs_f64()*1e3, t0.elapsed().as_secs_f64()*1e3, rss_mb());
        }
        "big" => {
            let t0 = Instant::now();
            let mut o = c.get_object().bucket("bk").key("big.bin").send().await.unwrap();
            let mut n = 0u64;
            while let Some(b) = o.body.try_next().await.unwrap() { n += b.len() as u64; }
            let el = t0.elapsed().as_secs_f64();
            println!("big aws-sdk-s3 {} MiB in {:.2}s = {:.0} MiB/s peak_rss_mb={:.0}", n >> 20, el, (n >> 20) as f64 / el, rss_mb());
        }
        "range" => {
            let o = c.get_object().bucket("bk").key("big.bin").range("bytes=1000-1999").send().await.unwrap();
            let cr = o.content_range().map(|s| s.to_string());
            println!("range content_range={cr:?} len={}", o.body.collect().await.unwrap().into_bytes().len());
        }
        _ => panic!(),
    }
}

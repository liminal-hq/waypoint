// Throwaway spike: rust-s3 listing against an S3-compatible endpoint (path-style).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use s3::{creds::Credentials, Bucket, Region};
use std::time::Instant;

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let region = Region::Custom { region: "us-east-1".into(), endpoint: format!("http://127.0.0.1:{}", a[1]) };
    let creds = Credentials::new(Some("AKIASPIKE"), Some("secretspike"), None, None, None).unwrap();
    let bucket = Bucket::new(&a[2], region, creds).unwrap().with_path_style();
    let t0 = Instant::now();
    // `list` collects every page before returning; rust-s3 has no page stream
    let pages = bucket.list(String::new(), None).await.unwrap();
    let n: usize = pages.iter().map(|p| p.contents.len()).sum();
    println!("rust-s3 total_ms={:.0} entries={n} pages={} (collects all pages)", t0.elapsed().as_secs_f64() * 1e3, pages.len());
}

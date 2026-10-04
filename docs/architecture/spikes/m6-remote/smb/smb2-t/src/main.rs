// Throwaway spike: `smb2` crate listing and streaming download.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::time::Instant;

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut client = smb2::connect(&format!("127.0.0.1:{}", a[1]), "scott", "spikepw").await.unwrap();
    let mut share = client.connect_share("data").await.unwrap();
    match a[2].as_str() {
        "list" => {
            let t0 = Instant::now();
            let e = client.list_directory(&mut share, &a[3]).await.unwrap();
            println!("smb2 list total_ms={:.0} entries={} (collects all; sizes included) peak_rss_mb={:.0}", t0.elapsed().as_secs_f64() * 1e3, e.len(), rss_mb());
        }
        "big" => {
            let limit: u64 = std::env::var("LIMIT").ok().map(|v| v.parse().unwrap()).unwrap_or(1 << 30);
            let t0 = Instant::now();
            let mut d = client.download(&share, &a[3]).await.unwrap();
            let mut n = 0u64;
            while let Some(c) = d.next_chunk().await { n += c.unwrap().len() as u64; if n >= limit { break; } }
            let el = t0.elapsed().as_secs_f64();
            println!("big smb2 {} MiB in {:.2}s = {:.0} MiB/s peak_rss_mb={:.0}", n >> 20, el, (n >> 20) as f64 / el, rss_mb());
        }
        _ => panic!(),
    }
}

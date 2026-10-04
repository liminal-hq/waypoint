// Throwaway spike: `smb` crate (pure Rust SMB2/3 client) listing and read benchmarks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use futures_util::StreamExt;
use smb::{Client, ClientConfig, ConnectionConfig, Directory, FileAccessMask, FileCreateArgs, DirAccessMask, ReadAt, UncPath};
use smb::FileIdBothDirectoryInformation;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let port: u16 = a[1].parse().unwrap();
    let mut cfg = ClientConfig::default();
    cfg.connection = ConnectionConfig { port: Some(port), ..Default::default() };
    let client = Client::new(cfg);
    let share = UncPath::from_str(r"\\127.0.0.1\data").unwrap();
    let t = Instant::now();
    client.share_connect(&share, "scott", "spikepw".to_string()).await.unwrap();
    println!("connect+auth+tree: {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    match a[2].as_str() {
        "list" => {
            let t0 = Instant::now();
            let dir = client.create_file(&share.clone().with_path(&a[3]), &FileCreateArgs::make_open_existing(DirAccessMask::new().with_list_directory(true).with_synchronize(true).into())).await.unwrap().unwrap_dir();
            eprintln!("opened dir");
            let dir = Arc::new(dir);
            let mut st = Directory::query::<FileIdBothDirectoryInformation>(&dir, "*").await.unwrap();
            let (mut n, mut first, mut bytes) = (0usize, None, 0u64);
            while let Some(e) = st.next().await {
                let e = e.unwrap();
                if n%500==0 || (n>=9000 && n%20==0) { eprintln!("{} entries at {:?}", n, t0.elapsed()); }
                first.get_or_insert_with(|| t0.elapsed());
                bytes += e.end_of_file; // attributes arrive with the listing
                n += 1;
            }
            println!("smb list first_ms={:.1} total_ms={:.0} entries={n} (with size/times) peak_rss_mb={:.0}", first.unwrap().as_secs_f64() * 1e3, t0.elapsed().as_secs_f64() * 1e3, rss_mb());
            let _ = bytes;
        }
        "big" => {
            let limit: u64 = std::env::var("LIMIT").ok().map(|v| v.parse().unwrap()).unwrap_or(1 << 30);
            let f = client.create_file(&share.clone().with_path(&a[3]), &FileCreateArgs::make_open_existing(FileAccessMask::new().with_generic_read(true))).await.unwrap().unwrap_file();
            let chunk: usize = a.get(4).map(|c| c.parse().unwrap()).unwrap_or(1 << 20);
            let mut buf = vec![0u8; chunk];
            let (t0, mut off) = (Instant::now(), 0u64);
            while off < limit { let n = f.read_at(&mut buf, off).await.unwrap(); if n == 0 { break; } off += n as u64; }
            let el = t0.elapsed().as_secs_f64();
            println!("big smb chunk={chunk} {} MiB in {:.2}s = {:.0} MiB/s peak_rss_mb={:.0}", off >> 20, el, (off >> 20) as f64 / el, rss_mb());
        }
        _ => panic!(),
    }
}

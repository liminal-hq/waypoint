// Throwaway spike: ssh2 (libssh2) listing and read benchmarks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use ssh2::Session;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;
use std::time::Instant;

const KEYS: &str = "/tmp/claude-1000/spike-remote/ssh/";

fn connect(port: u16) -> Session {
    let tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
    tcp.set_nodelay(true).ok();
    let mut s = Session::new().unwrap();
    s.set_tcp_stream(tcp);
    s.handshake().unwrap();
    s.userauth_pubkey_file("scott", None, Path::new(&format!("{KEYS}client_ed25519")), None).unwrap();
    assert!(s.authenticated());
    s
}

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let port: u16 = a[1].parse().unwrap();
    let s = connect(port);
    let sftp = s.sftp().unwrap();
    match a[2].as_str() {
        "list" => {
            let dir = a[3].clone();
            let t0 = Instant::now();
            let mut d = sftp.opendir(Path::new(&dir)).unwrap();
            let mut names = vec![];
            let mut first = None;
            while let Ok((p, _st)) = d.readdir() {
                first.get_or_insert_with(|| t0.elapsed());
                names.push(p);
            }
            let total = t0.elapsed();
            println!("ssh2 first_ms={:.1} total_ms={:.0} entries={}", first.unwrap().as_secs_f64() * 1e3, total.as_secs_f64() * 1e3, names.len());
            if let Some(k) = a.get(4) {
                let k = k.parse::<usize>().unwrap().min(names.len());
                let t = Instant::now();
                for n in names.iter().take(k) { sftp.lstat(&Path::new(&dir).join(n)).unwrap(); }
                let el = t.elapsed();
                println!("lstat x{k} sequential: {:.0} ms ({:.3} ms/entry)", el.as_secs_f64() * 1e3, el.as_secs_f64() * 1e3 / k as f64);
            }
            println!("peak_rss_mb={:.0}", rss_mb());
        }
        "big" => {
            let limit: u64 = std::env::var("LIMIT").ok().map(|v| v.parse().unwrap()).unwrap_or(1 << 30);
            let t0 = Instant::now();
            let mut f = sftp.open(Path::new(&a[3])).unwrap();
            let mut buf = vec![0u8; 1 << 20];
            let mut got = 0u64;
            while got < limit { let n = f.read(&mut buf).unwrap(); if n == 0 { break; } got += n as u64; }
            let el = t0.elapsed().as_secs_f64();
            println!("big ssh2 {} MiB in {:.2}s = {:.0} MiB/s peak_rss_mb={:.0}", got >> 20, el, (got >> 20) as f64 / el, rss_mb());
        }
        "small" => {
            let (dir, count): (String, usize) = (a[3].clone(), a[4].parse().unwrap());
            let t0 = Instant::now();
            let mut bytes = 0;
            for i in 1..=count {
                let mut f = sftp.open(&Path::new(&dir).join(format!("s_{i}"))).unwrap();
                let mut v = vec![]; f.read_to_end(&mut v).unwrap(); bytes += v.len();
            }
            let el = t0.elapsed().as_secs_f64();
            println!("small ssh2 x{count} sequential: {:.0} ms ({:.0} files/s, {} KiB)", el * 1e3, count as f64 / el, bytes >> 10);
        }
        _ => panic!(),
    }
}

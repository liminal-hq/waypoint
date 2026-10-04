// Throwaway spike: russh + russh-sftp listing and read benchmarks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{client, Disconnect};
use russh_sftp::client::{rawsession::SftpResult, Config, RawSftpSession, SftpSession};
use russh_sftp::protocol::{OpenFlags, StatusCode};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncReadExt;

struct H;
impl client::Handler for H {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _k: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true) // host-key hook: Waypoint's known-hosts prompt plugs in here
    }
}

const KEYS: &str = "/tmp/claude-1000/spike-remote/ssh/";

async fn connect(port: u16, key: &str, pass: Option<&str>) -> client::Handle<H> {
    let mut c = client::Config::default();
    if let Ok(w) = std::env::var("WINDOW") { c.window_size = w.parse().unwrap(); }
    if let Ok(w) = std::env::var("MAXPKT") { c.maximum_packet_size = w.parse().unwrap(); }
    let cfg = Arc::new(c);
    let mut s = client::connect(cfg, ("127.0.0.1", port), H).await.unwrap();
    let k = load_secret_key(format!("{KEYS}{key}"), pass).unwrap();
    let hash = s.best_supported_rsa_hash().await.unwrap().flatten();
    let r = s.authenticate_publickey("scott", PrivateKeyWithHashAlg::new(Arc::new(k), hash)).await.unwrap();
    assert!(r.success(), "auth failed");
    s
}

async fn raw(h: &client::Handle<H>, cfg: Config) -> RawSftpSession {
    let ch = h.channel_open_session().await.unwrap();
    ch.request_subsystem(true, "sftp").await.unwrap();
    let r = RawSftpSession::new_with_config(ch.into_stream(), cfg);
    r.init().await.unwrap();
    r
}

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0
}

async fn rd(s: &RawSftpSession, h: String) -> SftpResult<russh_sftp::protocol::Name> { s.readdir(h).await }

fn is_eof<T>(r: &SftpResult<T>) -> bool {
    matches!(r, Err(russh_sftp::client::error::Error::Status(s)) if s.status_code == StatusCode::Eof)
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let port: u16 = a[1].parse().unwrap();
    let cmd = a[2].as_str();
    if cmd == "auth" {
        // passphrase-protected key
        let t = Instant::now();
        let h = connect(port, "client_pass_ed25519", Some("pass123")).await;
        println!("key+passphrase ed25519: ok {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
        drop(h);
        let h = connect(port, "client_rsa", None).await;
        println!("rsa key (rsa-sha2 negotiated): ok");
        drop(h);
        // ssh-agent via SSH_AUTH_SOCK
        #[cfg(unix)]
        let mut agent = russh::keys::agent::client::AgentClient::connect_env().await.unwrap();
        #[cfg(windows)]
        let mut agent = russh::keys::agent::client::AgentClient::connect_pageant().await.unwrap();
        let ids = agent.request_identities().await.unwrap();
        let cfg = Arc::new(client::Config::default());
        let mut s = client::connect(cfg.clone(), ("127.0.0.1", port), H).await.unwrap();
        let mut ok = false;
        for id in ids.iter() {
            let r = s.authenticate_publickey_with("scott", id.public_key().into_owned(), None, &mut agent).await.unwrap();
            if r.success() { ok = true; break; }
        }
        println!("ssh-agent ({} identities): ok={ok}", ids.len());
        // ProxyJump: open direct-tcpip through the first hop, run a second SSH session over that channel
        let hop = connect(port, "client_ed25519", None).await;
        let ch = hop.channel_open_direct_tcpip("127.0.0.1", 2222, "127.0.0.1", 0).await.unwrap();
        let mut s2 = client::connect_stream(cfg, ch.into_stream(), H).await.unwrap();
        let k = load_secret_key(format!("{KEYS}client_ed25519"), None).unwrap();
        let r = s2.authenticate_publickey("scott", PrivateKeyWithHashAlg::new(Arc::new(k), None)).await.unwrap();
        println!("jump host (direct-tcpip, second session over channel): ok={}", r.success());
        // methods offered by the server
        let mut s3 = client::connect(Arc::new(client::Config::default()), ("127.0.0.1", port), H).await.unwrap();
        let r = s3.authenticate_none("scott").await.unwrap();
        println!("none-auth probe: {:?}", r);
        return;
    }
    let h = connect(port, "client_ed25519", None).await;
    match cmd {
        "list" => {
            let (dir, mode) = (a[3].clone(), a[4].as_str());
            let sftp = raw(&h, Config::default()).await;
            let t0 = Instant::now();
            if mode == "hl" {
                // high-level API: collects everything before returning
                let s2 = SftpSession::new({
                    let ch = h.channel_open_session().await.unwrap();
                    ch.request_subsystem(true, "sftp").await.unwrap();
                    ch.into_stream()
                }).await.unwrap();
                let t0 = Instant::now();
                let n = s2.read_dir(dir).await.unwrap().count();
                println!("hl total_ms={:.0} entries={n} (no first batch: not streaming)", t0.elapsed().as_secs_f64() * 1e3);
            } else {
                let depth: usize = mode.strip_prefix("pipe:").map(|d| d.parse().unwrap()).unwrap_or(1);
                let handle = sftp.opendir(dir.clone()).await.unwrap().handle;
                let mut names: Vec<String> = vec![];
                let (mut first, mut batches) = (None, 0);
                use futures::StreamExt;
                // sliding window: `depth` READDIR requests on the wire at once
                let mut futs = futures::stream::FuturesOrdered::new();
                for _ in 0..depth { futs.push_back(rd(&sftp, handle.clone())); }
                let mut eof = false;
                while let Some(r) = futs.next().await {
                    if is_eof(&r) { eof = true; continue; }
                    let r = r.unwrap();
                    first.get_or_insert_with(|| t0.elapsed());
                    batches += 1;
                    names.extend(r.files.into_iter().map(|f| f.filename));
                    if !eof { futs.push_back(rd(&sftp, handle.clone())); }
                }
                let total = t0.elapsed();
                println!("{mode} first_ms={:.1} total_ms={:.0} entries={} batches={batches}", first.unwrap().as_secs_f64() * 1e3, total.as_secs_f64() * 1e3, names.len());
                // stat-per-entry cost
                if let Some(k) = a.get(5) {
                    let (k, conc) = k.split_once(':').map(|(k, c)| (k.parse::<usize>().unwrap(), c.parse::<usize>().unwrap())).unwrap();
                    let k = k.min(names.len());
                    let t = Instant::now();
                    use futures::StreamExt;
                    futures::stream::iter(names.iter().take(k).map(|n| { let p = format!("{dir}/{n}"); let s = &sftp; async move { s.lstat(p).await.unwrap() } }))
                        .buffered(conc).count().await;
                    let el = t.elapsed();
                    println!("lstat x{k} conc={conc}: {:.0} ms ({:.3} ms/entry)", el.as_secs_f64() * 1e3, el.as_secs_f64() * 1e3 / k as f64);
                }
            }
            println!("peak_rss_mb={:.0}", rss_mb());
        }
        "big" => {
            // big <path> hl|W:C  (W outstanding reads of C bytes each); LIMIT bytes via env
            let (path, mode) = (a[3].clone(), a[4].as_str());
            let limit: u64 = std::env::var("LIMIT").ok().map(|v| v.parse().unwrap()).unwrap_or(1 << 30);
            let t0 = Instant::now();
            let mut got = 0u64;
            if mode == "hl" {
                let cfg = Config::default();
                let ch = h.channel_open_session().await.unwrap();
                ch.request_subsystem(true, "sftp").await.unwrap();
                let s = SftpSession::new_with_config(ch.into_stream(), cfg).await.unwrap();
                let mut f = s.open(path).await.unwrap();
                let mut buf = vec![0u8; 1 << 20];
                while got < limit { let n = f.read(&mut buf).await.unwrap(); if n == 0 { break; } got += n as u64; }
            } else {
                let (w, c) = mode.split_once(':').map(|(w, c)| (w.parse::<usize>().unwrap(), c.parse::<u32>().unwrap())).unwrap();
                let s = raw(&h, Config::default()).await;
                let hd = s.open(path, OpenFlags::READ, Default::default()).await.unwrap().handle;
                use futures::StreamExt;
                let offs = (0..limit / c as u64).map(|i| i * c as u64);
                let mut st = futures::stream::iter(offs.map(|o| { let s = &s; let hd = hd.clone(); async move { s.read(hd, o, c).await } })).buffered(w);
                while let Some(r) = st.next().await { match r { Ok(d) => got += d.data.len() as u64, Err(_) => break } }
            }
            let el = t0.elapsed().as_secs_f64();
            println!("big {mode} {} MiB in {:.2}s = {:.0} MiB/s peak_rss_mb={:.0}", got >> 20, el, (got >> 20) as f64 / el, rss_mb());
        }
        "small" => {
            let (dir, count, conc): (String, usize, usize) = (a[3].clone(), a[4].parse().unwrap(), a[5].parse().unwrap());
            let s = raw(&h, Config::default()).await;
            let t0 = Instant::now();
            use futures::StreamExt;
            let bytes: usize = futures::stream::iter((1..=count).map(|i| { let s = &s; let p = format!("{dir}/s_{i}"); async move {
                let hd = s.open(p, OpenFlags::READ, Default::default()).await.unwrap().handle;
                let mut n = 0; let mut off = 0u64;
                loop { match s.read(hd.clone(), off, 32768).await { Ok(d) => { n += d.data.len(); off += d.data.len() as u64 } Err(_) => break } }
                s.close(hd).await.unwrap(); n } })).buffered(conc).fold(0, |a, b| async move { a + b }).await;
            let el = t0.elapsed().as_secs_f64();
            println!("small x{count} conc={conc}: {:.0} ms ({:.0} files/s, {} KiB)", el * 1e3, count as f64 / el, bytes >> 10);
        }
        _ => panic!("cmd"),
    }
    let _ = h.disconnect(Disconnect::ByApplication, "", "en").await;
}

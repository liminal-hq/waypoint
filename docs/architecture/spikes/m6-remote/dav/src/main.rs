// Throwaway spike: WebDAV PROPFIND listing, thin client (reqwest + quick-xml) vs reqwest_dav.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use futures_util::StreamExt;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::time::Instant;

fn rss_mb() -> f64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let l = s.lines().find(|l| l.starts_with("VmHWM")).unwrap();
    l.split_whitespace().nth(1).unwrap().parse::<f64>().unwrap() / 1024.0
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let base = format!("http://127.0.0.1:{}", a[1]);
    let path = a[2].clone();
    match a[3].as_str() {
        "thin" => {
            let c = reqwest::Client::new();
            let t0 = Instant::now();
            let resp = c.request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), format!("{base}{path}"))
                .basic_auth("scott", Some("spikepw")).header("Depth", "1").send().await.unwrap();
            let ttfb = t0.elapsed();
            // stream the body; count entries as they arrive (first-entry latency), then parse all
            let mut body = Vec::new();
            let mut first_entry = None;
            let mut st = resp.bytes_stream();
            while let Some(chunk) = st.next().await {
                let chunk = chunk.unwrap();
                body.extend_from_slice(&chunk);
                if first_entry.is_none() && body.windows(12).any(|w| w == b"<D:response>") && body.windows(12).filter(|w| *w == b"<D:response>").count() >= 2 {
                    first_entry = Some(t0.elapsed());
                }
            }
            let fetched = t0.elapsed();
            let mut r = Reader::from_reader(&body[..]);
            let (mut n, mut cur) = (0usize, String::new());
            let mut in_len = false;
            let mut total = 0u64;
            loop {
                match r.read_event().unwrap() {
                    Event::Start(e) if e.local_name().as_ref() == b"href" => { cur.clear(); n += 1; }
                    Event::Start(e) if e.local_name().as_ref() == b"getcontentlength" => in_len = true,
                    Event::Text(t) => { if in_len { total += t.decode().unwrap().parse::<u64>().unwrap_or(0); in_len = false; } }
                    Event::Eof => break,
                    _ => {}
                }
            }
            println!("thin ttfb_ms={:.1} first_entry_ms={:.1} fetched_ms={:.0} parsed_ms={:.0} entries={n} body_mb={:.1} peak_rss_mb={:.0}",
                ttfb.as_secs_f64()*1e3, first_entry.map(|d| d.as_secs_f64()*1e3).unwrap_or(-1.0), fetched.as_secs_f64()*1e3, t0.elapsed().as_secs_f64()*1e3, body.len() as f64/1e6, rss_mb());
            let _ = total;
        }
        "rdav" => {
            use reqwest_dav::{Auth, ClientBuilder, Depth};
            let c = ClientBuilder::new().set_host(base).set_auth(Auth::Basic("scott".into(), "spikepw".into())).build().unwrap();
            let t0 = Instant::now();
            let l = c.list(&path, Depth::Number(1)).await.unwrap();
            println!("reqwest_dav total_ms={:.0} entries={} peak_rss_mb={:.0}", t0.elapsed().as_secs_f64()*1e3, l.len(), rss_mb());
        }
        "range" => {
            let c = reqwest::Client::new();
            let t0 = Instant::now();
            let r = c.get(format!("{base}{path}")).basic_auth("scott", Some("spikepw")).header("Range", "bytes=1000-1999").send().await.unwrap();
            let (st, cr) = (r.status(), r.headers().get("content-range").cloned());
            let len = r.bytes().await.unwrap().len();
            println!("range status={st} content-range={cr:?} len={len} {:.1} ms", t0.elapsed().as_secs_f64()*1e3);
        }
        "big" => {
            let c = reqwest::Client::new();
            let t0 = Instant::now();
            let mut st = c.get(format!("{base}{path}")).basic_auth("scott", Some("spikepw")).send().await.unwrap().bytes_stream();
            let mut n = 0u64; while let Some(x) = st.next().await { n += x.unwrap().len() as u64; }
            let el = t0.elapsed().as_secs_f64();
            println!("big http {} MiB in {:.2}s = {:.0} MiB/s peak_rss_mb={:.0}", n >> 20, el, (n >> 20) as f64 / el, rss_mb());
        }
        _ => panic!(),
    }
}

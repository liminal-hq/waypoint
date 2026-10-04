// Throwaway spike: TCP proxy adding a fixed one-way delay in each direction.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
// usage: proxy <listen_port> <upstream_port> <one_way_delay_ms>
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{sleep_until, Instant};

async fn pump(mut r: tokio::net::tcp::OwnedReadHalf, mut w: tokio::net::tcp::OwnedWriteHalf, d: Duration) {
    let (tx, mut rx) = mpsc::unbounded_channel::<(Instant, Vec<u8>)>();
    let writer = tokio::spawn(async move {
        while let Some((at, buf)) = rx.recv().await {
            sleep_until(at).await;
            if w.write_all(&buf).await.is_err() { break; }
        }
        let _ = w.shutdown().await;
    });
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        match r.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => { if tx.send((Instant::now() + d, buf[..n].to_vec())).is_err() { break; } }
        }
    }
    drop(tx);
    let _ = writer.await;
}

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().collect();
    let l = TcpListener::bind(("127.0.0.1", a[1].parse::<u16>().unwrap())).await.unwrap();
    let up: u16 = a[2].parse().unwrap();
    let d = Duration::from_millis(a[3].parse().unwrap());
    loop {
        let (c, _) = l.accept().await.unwrap();
        c.set_nodelay(true).ok();
        tokio::spawn(async move {
            let s = TcpStream::connect(("127.0.0.1", up)).await.unwrap();
            s.set_nodelay(true).ok();
            let (cr, cw) = c.into_split();
            let (sr, sw) = s.into_split();
            tokio::join!(pump(cr, sw, d), pump(sr, cw, d));
        });
    }
}

// A server that answers each request with the next canned response, for replaying recorded bodies.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Canned {
    pub port: u16,
    /// The request line and the signing region of each request, in order.
    pub requests: Arc<Mutex<Vec<(String, String)>>>,
}

/// One canned response: status, extra headers and body.
pub type Response = (u16, Vec<(String, String)>, String);

impl Canned {
    pub fn start(responses: Vec<Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        thread::spawn(move || {
            let mut responses = responses.into_iter();
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let log = log.clone();
                // One connection may carry several requests; answer them in order.
                loop {
                    let mut buf = Vec::new();
                    let mut byte = [0u8; 1];
                    while !buf.ends_with(b"\r\n\r\n") {
                        if stream.read(&mut byte).unwrap_or(0) == 0 {
                            break;
                        }
                        buf.push(byte[0]);
                    }
                    if buf.is_empty() {
                        break;
                    }
                    let head = String::from_utf8_lossy(&buf).into_owned();
                    let line = head.lines().next().unwrap_or("").to_owned();
                    let region = head
                        .split("Credential=")
                        .nth(1)
                        .and_then(|c| c.split('/').nth(2))
                        .unwrap_or("")
                        .to_owned();
                    let length: usize = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    let mut body = vec![0u8; length];
                    let _ = stream.read_exact(&mut body);
                    log.lock().unwrap().push((line.clone(), region));
                    let Some((status, headers, body)) = responses.next() else {
                        return;
                    };
                    let mut out = format!("HTTP/1.1 {status} X\r\n");
                    let has_length = headers
                        .iter()
                        .any(|(k, _)| k.eq_ignore_ascii_case("content-length"));
                    for (k, v) in &headers {
                        out.push_str(&format!("{k}: {v}\r\n"));
                    }
                    if !has_length {
                        out.push_str(&format!("content-length: {}\r\n", body.len()));
                    }
                    out.push_str("\r\n");
                    let _ = stream.write_all(out.as_bytes());
                    if !line.starts_with("HEAD") {
                        let _ = stream.write_all(body.as_bytes());
                    }
                }
            }
        });
        Self { port, requests }
    }

    pub fn lines(&self) -> Vec<(String, String)> {
        self.requests.lock().unwrap().clone()
    }
}

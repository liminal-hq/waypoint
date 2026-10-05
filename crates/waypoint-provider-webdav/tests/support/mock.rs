// A scripted HTTP server for the tests of what a real server will not do on demand: refuse, slow
// down, challenge, break off mid-body, redirect, and present a certificate nobody trusts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A request as the server read it.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

pub type Writer<'a> = &'a mut dyn Write;

/// What a reply carries.
pub enum Body {
    Bytes(Vec<u8>),
    /// Written by the closure as the connection stays open; the connection closes after.
    Stream(Box<dyn FnOnce(Writer) -> io::Result<()> + Send>),
    /// Announces `announced` bytes, sends these and closes: a connection lost mid-body.
    Truncated {
        announced: usize,
        sent: Vec<u8>,
    },
}

pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl Reply {
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Body::Bytes(Vec::new()),
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Body::Bytes(body.into());
        self
    }

    pub fn xml(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self::new(status)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
    }
}

/// The handler gets each request and its number (from 0), and answers it.
pub type Handler = dyn Fn(&Seen, usize) -> Reply + Send + Sync;

pub struct Mock {
    pub port: u16,
    pub seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

impl Mock {
    pub fn start(handler: impl Fn(&Seen, usize) -> Reply + Send + Sync + 'static) -> Mock {
        Self::run(Arc::new(handler), None)
    }

    pub fn start_tls(
        config: Arc<rustls::ServerConfig>,
        handler: impl Fn(&Seen, usize) -> Reply + Send + Sync + 'static,
    ) -> Mock {
        Self::run(Arc::new(handler), Some(config))
    }

    fn run(handler: Arc<Handler>, tls: Option<Arc<rustls::ServerConfig>>) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicUsize::new(0));
        let mock = Mock {
            port,
            seen: seen.clone(),
            stop: stop.clone(),
        };
        thread::spawn(move || {
            for stream in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(stream) = stream else { continue };
                let (handler, seen, count, tls) =
                    (handler.clone(), seen.clone(), count.clone(), tls.clone());
                thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                    match tls {
                        Some(config) => {
                            let Ok(conn) = rustls::ServerConnection::new(config) else {
                                return;
                            };
                            serve(
                                rustls::StreamOwned::new(conn, stream),
                                &handler,
                                &seen,
                                &count,
                            )
                        }
                        None => serve(stream, &handler, &seen, &count),
                    }
                });
            }
        });
        mock
    }

    pub fn requests(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }

    pub fn url(&self) -> String {
        format!("dav://127.0.0.1:{}", self.port)
    }
}

fn serve<S: Read + Write>(
    stream: S,
    handler: &Arc<Handler>,
    seen: &Mutex<Vec<Seen>>,
    count: &AtomicUsize,
) {
    let mut reader = BufReader::new(stream);
    loop {
        let Some(request) = read_request(&mut reader) else {
            return;
        };
        let n = count.fetch_add(1, Ordering::SeqCst);
        seen.lock().unwrap().push(request.clone());
        let reply = handler(&request, n);
        if respond(reader.get_mut(), reply).is_err() {
            return;
        }
    }
}

fn read_request<S: Read>(reader: &mut BufReader<S>) -> Option<Seen> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next()?.to_owned(), parts.next()?.to_owned());
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        let (name, value) = header.split_once(':')?;
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Seen {
        method,
        target,
        headers,
        body,
    })
}

fn respond<S: Write>(out: &mut S, reply: Reply) -> io::Result<()> {
    let mut head = format!("HTTP/1.1 {} Mock\r\n", reply.status);
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    match reply.body {
        Body::Bytes(bytes) => {
            head.push_str(&format!("Content-Length: {}\r\n\r\n", bytes.len()));
            out.write_all(head.as_bytes())?;
            out.write_all(&bytes)?;
            out.flush()
        }
        Body::Stream(write) => {
            head.push_str("Connection: close\r\n\r\n");
            out.write_all(head.as_bytes())?;
            write(out)?;
            out.flush()?;
            Err(io::Error::other("closed after a streamed body"))
        }
        Body::Truncated { announced, sent } => {
            head.push_str(&format!("Content-Length: {announced}\r\n\r\n"));
            out.write_all(head.as_bytes())?;
            out.write_all(&sent)?;
            out.flush()?;
            Err(io::Error::other("closed mid-body"))
        }
    }
}

/// A multistatus of `names`, each a file of `size` bytes in `folder`, preceded by the folder.
pub fn multistatus(folder: &str, names: impl Iterator<Item = String>) -> String {
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?><D:multistatus xmlns:D=\"DAV:\">");
    out.push_str(&format!(
        "<D:response><D:href>{folder}</D:href><D:propstat><D:prop><D:resourcetype><D:collection/>\
         </D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>"
    ));
    for name in names {
        out.push_str(&entry_xml(folder, &name));
    }
    out.push_str("</D:multistatus>");
    out
}

pub fn entry_xml(folder: &str, name: &str) -> String {
    format!(
        "<D:response><D:href>{folder}{name}</D:href><D:propstat><D:prop><D:resourcetype/>\
         <D:getcontentlength>10</D:getcontentlength>\
         <D:getlastmodified>Sun, 06 Nov 1994 08:49:37 GMT</D:getlastmodified>\
         <D:getetag>\"{name}\"</D:getetag></D:prop><D:status>HTTP/1.1 200 OK</D:status>\
         </D:propstat></D:response>"
    )
}

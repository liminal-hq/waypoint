// `GET`: streaming reads from an offset, resumed where they were if the connection drops.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{EntryKind, ReadStream};

use crate::client::{Request, Session};
use crate::errors::{from_status, from_transport, Op};
use crate::provider::WebDavProvider;
use crate::stream::{ChannelReader, Chunk};
use crate::xml::Props;

/// How often a read that broke off asks the server for the rest.
const RESUMES: usize = 2;

/// A strong entity tag, which `If-Match` may carry. A weak one (`W/"…"`) says the bytes may differ.
pub(crate) fn strong(etag: &str) -> bool {
    !etag.trim_start().starts_with("W/")
}

/// The precondition that keeps a change to what was seen: `If-Match` with a strong entity tag, or,
/// where the server's tag is weak (Apache's is for a second after a write) or missing,
/// `If-Unmodified-Since` with the modification time as the server wrote it.
pub(crate) fn guard(props: &Props) -> Option<(&'static str, String)> {
    match (&props.etag, &props.modified) {
        (Some(etag), _) if strong(etag) => Some(("If-Match", etag.clone())),
        (_, Some(modified)) => Some(("If-Unmodified-Since", modified.clone())),
        _ => None,
    }
}

fn get(url: &str, from: u64, etag: Option<&str>) -> Request {
    let mut request = Request::new("GET", url.to_owned()).header("Accept-Encoding", "identity");
    if from > 0 {
        request = request.header("Range", format!("bytes={from}-"));
    }
    if let Some(etag) = etag.filter(|etag| strong(etag)) {
        request = request.header("If-Match", etag);
    }
    request
}

impl WebDavProvider {
    pub(crate) fn dav_open_read(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        let target = self.target(path)?;
        let url = target.url(false);
        let reply = self.exec_raw(&target, &get(&url, start, None), None)?;
        let status = reply.status();
        match status.as_u16() {
            200 | 206 => {}
            // The start is past the end.
            416 => return Ok(Box::new(std::io::empty())),
            // A server answers a `GET` of a folder with a refusal of one kind or another (Apache's
            // is a `404`).
            403..=405 => {
                return Err(match self.dav_stat(path) {
                    Ok((entry, _)) if entry.kind == EntryKind::Directory => {
                        VfsError::IsADirectory {
                            location: target.location,
                        }
                    }
                    _ => from_status(Op::Read, status, None, &target.location),
                })
            }
            _ => return Err(from_status(Op::Read, status, None, &target.location)),
        }
        // Some servers answer a `GET` of a folder with an index page.
        let html = reply.header("content-type").is_some_and(|kind| {
            kind.trim_start()
                .to_ascii_lowercase()
                .starts_with("text/html")
        });
        if html {
            if let Ok((entry, _)) = self.dav_stat(path) {
                if entry.kind == EntryKind::Directory {
                    return Err(VfsError::IsADirectory {
                        location: target.location,
                    });
                }
            }
        }
        let etag = reply.header("etag").map(str::to_owned);
        // A server that ignored the range sent the file from its start.
        let position = if status.as_u16() == 200 { 0 } else { start };
        let response = reply.into_response();
        let (send, chunks) = ChannelReader::channel();
        let (reader, _) = ChannelReader::new(chunks);
        let task = Resumable {
            session: target.session.clone(),
            url,
            location: target.location.clone(),
            etag,
            start,
            position,
            timeout: target.session.options.timeout,
            // The runtime lives as long as a stream reads from it.
            _provider: self.clone(),
        };
        self.runtime().spawn(task.run(response, send));
        Ok(Box::new(reader) as Box<dyn Read + Send>)
    }
}

struct Resumable {
    session: Arc<Session>,
    url: String,
    location: Location,
    etag: Option<String>,
    /// Where the caller wants to read from.
    start: u64,
    /// The file offset of the next byte the response will send.
    position: u64,
    timeout: Duration,
    _provider: WebDavProvider,
}

impl Resumable {
    async fn run(mut self, mut response: reqwest::Response, send: mpsc::Sender<Chunk>) {
        let mut resumes = 0;
        loop {
            let failure = match tokio::time::timeout(self.timeout, response.chunk()).await {
                Ok(Ok(Some(chunk))) => {
                    let begins = self.position;
                    self.position += chunk.len() as u64;
                    // Bytes before the start are the skipped part of a response that began early.
                    let chunk = if self.position <= self.start {
                        continue;
                    } else if begins < self.start {
                        chunk.slice((self.start - begins) as usize..)
                    } else {
                        chunk
                    };
                    if send.send(Ok(chunk)).await.is_err() {
                        return;
                    }
                    continue;
                }
                Ok(Ok(None)) => return,
                Ok(Err(error)) => from_transport(&error, &self.location),
                Err(_) => VfsError::Timeout {
                    location: self.location.clone(),
                },
            };
            // The connection broke off: ask for the rest, as long as the file is the same one.
            resumes += 1;
            let next = if resumes <= RESUMES {
                self.reopen().await
            } else {
                None
            };
            match next {
                Some(next) => response = next,
                None => {
                    let _ = send.send(Err(failure)).await;
                    return;
                }
            }
        }
    }

    async fn reopen(&self) -> Option<reqwest::Response> {
        // Without an entity tag there is nothing to say the bytes are the same ones.
        let etag = self.etag.as_deref().filter(|etag| strong(etag))?;
        let request = get(&self.url, self.position, Some(etag));
        let reply = self
            .session
            .send(&request, &self.location, None)
            .await
            .ok()?;
        (reply.status().as_u16() == 206).then(|| reply.into_response())
    }
}

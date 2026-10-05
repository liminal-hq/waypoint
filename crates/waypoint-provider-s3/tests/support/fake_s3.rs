// A small S3 service in memory, for the tests the real servers on hand cannot serve: folder marker
// objects, multipart uploads (with the abort that must follow a failure), conditional writes, storage
// classes, region redirects and injected failures.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! It speaks path-style S3 over HTTP/1.1 on a loopback port and checks nothing about signatures
//! (it only reads the signing region from the `Authorization` header). Answers follow the shapes of
//! real S3: a missing key is a `404` with `NoSuchKey` (a `HEAD` has no body), a listing's keys are
//! URL-encoded when the request asks, `max-keys` is honoured with continuation tokens, and an
//! object in `GLACIER` or `DEEP_ARCHIVE` answers a `GET` with `InvalidObjectState`.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone)]
pub struct Object {
    pub data: Vec<u8>,
    pub class: String,
    pub modified: i64,
}

pub struct Upload {
    pub bucket: String,
    pub key: String,
    pub parts: BTreeMap<i32, Vec<u8>>,
}

/// A failure to answer with instead of the real answer.
#[derive(Clone)]
pub struct Injected {
    pub status: u16,
    pub code: &'static str,
    pub retry_after: Option<u64>,
    /// Only requests whose line contains this (`PUT`, `list-type`).
    pub when: &'static str,
    pub times: usize,
}

#[derive(Default)]
pub struct State {
    pub buckets: BTreeMap<String, BTreeMap<String, Object>>,
    pub uploads: HashMap<String, Upload>,
    /// Every request line, in order.
    pub log: Vec<String>,
    /// The uploads that were aborted, as `bucket/key`.
    pub aborted: Vec<String>,
    /// The signing regions requests used, in order.
    pub regions: Vec<String>,
    /// Buckets that live in a region: a request signed for another gets a redirect.
    pub bucket_regions: HashMap<String, String>,
    pub injected: Vec<Injected>,
    next_upload: usize,
}

pub struct FakeS3 {
    pub port: u16,
    pub state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeS3 {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (s, st) = (state.clone(), stop.clone());
        let thread = thread::spawn(move || {
            while !st.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let s = s.clone();
                        thread::spawn(move || serve(stream, s));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            port,
            state,
            stop,
            thread: Some(thread),
        }
    }

    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn bucket(&self, name: &str) {
        self.state
            .lock()
            .unwrap()
            .buckets
            .entry(name.to_owned())
            .or_default();
    }

    pub fn put(&self, bucket: &str, key: &str, data: &[u8], class: &str) {
        self.state
            .lock()
            .unwrap()
            .buckets
            .entry(bucket.to_owned())
            .or_default()
            .insert(
                key.to_owned(),
                Object {
                    data: data.to_vec(),
                    class: class.to_owned(),
                    modified: 1_700_000_000,
                },
            );
    }

    pub fn get(&self, bucket: &str, key: &str) -> Option<Vec<u8>> {
        self.state
            .lock()
            .unwrap()
            .buckets
            .get(bucket)?
            .get(key)
            .map(|o| o.data.clone())
    }

    pub fn keys(&self, bucket: &str) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .buckets
            .get(bucket)
            .map(|b| b.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn inject(&self, failure: Injected) {
        self.state.lock().unwrap().injected.push(failure);
    }

    pub fn log(&self) -> Vec<String> {
        self.state.lock().unwrap().log.clone()
    }

    pub fn open_uploads(&self) -> usize {
        self.state.lock().unwrap().uploads.len()
    }
}

impl Drop for FakeS3 {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    line: String,
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).ok()? == 0 {
            return None;
        }
        buf.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&buf).into_owned();
    let mut lines = head.split("\r\n");
    let line = lines.next()?.to_owned();
    let mut parts = line.split(' ');
    let method = parts.next()?.to_owned();
    let target = parts.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(k), decode(v))
        })
        .collect();
    let headers: HashMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).ok()?;
    Some(Request {
        method,
        path: decode(path),
        query,
        headers,
        body,
        line,
    })
}

fn civil(secs: i64) -> (i64, u32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    let weekday = ((days + 4).rem_euclid(7)) as u32;
    (
        y,
        m,
        d,
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
        weekday,
    )
}

fn iso(secs: i64) -> String {
    let (y, m, d, h, mi, s, _) = civil(secs);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.000Z")
}

fn http_date(secs: i64) -> String {
    let (y, m, d, h, mi, s, wd) = civil(secs);
    let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{}, {d:02} {} {y:04} {h:02}:{mi:02}:{s:02} GMT",
        days[wd as usize],
        months[m as usize - 1]
    )
}

fn etag(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!("\"{hash:016x}{:016x}\"", data.len())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn xml(status: u16, body: String) -> Reply {
    Reply {
        status,
        headers: vec![("content-type".into(), "application/xml".into())],
        body: format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{body}").into_bytes(),
    }
}

fn error(status: u16, code: &str, message: &str) -> Reply {
    xml(status, format!("<Error><Code>{code}</Code><Message>{message}</Message><RequestId>FAKE</RequestId></Error>"))
}

fn empty(status: u16) -> Reply {
    Reply {
        status,
        headers: Vec::new(),
        body: Vec::new(),
    }
}

fn serve(mut stream: TcpStream, state: Arc<Mutex<State>>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    while let Some(request) = read_request(&mut stream) {
        let head = request.method == "HEAD";
        let reply = handle(&request, &state);
        let reason = match reply.status {
            200 => "OK",
            204 => "No Content",
            206 => "Partial Content",
            301 => "Moved Permanently",
            403 => "Forbidden",
            404 => "Not Found",
            412 => "Precondition Failed",
            416 => "Range Not Satisfiable",
            503 => "Service Unavailable",
            _ => "Status",
        };
        let mut out = format!("HTTP/1.1 {} {reason}\r\n", reply.status);
        let mut has_length = false;
        for (k, v) in &reply.headers {
            has_length |= k.eq_ignore_ascii_case("content-length");
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        if !has_length {
            out.push_str(&format!("content-length: {}\r\n", reply.body.len()));
        }
        out.push_str("x-amz-request-id: FAKE\r\n\r\n");
        if stream.write_all(out.as_bytes()).is_err() {
            return;
        }
        if !head && stream.write_all(&reply.body).is_err() {
            return;
        }
    }
}

fn handle(request: &Request, state: &Arc<Mutex<State>>) -> Reply {
    let mut state = state.lock().unwrap();
    state
        .log
        .push(request.line.trim_end_matches(" HTTP/1.1").to_owned());
    if let Some(auth) = request.headers.get("authorization") {
        if let Some(region) = auth
            .split("Credential=")
            .nth(1)
            .and_then(|c| c.split('/').nth(2))
        {
            state.regions.push(region.to_owned());
        }
    }
    if let Some(at) = state
        .injected
        .iter()
        .position(|i| request.line.contains(i.when))
    {
        let injected = state.injected[at].clone();
        state.injected[at].times -= 1;
        if state.injected[at].times == 0 {
            state.injected.remove(at);
        }
        let mut reply = error(injected.status, injected.code, "injected");
        if let Some(secs) = injected.retry_after {
            reply.headers.push(("retry-after".into(), secs.to_string()));
        }
        return reply;
    }
    let trimmed = request.path.trim_start_matches('/');
    let (bucket, key) = match trimmed.split_once('/') {
        Some((bucket, key)) => (bucket.to_owned(), key.to_owned()),
        None => (trimmed.to_owned(), String::new()),
    };
    let query = |name: &str| {
        request
            .query
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    let has = |name: &str| request.query.iter().any(|(k, _)| k == name);
    if bucket.is_empty() {
        let names: String = state
            .buckets
            .keys()
            .map(|b| {
                format!(
                    "<Bucket><Name>{}</Name><CreationDate>{}</CreationDate></Bucket>",
                    escape(b),
                    iso(1_700_000_000)
                )
            })
            .collect();
        return xml(200, format!("<ListAllMyBucketsResult><Owner><ID>x</ID></Owner><Buckets>{names}</Buckets></ListAllMyBucketsResult>"));
    }
    if let Some(region) = state.bucket_regions.get(&bucket).cloned() {
        if state.regions.last() != Some(&region) {
            let mut reply = error(301, "PermanentRedirect", "The bucket you are attempting to access must be addressed using the specified endpoint.");
            reply.headers.push(("x-amz-bucket-region".into(), region));
            return reply;
        }
    }
    if !state.buckets.contains_key(&bucket) {
        if request.method == "PUT" && key.is_empty() {
            state.buckets.entry(bucket).or_default();
            return empty(200);
        }
        return error(404, "NoSuchBucket", "The specified bucket does not exist");
    }
    if key.is_empty() {
        return match request.method.as_str() {
            "HEAD" => {
                let mut reply = empty(200);
                reply.headers.push((
                    "x-amz-bucket-region".into(),
                    state
                        .bucket_regions
                        .get(&bucket)
                        .cloned()
                        .unwrap_or_else(|| "us-east-1".into()),
                ));
                reply
            }
            "GET" => list(&state, &bucket, &query),
            _ => error(405, "MethodNotAllowed", "no"),
        };
    }
    let upload_id = query("uploadId");
    match (request.method.as_str(), upload_id) {
        ("POST", None) if has("uploads") => {
            state.next_upload += 1;
            let id = format!("upload-{}", state.next_upload);
            state.uploads.insert(
                id.clone(),
                Upload {
                    bucket: bucket.clone(),
                    key: key.clone(),
                    parts: BTreeMap::new(),
                },
            );
            xml(200, format!("<InitiateMultipartUploadResult><Bucket>{}</Bucket><Key>{}</Key><UploadId>{id}</UploadId></InitiateMultipartUploadResult>", escape(&bucket), escape(&key)))
        }
        ("PUT", Some(id)) => {
            let number: i32 = query("partNumber")
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            let data = match request.headers.get("x-amz-copy-source") {
                Some(source) => match copy_source(
                    &state,
                    source,
                    request.headers.get("x-amz-copy-source-range"),
                ) {
                    Ok(data) => data,
                    Err(reply) => return reply,
                },
                None => request.body.clone(),
            };
            let tag = etag(&data);
            let Some(upload) = state.uploads.get_mut(&id) else {
                return error(404, "NoSuchUpload", "no such upload");
            };
            upload.parts.insert(number, data);
            if request.headers.contains_key("x-amz-copy-source") {
                xml(200, format!("<CopyPartResult><ETag>{tag}</ETag><LastModified>{}</LastModified></CopyPartResult>", iso(1_700_000_000)))
            } else {
                Reply {
                    status: 200,
                    headers: vec![("etag".into(), tag)],
                    body: Vec::new(),
                }
            }
        }
        ("POST", Some(id)) => {
            let Some(upload) = state.uploads.get(&id) else {
                return error(404, "NoSuchUpload", "no such upload");
            };
            let data: Vec<u8> = upload.parts.values().flatten().copied().collect();
            if request
                .headers
                .get("if-none-match")
                .is_some_and(|v| v == "*")
                && state.buckets[&bucket].contains_key(&key)
            {
                return error(
                    412,
                    "PreconditionFailed",
                    "At least one of the pre-conditions you specified did not hold",
                );
            }
            let tag = etag(&data);
            state.uploads.remove(&id);
            state.buckets.get_mut(&bucket).unwrap().insert(
                key.clone(),
                Object {
                    data,
                    class: "STANDARD".into(),
                    modified: 1_700_000_100,
                },
            );
            xml(200, format!("<CompleteMultipartUploadResult><Bucket>{}</Bucket><Key>{}</Key><ETag>{tag}</ETag></CompleteMultipartUploadResult>", escape(&bucket), escape(&key)))
        }
        ("DELETE", Some(id)) => {
            state.uploads.remove(&id);
            state.aborted.push(format!("{bucket}/{key}"));
            empty(204)
        }
        ("PUT", None) => {
            if request
                .headers
                .get("if-none-match")
                .is_some_and(|v| v == "*")
                && state.buckets[&bucket].contains_key(&key)
            {
                return error(
                    412,
                    "PreconditionFailed",
                    "At least one of the pre-conditions you specified did not hold",
                );
            }
            let (data, class) = match request.headers.get("x-amz-copy-source") {
                Some(source) => match copy_source(&state, source, None) {
                    Ok(data) => (data, "STANDARD".to_owned()),
                    Err(reply) => return reply,
                },
                None => (
                    request.body.clone(),
                    request
                        .headers
                        .get("x-amz-storage-class")
                        .cloned()
                        .unwrap_or_else(|| "STANDARD".into()),
                ),
            };
            let tag = etag(&data);
            let copied = request.headers.contains_key("x-amz-copy-source");
            state.buckets.get_mut(&bucket).unwrap().insert(
                key,
                Object {
                    data,
                    class,
                    modified: 1_700_000_100,
                },
            );
            if copied {
                xml(200, format!("<CopyObjectResult><ETag>{tag}</ETag><LastModified>{}</LastModified></CopyObjectResult>", iso(1_700_000_100)))
            } else {
                Reply {
                    status: 200,
                    headers: vec![("etag".into(), tag)],
                    body: Vec::new(),
                }
            }
        }
        ("DELETE", None) => {
            state.buckets.get_mut(&bucket).unwrap().remove(&key);
            empty(204)
        }
        ("HEAD" | "GET", None) => {
            let Some(object) = state.buckets[&bucket].get(&key).cloned() else {
                return if request.method == "HEAD" {
                    empty(404)
                } else {
                    error(404, "NoSuchKey", "The specified key does not exist.")
                };
            };
            let mut headers = vec![
                ("etag".to_owned(), etag(&object.data)),
                ("last-modified".to_owned(), http_date(object.modified)),
                ("accept-ranges".to_owned(), "bytes".to_owned()),
            ];
            if object.class != "STANDARD" {
                headers.push(("x-amz-storage-class".into(), object.class.clone()));
            }
            if request.method == "GET"
                && (object.class == "GLACIER" || object.class == "DEEP_ARCHIVE")
            {
                return error(
                    403,
                    "InvalidObjectState",
                    "The operation is not valid for the object's storage class",
                );
            }
            let size = object.data.len();
            let (status, body) = match request
                .headers
                .get("range")
                .and_then(|r| r.strip_prefix("bytes="))
            {
                Some(range) => {
                    let start: usize = range
                        .split('-')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    if start >= size {
                        return error(
                            416,
                            "InvalidRange",
                            "The requested range is not satisfiable",
                        );
                    }
                    headers.push((
                        "content-range".into(),
                        format!("bytes {start}-{}/{size}", size - 1),
                    ));
                    (206, object.data[start..].to_vec())
                }
                None => (200, object.data.clone()),
            };
            if request.method == "HEAD" {
                headers.push(("content-length".into(), body.len().to_string()));
                return Reply {
                    status,
                    headers,
                    body: Vec::new(),
                };
            }
            Reply {
                status,
                headers,
                body,
            }
        }
        _ => error(405, "MethodNotAllowed", "no"),
    }
}

fn copy_source(state: &State, source: &str, range: Option<&String>) -> Result<Vec<u8>, Reply> {
    let source = decode(source);
    let source = source.trim_start_matches('/');
    let (bucket, key) = source.split_once('/').unwrap_or((source, ""));
    let Some(object) = state.buckets.get(bucket).and_then(|b| b.get(key)) else {
        return Err(error(404, "NoSuchKey", "The specified key does not exist."));
    };
    if object.class == "GLACIER" || object.class == "DEEP_ARCHIVE" {
        return Err(error(
            403,
            "InvalidObjectState",
            "Operation is not valid for the source object's storage class",
        ));
    }
    match range.and_then(|r| r.strip_prefix("bytes=")) {
        Some(range) => {
            let (start, end) = range.split_once('-').unwrap_or(("0", "0"));
            let (start, end): (usize, usize) =
                (start.parse().unwrap_or(0), end.parse().unwrap_or(0));
            Ok(object.data[start..=end.min(object.data.len() - 1)].to_vec())
        }
        None => Ok(object.data.clone()),
    }
}

fn list(state: &State, bucket: &str, query: &dyn Fn(&str) -> Option<String>) -> Reply {
    let prefix = query("prefix").unwrap_or_default();
    let delimiter = query("delimiter").unwrap_or_default();
    let max: usize = query("max-keys")
        .and_then(|m| m.parse().ok())
        .unwrap_or(1000)
        .min(1000);
    let after = query("continuation-token")
        .or_else(|| query("start-after"))
        .unwrap_or_default();
    let url = query("encoding-type").as_deref() == Some("url");
    let enc = |text: &str| if url { encode(text) } else { escape(text) };
    let mut contents = String::new();
    let mut commons = String::new();
    let mut seen_prefixes: Vec<String> = Vec::new();
    let mut count = 0;
    let mut last = String::new();
    let mut truncated = false;
    for (key, object) in &state.buckets[bucket] {
        if !key.starts_with(&prefix) || key.as_str() <= after.as_str() {
            continue;
        }
        let rest = &key[prefix.len()..];
        let common = (!delimiter.is_empty())
            .then(|| {
                rest.find(&delimiter)
                    .map(|at| format!("{prefix}{}", &rest[..at + delimiter.len()]))
            })
            .flatten();
        if let Some(common) = &common {
            if seen_prefixes.contains(common) {
                last = key.clone();
                continue;
            }
        }
        if count == max {
            truncated = true;
            break;
        }
        count += 1;
        last = key.clone();
        match common {
            Some(common) => {
                commons.push_str(&format!("<CommonPrefixes><Prefix>{}</Prefix></CommonPrefixes>", enc(&common)));
                // The next page starts after everything under this prefix.
                last = format!("{common}\u{10FFFF}");
                seen_prefixes.push(common);
            }
            None => contents.push_str(&format!(
                "<Contents><Key>{}</Key><LastModified>{}</LastModified><ETag>{}</ETag><Size>{}</Size><StorageClass>{}</StorageClass></Contents>",
                enc(key), iso(object.modified), etag(&object.data), object.data.len(), object.class
            )),
        }
    }
    let token = if truncated {
        format!(
            "<NextContinuationToken>{}</NextContinuationToken>",
            escape(&last)
        )
    } else {
        String::new()
    };
    xml(200, format!(
        "<ListBucketResult><Name>{bucket}</Name><Prefix>{}</Prefix><KeyCount>{count}</KeyCount><MaxKeys>{max}</MaxKeys><Delimiter>{}</Delimiter>{}<IsTruncated>{truncated}</IsTruncated>{token}{contents}{commons}</ListBucketResult>",
        enc(&prefix), escape(&delimiter), if url { "<EncodingType>url</EncodingType>" } else { "" }
    ))
}

// The requests of the provider as asynchronous functions: stat, listing pages, folders, removal,
// server-side copy and the bucket list.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;

use aws_sdk_s3::primitives::DateTime;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, EncodingType};
use waypoint_protocol::VfsError;
use waypoint_vfs::{group_for_scan, CancelToken, EntryKind, ScannedEntry};

use crate::address::Address;
use crate::errors::S3Error;
use crate::options::MAX_SINGLE_COPY;
use crate::service::{cancellable, Failed, Inner};
use crate::storage_class::{ObjectAttributes, StorageClass};

/// Entries handed over at once after the first page (A18's batches are 2,000 to 5,000).
const BATCH: usize = 2_000;
/// Keys the service returns per page: the most it allows.
const PAGE: i32 = 1_000;
/// The size of one part of a multipart copy.
const COPY_PART: u64 = 256 * 1024 * 1024;

/// An entry of a listing with what S3 adds to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedEntry {
    pub entry: ScannedEntry,
    /// The attributes of an object; `None` for a folder.
    pub attributes: Option<ObjectAttributes>,
}

/// A bucket of a service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketInfo {
    pub name: String,
    pub created_ms: Option<i64>,
}

/// What HEAD says about an object.
#[derive(Debug, Clone)]
pub(crate) struct Meta {
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub attributes: ObjectAttributes,
}

fn millis(time: Option<&DateTime>) -> Option<i64> {
    time.and_then(|time| time.to_millis().ok())
}

pub(crate) fn file_entry(name: &str, size: u64, modified_ms: Option<i64>) -> ScannedEntry {
    ScannedEntry {
        name: OsString::from(name),
        kind: EntryKind::File,
        link_target: None,
        link_pending: false,
        group: group_for_scan(name.as_bytes(), EntryKind::File, None, false, false),
        special: None,
        size: Some(size),
        modified_ms,
        hidden: name.starts_with('.'),
        trashed: None,
    }
}

pub(crate) fn dir_entry(name: &str, modified_ms: Option<i64>) -> ScannedEntry {
    ScannedEntry {
        name: OsString::from(name),
        kind: EntryKind::Directory,
        link_target: None,
        link_pending: false,
        group: group_for_scan(name.as_bytes(), EntryKind::Directory, None, false, false),
        special: None,
        size: None,
        modified_ms,
        hidden: name.starts_with('.'),
        trashed: None,
    }
}

/// A key a listing sent URL-encoded: `%XX` escapes, and `+` for a space.
pub(crate) fn url_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 3 <= bytes.len()
                && text.is_char_boundary(i + 1)
                && text.is_char_boundary(i + 3) =>
            {
                match u8::from_str_radix(&text[i + 1..i + 3], 16) {
                    Ok(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        i += 1;
                    }
                }
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

/// The `x-amz-copy-source` of an object: `bucket/key` with the key percent-encoded.
pub(crate) fn copy_source(bucket: &str, key: &str) -> String {
    let mut out = String::with_capacity(bucket.len() + key.len() + 1);
    out.push_str(bucket);
    out.push('/');
    for byte in key.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

impl Inner {
    /// Whether the bucket is there and may be listed (one key asked for).
    async fn probe_bucket(&self, address: &Address) -> Result<(), Failed> {
        let bucket = address.bucket.clone();
        self.run(address, |client| {
            let bucket = bucket.clone();
            async move {
                client
                    .list_objects_v2()
                    .bucket(bucket)
                    .max_keys(1)
                    .send()
                    .await
            }
        })
        .await
        .map(|_| ())
    }

    /// `HEAD` of the object at the key; `None` when there is none.
    pub(crate) async fn head(&self, address: &Address) -> Result<Option<Meta>, Failed> {
        let (bucket, key) = (address.bucket.clone(), address.key.clone());
        let result = self
            .run(address, |client| {
                let (bucket, key) = (bucket.clone(), key.clone());
                async move { client.head_object().bucket(bucket).key(key).send().await }
            })
            .await;
        match result {
            Ok(head) => Ok(Some(Meta {
                size: head.content_length().unwrap_or(0).max(0) as u64,
                modified_ms: millis(head.last_modified()),
                attributes: ObjectAttributes {
                    storage_class: head
                        .storage_class()
                        .map_or(StorageClass::Standard, |class| {
                            StorageClass::from_api_name(class.as_str())
                        }),
                    etag: head.e_tag().map(str::to_owned),
                },
            })),
            Err(failed) if failed.is(&S3Error::NoSuchKey) => Ok(None),
            Err(failed) => Err(failed),
        }
    }

    /// Whether anything has the key plus a `/` as a prefix: a folder with content, or its marker.
    pub(crate) async fn has_children(&self, address: &Address) -> Result<bool, Failed> {
        Ok(!self.first_keys(address, 1).await?.is_empty())
    }

    /// Up to `count` keys under the folder's prefix, with no delimiter.
    pub(crate) async fn first_keys(
        &self,
        address: &Address,
        count: i32,
    ) -> Result<Vec<String>, Failed> {
        let (bucket, prefix) = (address.bucket.clone(), address.prefix());
        let page = self
            .run(address, |client| {
                let (bucket, prefix) = (bucket.clone(), prefix.clone());
                async move {
                    client
                        .list_objects_v2()
                        .bucket(bucket)
                        .prefix(prefix)
                        .max_keys(count)
                        .send()
                        .await
                }
            })
            .await?;
        Ok(page
            .contents()
            .iter()
            .filter_map(|object| object.key().map(str::to_owned))
            .collect())
    }

    /// Whether anything is at the address, as a file or as a folder.
    pub(crate) async fn exists(&self, address: &Address) -> Result<bool, Failed> {
        if address.is_bucket() {
            self.probe_bucket(address).await?;
            return Ok(true);
        }
        Ok(self.head(address).await?.is_some() || self.has_children(address).await?)
    }

    pub(crate) async fn stat(
        &self,
        address: &Address,
    ) -> Result<(ScannedEntry, Option<ObjectAttributes>), Failed> {
        if address.is_bucket() {
            self.probe_bucket(address).await?;
            return Ok((dir_entry(&address.bucket, None), None));
        }
        if let Some(meta) = self.head(address).await? {
            return Ok((
                file_entry(address.name(), meta.size, meta.modified_ms),
                Some(meta.attributes),
            ));
        }
        if self.has_children(address).await? {
            return Ok((dir_entry(address.name(), None), None));
        }
        Err(Failed::S3(S3Error::NoSuchKey))
    }

    /// Lists a folder page by page, handing entries to `sink` as they arrive: the first page at
    /// once, then in batches of about 2,000.
    pub(crate) async fn list_stream(
        &self,
        address: &Address,
        cancel: &CancelToken,
        sink: &mut dyn FnMut(Vec<ListedEntry>),
    ) -> Result<(), Failed> {
        let (bucket, prefix) = (address.bucket.clone(), address.prefix());
        let mut token: Option<String> = None;
        let mut first = true;
        let mut seen_any = false;
        let mut last_file: Option<String> = None;
        let mut batch: Vec<ListedEntry> = Vec::new();
        loop {
            if cancel.is_cancelled() {
                return Err(Failed::Vfs(VfsError::Cancelled));
            }
            let page = cancellable(
                cancel,
                self.run(address, |client| {
                    let (bucket, prefix, token) = (bucket.clone(), prefix.clone(), token.clone());
                    async move {
                        client
                            .list_objects_v2()
                            .bucket(bucket)
                            .prefix(prefix)
                            .delimiter("/")
                            // Keys may hold characters XML cannot (control characters), so ask
                            // for them URL-encoded; the SDK leaves decoding to the caller.
                            .encoding_type(EncodingType::Url)
                            .max_keys(PAGE)
                            .set_continuation_token(token)
                            .send()
                            .await
                    }
                }),
            )
            .await?;
            let encoded = page.encoding_type() == Some(&EncodingType::Url);
            let plain = |text: &str| {
                if encoded {
                    url_decode(text)
                } else {
                    text.to_owned()
                }
            };
            for object in page.contents() {
                seen_any = true;
                let Some(key) = object.key().map(&plain) else {
                    continue;
                };
                let key = key.as_str();
                let name = key.strip_prefix(prefix.as_str()).unwrap_or(key);
                // The marker of this very folder, and a key no path can address (`a//b` lists as
                // an empty name under `a/`).
                if name.is_empty() || name == "." || name == ".." || name.contains('/') {
                    if !name.is_empty() {
                        log::debug!("s3: skipping the key {key:?}, which has no addressable name");
                    }
                    continue;
                }
                let size = object.size().unwrap_or(0).max(0) as u64;
                last_file = Some(name.to_owned());
                batch.push(ListedEntry {
                    entry: file_entry(name, size, millis(object.last_modified())),
                    attributes: Some(ObjectAttributes {
                        storage_class: object
                            .storage_class()
                            .map_or(StorageClass::Standard, |class| {
                                StorageClass::from_api_name(class.as_str())
                            }),
                        etag: object.e_tag().map(str::to_owned),
                    }),
                });
            }
            for common in page.common_prefixes() {
                seen_any = true;
                let Some(full) = common.prefix().map(&plain) else {
                    continue;
                };
                let full = full.as_str();
                let name = full
                    .strip_prefix(prefix.as_str())
                    .unwrap_or(full)
                    .trim_end_matches('/');
                if name.is_empty() || name == "." || name == ".." || name.contains('/') {
                    log::debug!("s3: skipping the prefix {full:?}, which has no addressable name");
                    continue;
                }
                // An object and a folder of one name cannot both be shown: the file wins, as it
                // does for `stat`.
                if last_file.as_deref() == Some(name) {
                    continue;
                }
                batch.push(ListedEntry {
                    entry: dir_entry(name, None),
                    attributes: None,
                });
            }
            token = if page.is_truncated().unwrap_or(false) {
                page.next_continuation_token().map(str::to_owned)
            } else {
                None
            };
            if !batch.is_empty() && (first || batch.len() >= BATCH || token.is_none()) {
                sink(std::mem::take(&mut batch));
            }
            first = false;
            if token.is_none() {
                break;
            }
        }
        if !seen_any && !address.is_bucket() {
            // Nothing under the prefix: a file, or nothing at all.
            return Err(if self.head(address).await?.is_some() {
                Failed::Vfs(VfsError::NotADirectory {
                    location: address.location.clone(),
                })
            } else {
                Failed::S3(S3Error::NoSuchKey)
            });
        }
        Ok(())
    }

    /// Writes the empty marker object of a folder.
    pub(crate) async fn put_marker(&self, address: &Address) -> Result<(), Failed> {
        let (bucket, key) = (address.bucket.clone(), address.marker());
        self.run(address, |client| {
            let (bucket, key) = (bucket.clone(), key.clone());
            async move {
                client
                    .put_object()
                    .bucket(bucket)
                    .key(key)
                    .content_type("application/x-directory")
                    .send()
                    .await
            }
        })
        .await
        .map(|_| ())
    }

    pub(crate) async fn delete_key(&self, address: &Address, key: String) -> Result<(), Failed> {
        let bucket = address.bucket.clone();
        self.run(address, |client| {
            let (bucket, key) = (bucket.clone(), key.clone());
            async move { client.delete_object().bucket(bucket).key(key).send().await }
        })
        .await
        .map(|_| ())
    }

    pub(crate) async fn list_buckets(&self, address: &Address) -> Result<Vec<BucketInfo>, Failed> {
        let output = self
            .run(address, |client| async move {
                client.list_buckets().send().await
            })
            .await?;
        Ok(output
            .buckets()
            .iter()
            .filter_map(|bucket| {
                Some(BucketInfo {
                    name: bucket.name()?.to_owned(),
                    created_ms: millis(bucket.creation_date()),
                })
            })
            .collect())
    }

    pub(crate) async fn create_bucket(&self, address: &Address) -> Result<(), Failed> {
        let bucket = address.bucket.clone();
        self.run(address, |client| {
            let bucket = bucket.clone();
            async move { client.create_bucket().bucket(bucket).send().await }
        })
        .await
        .map(|_| ())
    }

    /// Copies an object on the server: one `CopyObject`, or parts when it is bigger than
    /// `threshold`. Never sends the data through this computer.
    pub(crate) async fn copy_object(
        &self,
        src: &Address,
        dst: &Address,
        size: u64,
        threshold: u64,
        cancel: &CancelToken,
    ) -> Result<(), Failed> {
        let source = copy_source(&src.bucket, &src.key);
        if size <= threshold.min(MAX_SINGLE_COPY) {
            let (bucket, key) = (dst.bucket.clone(), dst.key.clone());
            return cancellable(
                cancel,
                self.run(dst, |client| {
                    let (bucket, key, source) = (bucket.clone(), key.clone(), source.clone());
                    async move {
                        client
                            .copy_object()
                            .bucket(bucket)
                            .key(key)
                            .copy_source(source)
                            .send()
                            .await
                    }
                }),
            )
            .await
            .map(|_| ());
        }
        let (bucket, key) = (dst.bucket.clone(), dst.key.clone());
        let created = self
            .run(dst, |client| {
                let (bucket, key) = (bucket.clone(), key.clone());
                async move {
                    client
                        .create_multipart_upload()
                        .bucket(bucket)
                        .key(key)
                        .send()
                        .await
                }
            })
            .await?;
        let Some(upload_id) = created.upload_id().map(str::to_owned) else {
            return Err(Failed::S3(S3Error::Service {
                status: None,
                code: None,
            }));
        };
        let copied = self
            .copy_parts(src, dst, &source, size, &upload_id, cancel)
            .await;
        if copied.is_err() {
            self.abort_upload(dst, &upload_id).await;
        }
        copied
    }

    async fn copy_parts(
        &self,
        _src: &Address,
        dst: &Address,
        source: &str,
        size: u64,
        upload_id: &str,
        cancel: &CancelToken,
    ) -> Result<(), Failed> {
        let mut parts = Vec::new();
        let mut start = 0u64;
        let mut number = 1i32;
        while start < size {
            let end = (start + COPY_PART).min(size) - 1;
            let (bucket, key) = (dst.bucket.clone(), dst.key.clone());
            let output = cancellable(
                cancel,
                self.run(dst, |client| {
                    let (bucket, key) = (bucket.clone(), key.clone());
                    let (source, upload_id) = (source.to_owned(), upload_id.to_owned());
                    async move {
                        client
                            .upload_part_copy()
                            .bucket(bucket)
                            .key(key)
                            .copy_source(source)
                            .copy_source_range(format!("bytes={start}-{end}"))
                            .upload_id(upload_id)
                            .part_number(number)
                            .send()
                            .await
                    }
                }),
            )
            .await?;
            let etag = output
                .copy_part_result()
                .and_then(|result| result.e_tag())
                .map(str::to_owned);
            parts.push(
                CompletedPart::builder()
                    .part_number(number)
                    .set_e_tag(etag)
                    .build(),
            );
            start = end + 1;
            number += 1;
        }
        self.complete_upload(dst, upload_id, parts, false).await
    }

    pub(crate) async fn complete_upload(
        &self,
        address: &Address,
        upload_id: &str,
        parts: Vec<CompletedPart>,
        exclusive: bool,
    ) -> Result<(), Failed> {
        let (bucket, key) = (address.bucket.clone(), address.key.clone());
        let upload = CompletedMultipartUpload::builder()
            .set_parts(Some(parts))
            .build();
        self.run(address, |client| {
            let (bucket, key, upload_id) = (bucket.clone(), key.clone(), upload_id.to_owned());
            let upload = upload.clone();
            async move {
                let mut request = client
                    .complete_multipart_upload()
                    .bucket(bucket)
                    .key(key)
                    .upload_id(upload_id)
                    .multipart_upload(upload);
                if exclusive {
                    request = request.if_none_match("*");
                }
                request.send().await
            }
        })
        .await
        .map(|_| ())
    }

    /// Abandons a multipart upload so the service keeps (and bills) none of its parts.
    pub(crate) async fn abort_upload(&self, address: &Address, upload_id: &str) {
        let (bucket, key) = (address.bucket.clone(), address.key.clone());
        let result = self
            .run(address, |client| {
                let (bucket, key, upload_id) = (bucket.clone(), key.clone(), upload_id.to_owned());
                async move {
                    client
                        .abort_multipart_upload()
                        .bucket(bucket)
                        .key(key)
                        .upload_id(upload_id)
                        .send()
                        .await
                }
            })
            .await;
        if let Err(failed) = result {
            log::warn!(
                "s3: could not abort the upload of {}: {failed:?}",
                address.location.uri
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_copy_source_encodes_the_key_but_not_its_slashes() {
        assert_eq!(copy_source("b", "a/b c/é#.txt"), "b/a/b%20c/%C3%A9%23.txt");
        assert_eq!(copy_source("b", "plain-name_1.0~"), "b/plain-name_1.0~");
    }

    #[test]
    fn listed_keys_are_url_decoded() {
        assert_eq!(
            url_decode("caf%C3%A9+menu%2B1%20%231.jpg"),
            "café menu+1 #1.jpg"
        );
        assert_eq!(url_decode("a%2Fb"), "a/b");
        assert_eq!(url_decode("100%"), "100%");
        assert_eq!(url_decode("%zz"), "%zz");
        assert_eq!(url_decode("%01"), "\u{1}");
    }

    #[test]
    fn entries_know_their_names_and_kinds() {
        let file = file_entry(".env", 12, Some(5));
        assert_eq!(file.kind, EntryKind::File);
        assert!(file.hidden);
        assert_eq!(file.size, Some(12));
        let dir = dir_entry("photos", None);
        assert_eq!(dir.kind, EntryKind::Directory);
        assert_eq!(dir.size, None);
        assert!(!dir.hidden);
    }
}

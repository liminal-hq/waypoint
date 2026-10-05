// The S3 provider against recorded response bodies, replayed through the SDK in the order the
// provider asks: the shapes of AWS's own answers (a listing with URL-encoded keys, folders and
// storage classes; an empty listing; a bucket in another region; an archived object), without a
// server.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::sync::Arc;

use support::canned::{Canned, Response};
use support::FixedKey;
use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_provider_s3::{S3Config, S3Options, S3Provider, StorageClass};
use waypoint_vfs::{CancelToken, EntryKind, Provider};

fn reply(status: u16, headers: &[(&str, &str)], body: &str) -> Response {
    (
        status,
        headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        body.to_owned(),
    )
}

/// A provider that talks to the canned server, and a location of `bucket` on it.
fn replayed(responses: Vec<Response>) -> (S3Provider, Canned) {
    let canned = Canned::start(responses);
    let provider = S3Provider::new(
        S3Config::new(Arc::new(FixedKey {
            key_id: "AKIAIOSFODNN7EXAMPLE".to_owned(),
            secret: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_owned(),
        }))
        .with_options(S3Options {
            follow_region_redirects: Some(true),
            max_attempts: Some(1),
            ..S3Options::default()
        }),
    );
    (provider, canned)
}

fn at(canned: &Canned, key: &str) -> VfsPath {
    VfsPath::from_uri(&format!(
        "s3://my-bucket/{key}?endpoint=http%3A%2F%2F127.0.0.1%3A{}",
        canned.port
    ))
    .unwrap()
}

const LISTING: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>my-bucket</Name>
  <Prefix>photos/</Prefix>
  <Delimiter>/</Delimiter>
  <MaxKeys>1000</MaxKeys>
  <EncodingType>url</EncodingType>
  <KeyCount>5</KeyCount>
  <IsTruncated>false</IsTruncated>
  <Contents>
    <Key>photos/</Key>
    <LastModified>2026-01-02T03:04:05.000Z</LastModified>
    <ETag>&quot;d41d8cd98f00b204e9800998ecf8427e&quot;</ETag>
    <Size>0</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
  <Contents>
    <Key>photos/caf%C3%A9+menu%2B1%20%231.jpg</Key>
    <LastModified>2026-02-03T04:05:06.000Z</LastModified>
    <ETag>&quot;9a0364b9e99bb480dd25e1f0284c8555&quot;</ETag>
    <Size>12345</Size>
    <StorageClass>GLACIER</StorageClass>
  </Contents>
  <Contents>
    <Key>photos/.cover</Key>
    <LastModified>2026-02-03T04:05:06.000Z</LastModified>
    <ETag>&quot;9a0364b9e99bb480dd25e1f0284c8555&quot;</ETag>
    <Size>7</Size>
    <StorageClass>INTELLIGENT_TIERING</StorageClass>
  </Contents>
  <CommonPrefixes><Prefix>photos/2025/</Prefix></CommonPrefixes>
  <CommonPrefixes><Prefix>photos/%E6%97%A5%E6%9C%AC%E8%AA%9E/</Prefix></CommonPrefixes>
</ListBucketResult>"#;

#[test]
fn an_aws_listing_has_folders_files_decoded_names_and_storage_classes() {
    let (provider, canned) = replayed(vec![reply(200, &[], LISTING)]);
    let folder = at(&canned, "photos");
    let mut listed = Vec::new();
    provider
        .list_batches_with_attributes(&folder, &CancelToken::new(), &mut |b| listed.extend(b))
        .unwrap();
    let names: Vec<String> = listed
        .iter()
        .map(|l| l.entry.name.to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["café menu+1 #1.jpg", ".cover", "2025", "日本語"]);
    let find = |name: &str| listed.iter().find(|l| l.entry.name == name).unwrap();
    // The folder's own marker is not an entry, a URL-encoded key is decoded, and a prefix is a folder.
    let cafe = find("café menu+1 #1.jpg");
    assert_eq!(cafe.entry.kind, EntryKind::File);
    assert_eq!(cafe.entry.size, Some(12345));
    assert_eq!(cafe.entry.modified_ms, Some(1_770_091_506_000));
    let attributes = cafe.attributes.as_ref().unwrap();
    assert_eq!(attributes.storage_class, StorageClass::Glacier);
    assert!(attributes.archived());
    assert_eq!(
        attributes.etag.as_deref(),
        Some("\"9a0364b9e99bb480dd25e1f0284c8555\"")
    );
    assert!(find(".cover").entry.hidden);
    assert_eq!(
        find(".cover").attributes.as_ref().unwrap().storage_class,
        StorageClass::IntelligentTiering
    );
    assert_eq!(find("日本語").entry.kind, EntryKind::Directory);
    assert!(find("2025").attributes.is_none());
    let (line, _) = canned.lines().remove(0);
    assert!(
        line.contains("prefix=photos%2F")
            && line.contains("delimiter=%2F")
            && line.contains("encoding-type=url"),
        "{line}"
    );
}

#[test]
fn an_empty_listing_is_a_missing_folder_unless_the_name_is_a_file() {
    let empty = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>my-bucket</Name><Prefix>nothing/</Prefix><KeyCount>0</KeyCount><MaxKeys>1000</MaxKeys><Delimiter>/</Delimiter><IsTruncated>false</IsTruncated></ListBucketResult>"#;
    let missing = reply(404, &[("x-amz-request-id", "R")], "");
    let (provider, canned) = replayed(vec![reply(200, &[], empty), missing]);
    let result = provider.list(&at(&canned, "nothing"), &CancelToken::new(), 0, &mut |_| {});
    assert!(
        matches!(result, Err(VfsError::NotFound { .. })),
        "{result:?}"
    );

    let head = reply(
        200,
        &[
            ("content-length", "3"),
            ("last-modified", "Fri, 02 Jan 2026 03:04:05 GMT"),
            ("etag", "\"abc\""),
        ],
        "",
    );
    let (provider, canned) = replayed(vec![reply(200, &[], empty), head]);
    let result = provider.list(&at(&canned, "nothing"), &CancelToken::new(), 0, &mut |_| {});
    assert!(
        matches!(result, Err(VfsError::NotADirectory { .. })),
        "{result:?}"
    );
}

#[test]
fn errors_are_read_from_aws_s_xml_bodies() {
    let error = |status, code: &str| {
        reply(
            status,
            &[("content-type", "application/xml")],
            &format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Error><Code>{code}</Code><Message>m</Message><RequestId>R</RequestId></Error>"),
        )
    };
    let run = |response: Response| {
        let (provider, canned) = replayed(vec![response]);
        provider.list(&at(&canned, "x"), &CancelToken::new(), 0, &mut |_| {})
    };
    assert!(matches!(
        run(error(404, "NoSuchBucket")),
        Err(VfsError::NotFound { .. })
    ));
    assert!(matches!(
        run(error(403, "AccessDenied")),
        Err(VfsError::PermissionDenied { .. })
    ));
    assert!(matches!(
        run(error(403, "InvalidAccessKeyId")),
        Err(VfsError::AuthFailed { .. })
    ));
    assert!(matches!(
        run(error(403, "SignatureDoesNotMatch")),
        Err(VfsError::AuthFailed { .. })
    ));
}

#[test]
fn a_bucket_in_another_region_is_followed_through_the_region_header() {
    let moved = reply(
        301,
        &[("x-amz-bucket-region", "eu-west-1"), ("content-type", "application/xml")],
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Error><Code>PermanentRedirect</Code><Message>The bucket you are attempting to access must be addressed using the specified endpoint.</Message><Endpoint>my-bucket.s3.eu-west-1.amazonaws.com</Endpoint></Error>",
    );
    let (provider, canned) = replayed(vec![moved, reply(200, &[], LISTING)]);
    let listed = provider
        .list(&at(&canned, "photos"), &CancelToken::new(), 0, &mut |_| {})
        .unwrap();
    assert_eq!(listed.len(), 4);
    let regions: Vec<String> = canned.lines().into_iter().map(|(_, r)| r).collect();
    assert_eq!(
        regions,
        ["us-east-1", "eu-west-1"],
        "the retry is signed for the bucket's region"
    );
}

#[test]
fn an_archived_object_is_a_typed_error_not_a_download() {
    let archived = reply(
        403,
        &[("content-type", "application/xml")],
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Error><Code>InvalidObjectState</Code><Message>The operation is not valid for the object's storage class</Message><StorageClass>DEEP_ARCHIVE</StorageClass></Error>",
    );
    let (provider, canned) = replayed(vec![archived]);
    let result = provider.open_read(&at(&canned, "cold.bin"));
    assert!(
        matches!(&result, Err(VfsError::Archived { .. })),
        "{:?}",
        result.err()
    );
    // One GET, and no restore request.
    let lines = canned.lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].0.starts_with("GET"), "{lines:?}");
    assert!(!lines[0].0.contains("restore"));
}

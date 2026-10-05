// The S3 provider against recorded response bodies, replayed through the SDK in the order the
// provider asks: the shapes of AWS's own answers (a listing with URL-encoded keys, folders and
// storage classes; an empty listing; a bucket in another region; an archived object), without a
// server.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod support;

use std::sync::Arc;

use aws_sdk_s3::config::SharedHttpClient;
use aws_sdk_s3::primitives::SdkBody;
use aws_smithy_http_client::test_util::{ReplayEvent, StaticReplayClient};
use support::FixedKey;
use waypoint_path::VfsPath;
use waypoint_protocol::VfsError;
use waypoint_provider_s3::{S3Config, S3Provider, StorageClass};
use waypoint_vfs::{CancelToken, EntryKind, Provider};

fn reply(status: u16, headers: &[(&str, &str)], body: &str) -> ReplayEvent {
    let mut response = http::Response::builder().status(status);
    for (name, value) in headers {
        response = response.header(*name, *value);
    }
    ReplayEvent::new(
        http::Request::builder()
            .uri("https://bucket.s3.amazonaws.com/")
            .body(SdkBody::empty())
            .unwrap(),
        response.body(SdkBody::from(body.to_owned())).unwrap(),
    )
}

fn replayed(events: Vec<ReplayEvent>) -> (S3Provider, StaticReplayClient) {
    let client = StaticReplayClient::new(events);
    let provider = S3Provider::new(
        S3Config::new(Arc::new(FixedKey {
            key_id: "AKIAIOSFODNN7EXAMPLE".to_owned(),
            secret: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_owned(),
        }))
        .with_http_client(SharedHttpClient::new(client.clone())),
    );
    (provider, client)
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
    let (provider, client) = replayed(vec![reply(200, &[], LISTING)]);
    let folder = VfsPath::from_uri("s3://my-bucket/photos").unwrap();
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
    assert!(!names.iter().any(|n| n.is_empty()));
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
    let request = &client.actual_requests().next().unwrap();
    let uri = request.uri().to_string();
    assert!(
        uri.contains("prefix=photos%2F") && uri.contains("delimiter=%2F"),
        "{uri}"
    );
}

#[test]
fn an_empty_listing_is_a_missing_folder_unless_the_name_is_a_file() {
    let empty = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>my-bucket</Name><Prefix>nothing/</Prefix><KeyCount>0</KeyCount><MaxKeys>1000</MaxKeys><Delimiter>/</Delimiter><IsTruncated>false</IsTruncated></ListBucketResult>"#;
    let missing = reply(404, &[("x-amz-request-id", "R")], "");
    let (provider, _) = replayed(vec![reply(200, &[], empty), missing]);
    let result = provider.list(
        &VfsPath::from_uri("s3://my-bucket/nothing").unwrap(),
        &CancelToken::new(),
        0,
        &mut |_| {},
    );
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
    let (provider, _) = replayed(vec![reply(200, &[], empty), head]);
    let result = provider.list(
        &VfsPath::from_uri("s3://my-bucket/nothing").unwrap(),
        &CancelToken::new(),
        0,
        &mut |_| {},
    );
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
    let folder = VfsPath::from_uri("s3://my-bucket/x").unwrap();
    let run = |event: ReplayEvent| {
        let (provider, _) = replayed(vec![event]);
        provider.list(&folder, &CancelToken::new(), 0, &mut |_| {})
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
    let (provider, client) = replayed(vec![moved, reply(200, &[], LISTING)]);
    let listed = provider
        .list(
            &VfsPath::from_uri("s3://my-bucket/photos").unwrap(),
            &CancelToken::new(),
            0,
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(listed.len(), 4);
    let requests: Vec<String> = client
        .actual_requests()
        .map(|r| r.uri().to_string())
        .collect();
    assert!(
        requests[0].contains("us-east-1") || requests[0].contains("s3.amazonaws.com"),
        "{requests:?}"
    );
    assert!(
        requests[1].contains("eu-west-1"),
        "the retry goes to the bucket's region: {requests:?}"
    );
}

#[test]
fn an_archived_object_is_a_typed_error_not_a_download() {
    let archived = reply(
        403,
        &[("content-type", "application/xml")],
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Error><Code>InvalidObjectState</Code><Message>The operation is not valid for the object's storage class</Message><StorageClass>DEEP_ARCHIVE</StorageClass></Error>",
    );
    let (provider, client) = replayed(vec![archived]);
    let result = provider.open_read(&VfsPath::from_uri("s3://my-bucket/cold.bin").unwrap());
    assert!(
        matches!(&result, Err(VfsError::Unsupported { what }) if what.contains("restore")),
        "{:?}",
        result.err()
    );
    // One GET, and no restore request.
    let uris: Vec<_> = client
        .actual_requests()
        .map(|r| (r.method().to_string(), r.uri().to_string()))
        .collect();
    assert_eq!(uris.len(), 1);
    assert!(
        uris.iter().all(|(_, uri)| !uri.contains("restore")),
        "{uris:?}"
    );
}

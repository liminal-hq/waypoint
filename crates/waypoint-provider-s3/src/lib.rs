// Waypoint's S3 provider: `s3://` locations served by the AWS SDK for S3, for AWS and for
// S3-compatible services.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! `S3Provider` implements `waypoint_vfs::Provider` for `s3://bucket/key?endpoint=origin`
//! locations (`docs/architecture/remote-locations.md`). The bucket is the authority of the
//! address and the first level of the tree; the keys under it are folders (by prefix and the
//! `/` delimiter) and files. Without `endpoint` the bucket is on AWS (its region is found by
//! following the service's redirect); with one it is on MinIO, Cloudflare R2, Backblaze B2,
//! Wasabi, DigitalOcean Spaces or any other S3-compatible service, addressed by path. `presets`
//! holds what those services need.
//!
//! The provider is synchronous, as the trait is, and runs the asynchronous SDK on a small Tokio
//! runtime of its own. A listing streams its pages as they arrive and hands the entries over
//! in batches (the first page at once). Reads are ranged and reopen from where they stopped when
//! the connection breaks; writes are one `PutObject` or a multipart upload that is aborted when
//! the write fails or is dropped; a copy within one service happens on the server. S3 has no
//! rename, so `Capabilities::rename` is `None` and a move is a copy and a delete, which the
//! engine does and which is not atomic.
//!
//! A folder is a prefix: it exists while something is under it, and creating one writes a `key/`
//! marker object. An object in the `GLACIER` or `DEEP_ARCHIVE` class is listed, and reading it
//! fails with a typed "needs a restore" error; Waypoint never starts a restore.
//!
//! Nothing here stores a secret. The access key and secret come from the `CredentialSource` or
//! the person's answer to `Provider::connect`; the AWS environment and `~/.aws` files are read
//! only when the app turns `S3Config::with_ambient_credentials` on, and only for AWS endpoints.
//! The storage class and ETag of each object are on `ListedEntry` (`list_batches_with_attributes`)
//! for a column that shows them.

mod address;
pub mod ambient;
mod errors;
mod ops;
mod options;
pub mod presets;
mod provider;
mod reader;
mod service;
mod storage_class;
mod writer;

pub use errors::{classify_response, xml_text, Response, S3Error};
pub use ops::{BucketInfo, ListedEntry};
pub use options::{S3Config, S3Options, DEFAULT_PART_SIZE, MIN_PART_SIZE};
pub use presets::Preset;
pub use provider::S3Provider;
pub use storage_class::{ObjectAttributes, StorageClass};

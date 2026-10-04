// Waypoint's virtual file system: the `Provider` trait, the local provider, and listings that hold
// the sorted, filtered index in Rust and serve ranges of it to the frontend.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod cancel;
#[cfg(any(test, feature = "testing"))]
pub mod conformance;
mod details;
mod dirscan;
mod dirscan_cache;
mod error;
#[cfg(any(test, feature = "testing"))]
mod fake_remote;
mod group;
mod icon;
mod index;
mod inspect;
mod listing;
mod local;
#[cfg(any(test, feature = "testing"))]
mod memory;
mod mime;
mod model;
mod names;
mod navigation;
mod order;
mod places;
mod poll;
mod provider;
mod provider_registry;
mod remote;
mod serve;
mod size;
mod space;
mod special;
#[cfg(unix)]
#[path = "sys_unix.rs"]
mod sys;
#[cfg(windows)]
#[path = "sys_windows.rs"]
mod sys;
mod text;
mod trash;
mod watch;
mod write;

pub use cancel::CancelToken;
pub use details::*;
pub use dirscan::*;
pub use dirscan_cache::{DirScanCache, CACHE_FILE, CACHE_MAX_ROOTS, CACHE_MAX_ROWS, CACHE_VERSION};
pub use error::{from_io, from_io_pair, InjectedError};
#[cfg(any(test, feature = "testing"))]
pub use fake_remote::{FakeRemoteProvider, RemoteFault};
pub use icon::{group_for, group_for_mime, group_for_scan};
pub use listing::{EventSink, Listing, ListingOptions, WatchState};
pub use local::LocalProvider;
#[cfg(any(test, feature = "testing"))]
pub use memory::{MemOp, MemoryProvider};
pub use mime::{guess as guess_mime, SNIFF_LEN};
pub use model::*;
pub use names::{child_path, validate_name};
pub use navigation::{describe_location, parse_location, parse_location_with};
pub use order::natural_key;
pub use places::*;
pub use poll::PollWatch;
pub use provider::*;
pub use provider_registry::ProviderRegistry;
pub use remote::{ConnectAnswer, Credential, CredentialSource, NoCredentials, Secret};
pub use serve::{
    error_response, parse_range, serve_file, status_response, ByteRange, ServedFile, MAX_CHUNK,
    MAX_WHOLE,
};
pub use size::{lower_thread_priority, FolderSizeRun, REPORT_EVERY as FOLDER_SIZE_REPORT_EVERY};
pub use space::free_space;
pub use special::{SpecialDirs, SpecialFolder};
pub use text::{read_text_head, TEXT_HEAD_MAX};
#[cfg(any(test, feature = "testing"))]
pub use trash::MemoryTrashSource;
pub use trash::{
    TrashInfo, TrashProvider, TrashSource, TrashedItem, POLL_INTERVAL as TRASH_POLL_INTERVAL,
};
pub use watch::{WatchMode, WatchOptions};
pub use write::{FileTimes, Permissions, ReadStream, VolumeId, WriteOptions, WriteStream};

// `PROPFIND`: stat, streamed listings, free space, and the probe a connection starts with.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io::BufReader;

use waypoint_path::{ConnectionKey, VfsPath};
use waypoint_protocol::{ConnectionState, VfsError};
use waypoint_vfs::{CancelToken, ConnectAnswer, EntryKind, ScannedEntry, VolumeSpace};

use crate::client::Request;
use crate::entry::entry;
use crate::errors::Op;
use crate::paths::{href_segments, remote};
use crate::provider::{Target, WebDavProvider};
use crate::stream::{pump, ChannelReader};
use crate::xml::{parse_multistatus, Multi, Props, XmlError};

const STAT_PROPS: &str = "<resourcetype/><getcontentlength/><getlastmodified/><getetag/>\
                          <getcontenttype/>";
/// What a listing shows of each entry: its kind, size and modification time, and nothing more. An
/// entity tag, a content type or checksums are asked for when one entry is looked at (`stat`), so a
/// server answering for 100 000 entries neither computes nor sends them (a quarter of
/// `rclone serve webdav`'s answer, #512).
const LIST_PROPS: &str = "<resourcetype/><getcontentlength/><getlastmodified/>";
const NEXTCLOUD_PROPS: &str = "<oc:checksums/>";
const QUOTA_PROPS: &str = "<quota-available-bytes/><quota-used-bytes/>";

/// The body asking for `props`. Only the properties wanted are asked for, so a server does not
/// compute what is not shown (`allprop` on Nextcloud lists a dozen of its own).
fn propfind_body(props: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <propfind xmlns=\"DAV:\" xmlns:oc=\"http://owncloud.org/ns\"><prop>{props}</prop></propfind>"
    )
    .into_bytes()
}

pub(crate) fn propfind(url: String, depth: &str, props: &str) -> Request {
    Request::new("PROPFIND", url)
        .header("Depth", depth)
        .header("Content-Type", "application/xml; charset=utf-8")
        .bytes(propfind_body(props))
}

fn entry_props(target: &Target) -> String {
    if target.nextcloud {
        format!("{STAT_PROPS}{NEXTCLOUD_PROPS}")
    } else {
        STAT_PROPS.to_owned()
    }
}

impl WebDavProvider {
    /// Reads a small multistatus body whole.
    fn read_multistatus(
        &self,
        target: &Target,
        reply: crate::client::Reply,
    ) -> Result<Vec<Multi>, VfsError> {
        let timeout = target.session.options.timeout;
        let bytes = self
            .block(async { tokio::time::timeout(timeout, reply.into_response().bytes()).await })
            .map_err(|_| VfsError::Timeout {
                location: target.location.clone(),
            })?
            .map_err(|error| crate::errors::from_transport(&error, &target.location))?;
        let mut all = Vec::new();
        parse_multistatus(&bytes[..], &CancelToken::new(), &mut |multi| {
            all.push(multi);
            true
        })
        .map_err(|_| unreadable(target))?;
        Ok(all)
    }

    /// The entry at `path` with everything the server said about it.
    pub(crate) fn dav_stat(&self, path: &VfsPath) -> Result<(ScannedEntry, Props), VfsError> {
        let target = self.target(path)?;
        let request = propfind(target.url(false), "0", &entry_props(&target));
        let reply = self.exec(&target, &request, Op::Read, None)?;
        let all = self.read_multistatus(&target, reply)?;
        let wanted = target.remote.segments();
        let multi = all
            .iter()
            .find(|multi| href_segments(&multi.href) == wanted)
            .or_else(|| all.first())
            .ok_or_else(|| unreadable(&target))?;
        if let Some(code) = multi.status.filter(|code| !(200..300).contains(code)) {
            let status = reqwest::StatusCode::from_u16(code).unwrap_or_default();
            return Err(crate::errors::from_status(
                Op::Read,
                status,
                None,
                &target.location,
            ));
        }
        let folder_hint = target.remote.is_root() || multi.href.trim_end().ends_with('/');
        Ok((
            entry(&target.name(), &multi.props, folder_hint),
            multi.props.clone(),
        ))
    }

    /// Lists a folder: one `PROPFIND` of depth 1, parsed as it streams, and handed over in batches.
    pub(crate) fn dav_list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        if cancel.is_cancelled() {
            return Err(VfsError::Cancelled);
        }
        let target = self.target(path)?;
        // No trailing slash: a name that turns out to be a file is then a file, not a missing folder.
        let request = propfind(target.url(false), "1", LIST_PROPS);
        let reply = self.exec(&target, &request, Op::Read, Some(cancel))?;
        let options = target.session.options;
        let (send, chunks) = ChannelReader::channel();
        let (reader, failure) = ChannelReader::new(chunks);
        self.runtime().spawn(pump(
            reply,
            Some(cancel.clone()),
            options.timeout,
            target.location.clone(),
            send,
        ));

        let wanted = target.remote.segments();
        let mut batch: Vec<ScannedEntry> = Vec::new();
        let mut seen = 0usize;
        let mut children = 0usize;
        // What the folder's own response said: whether it is a collection, and its href's slash.
        let mut itself: Option<(Option<bool>, bool)> = None;
        let parsed = parse_multistatus(
            BufReader::with_capacity(64 * 1024, reader),
            cancel,
            &mut |multi| {
                let segments = href_segments(&multi.href);
                let first = seen == 0;
                seen += 1;
                // The folder's own response comes first by convention, but servers differ in how
                // they spell it, so a first response that cannot be a child is taken as it.
                let child_shaped =
                    segments.len() == wanted.len() + 1 && segments.starts_with(wanted);
                if segments == wanted || (first && !child_shaped) {
                    itself = Some((multi.props.collection, multi.href.trim_end().ends_with('/')));
                    return true;
                }
                if multi.status.is_some_and(|code| !(200..300).contains(&code)) {
                    return true;
                }
                let Some(name) = segments.last() else {
                    return true;
                };
                children += 1;
                batch.push(entry(
                    name,
                    &multi.props,
                    multi.href.trim_end().ends_with('/'),
                ));
                if batch.len() >= options.batch {
                    sink(std::mem::take(&mut batch));
                }
                true
            },
        );
        match parsed {
            Ok(()) => {}
            Err(XmlError::Stopped) => return Err(VfsError::Cancelled),
            Err(XmlError::Malformed(reason)) => {
                return Err(failure.take().unwrap_or_else(|| {
                    log::debug!(
                        "webdav: unreadable listing at {}: {reason}",
                        target.location.uri
                    );
                    unreadable(&target)
                }));
            }
        }
        if let Some(error) = failure.take() {
            return Err(error);
        }
        let is_file = match itself {
            Some((Some(collection), _)) => !collection,
            Some((None, slash)) => !slash && children == 0,
            None => false,
        };
        if is_file {
            return Err(VfsError::NotADirectory {
                location: target.location,
            });
        }
        if !batch.is_empty() {
            sink(batch);
        }
        Ok(())
    }

    /// Free and total space from the quota properties (RFC 4331), where the server has them.
    pub(crate) fn dav_free_space(&self, path: &VfsPath) -> Option<VolumeSpace> {
        let target = self.target(path).ok()?;
        let request = propfind(target.url(false), "0", QUOTA_PROPS);
        let reply = self.exec(&target, &request, Op::Read, None).ok()?;
        let all = self.read_multistatus(&target, reply).ok()?;
        let props = &all.first()?.props;
        let free = props.quota_available?;
        let used = props.quota_used?;
        Some(VolumeSpace {
            free_bytes: free,
            total_bytes: free.checked_add(used)?,
        })
    }

    /// What the server stores as the checksums of a file (Nextcloud's `oc:checksums`, such as
    /// `SHA1:abc MD5:def`), when it has any, for a copy to be verified against.
    pub fn checksums(&self, path: &VfsPath) -> Result<Option<String>, VfsError> {
        let (entry, props) = self.dav_stat(path)?;
        if entry.kind == EntryKind::Directory {
            return Err(VfsError::IsADirectory {
                location: path.to_location(),
            });
        }
        Ok(props.checksums)
    }

    /// Connecting: applies the answer, then asks the server about a path this connection used (or
    /// its root), which brings out an untrusted certificate, a refused login or a server that is not
    /// there. Whatever the server says about the path itself, bar a challenge, is a connection.
    pub(crate) fn dav_connect(
        &self,
        key: &ConnectionKey,
        answer: Option<ConnectAnswer>,
        cancel: &CancelToken,
    ) -> Result<(), VfsError> {
        let root = VfsPath::from_uri(key.as_str()).map_err(|_| VfsError::InvalidLocation {
            input: key.to_string(),
        })?;
        let remote = remote(&root, self.inner.scheme)?;
        let slot = self.inner.pool.slot(key);
        let target = self.target(&root)?;
        match answer {
            Some(ConnectAnswer::Credential(credential)) => {
                if !target.session.answer(credential) {
                    return Err(VfsError::Unsupported {
                        what: "this kind of login over WebDAV".to_owned(),
                    });
                }
            }
            Some(ConnectAnswer::TrustCertificate { fingerprint, .. }) => {
                slot.trust.trust(&fingerprint);
            }
            Some(_) => {
                return Err(VfsError::Unsupported {
                    what: "this answer over WebDAV".to_owned(),
                })
            }
            None => {}
        }
        slot.set_state(ConnectionState::Connecting);
        let probe = crate::pool::lock(&slot.probe).clone();
        let url = match probe {
            Some(path) => format!("{}{path}", crate::paths::origin(remote)),
            None => crate::paths::url(remote, true),
        };
        let request = propfind(url, "0", "<resourcetype/>");
        self.exec_raw(&target, &request, Some(cancel)).map(|_| ())
    }
}

fn unreadable(target: &Target) -> VfsError {
    VfsError::Io {
        message: "the server's answer could not be read as a listing".to_owned(),
        location: Some(target.location.clone()),
    }
}

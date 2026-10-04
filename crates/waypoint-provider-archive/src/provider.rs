// The archive provider: zip, tar and 7z files served as read-only folders.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsString;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, Weak};

use waypoint_path::{CaseRule, VfsPath};
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{
    group_for_scan, ArchiveBuilder, ArchiveCatalog, ArchiveEntryInfo, ArchiveKind, ArchiveWriters,
    CancelToken, Capabilities, EntryKind, PermissionModel, Permissions, Provider, ReadStream,
    ScannedEntry, Secret, WriteStream,
};

use crate::compress::{decoder, CancelReader};
use crate::errors::{corrupt, from_io, password_required, unsupported};
use crate::format::{self, ArchiveFormat, Magic, TarCompression};
use crate::index::{ArchiveIndex, Locator, Node};
use crate::info::{ArchiveInfo, EntryInfo};
use crate::names::{name_bytes, os_name, resolve_link_target};
use crate::options::{ArchiveNotice, ArchiveOptions, ContainerSource, NoticeSink};
use crate::sevenz::{self, SevenScan};
use crate::source::SourceSpec;
use crate::stream;
use crate::zip_scan::ScanError;
use crate::{tar_scan, zip_read, zip_scan};

/// The most bytes of a link's target read from its data.
const LINK_LIMIT: u64 = 4096;
/// The folder mode and the file mode reported when an archive stores none.
const DEFAULT_FOLDER_MODE: u32 = 0o755;
const DEFAULT_FILE_MODE: u32 = 0o644;

#[derive(Default)]
struct State {
    /// Built indexes, least recently used first.
    cache: Vec<(String, Arc<ArchiveIndex>)>,
    passwords: std::collections::HashMap<String, Secret>,
}

/// Serves `archive:` locations. It reads the archive file through the provider that holds it (found
/// with a `ContainerSource`), keeps the listing of each archive it has opened, and never writes.
pub struct ArchiveProvider {
    containers: Arc<dyn ContainerSource>,
    options: ArchiveOptions,
    notices: Mutex<Option<NoticeSink>>,
    state: Mutex<State>,
    me: Weak<ArchiveProvider>,
}

/// An archive's file, found and described.
struct Opened {
    spec: SourceSpec,
    container: Location,
    signature: (u64, Option<i64>),
}

impl ArchiveProvider {
    /// A provider that finds archive files through `containers`. Register the result under the
    /// `archive` scheme in the registry `containers` reads, so archives inside archives work.
    pub fn new(containers: Arc<dyn ContainerSource>, options: ArchiveOptions) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            containers,
            options,
            notices: Mutex::new(None),
            state: Mutex::new(State::default()),
            me: me.clone(),
        })
    }

    /// Where notices (a slow listing is starting) go.
    pub fn set_notice_sink(&self, sink: Option<NoticeSink>) {
        *self.notices.lock().unwrap_or_else(|e| e.into_inner()) = sink;
    }

    /// Supplies the password of the archive file at `container` (the answer to an `AuthRequired`).
    /// The archive's listing is made again with it. The password stays in memory only.
    pub fn unlock(&self, container: &VfsPath, password: Secret) {
        let key = container.to_uri();
        let mut state = self.lock();
        state.cache.retain(|(cached, _)| cached != &key);
        state.passwords.insert(key, password);
    }

    /// Forgets the password given for `container`.
    pub fn forget_password(&self, container: &VfsPath) {
        self.lock().passwords.remove(&container.to_uri());
    }

    /// Drops the kept listing of `container`, so the next call reads the archive again.
    pub fn invalidate(&self, container: &VfsPath) {
        let key = container.to_uri();
        self.lock().cache.retain(|(cached, _)| cached != &key);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn password(&self, container: &VfsPath) -> Option<Secret> {
        self.lock().passwords.get(&container.to_uri()).cloned()
    }

    fn notify(&self, notice: ArchiveNotice) {
        let sink = self
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(sink) = sink {
            sink(notice);
        }
    }

    fn provider_of(&self, container: &VfsPath) -> Result<Arc<dyn Provider>, VfsError> {
        if matches!(container, VfsPath::Archive(_)) {
            let me: Arc<ArchiveProvider> = self.me.upgrade().ok_or(VfsError::Cancelled)?;
            return Ok(me);
        }
        self.containers.provider_for(container)
    }

    /// The archive file's provider, size and signature, without reading it.
    fn open(&self, container: &VfsPath) -> Result<Opened, VfsError> {
        let location = container.to_location();
        let provider = self.provider_of(container)?;
        let entry = provider.stat(container)?;
        if entry.kind == EntryKind::Directory {
            return Err(VfsError::IsADirectory { location });
        }
        let size = entry.size.unwrap_or(0);
        Ok(Opened {
            spec: SourceSpec {
                provider,
                path: container.clone(),
                size,
                spool: None,
            },
            container: location,
            signature: (size, entry.modified_ms),
        })
    }

    /// The listing of the archive at `container`, from the cache or built now. `progress` receives
    /// the entries read so far while it is built.
    fn index(
        &self,
        container: &VfsPath,
        cancel: &CancelToken,
        progress: &mut dyn FnMut(u32),
    ) -> Result<(Arc<ArchiveIndex>, SourceSpec), VfsError> {
        let opened = self.open(container)?;
        let key = container.to_uri();
        {
            let mut state = self.lock();
            if let Some(at) = state.cache.iter().position(|(cached, _)| cached == &key) {
                if state.cache[at].1.signature == opened.signature {
                    let entry = state.cache.remove(at);
                    let index = entry.1.clone();
                    state.cache.push(entry);
                    let mut spec = opened.spec;
                    spec.spool = index.spool.clone();
                    return Ok((index, spec));
                }
                state.cache.remove(at);
            }
        }
        let (index, spec) = self.build(opened, cancel, progress)?;
        let index = Arc::new(index);
        let mut state = self.lock();
        state.cache.retain(|(cached, _)| cached != &key);
        state.cache.push((key, index.clone()));
        while state.cache.len() > self.options.cached_archives.max(1) {
            state.cache.remove(0);
        }
        Ok((index, spec))
    }

    fn build(
        &self,
        mut opened: Opened,
        cancel: &CancelToken,
        progress: &mut dyn FnMut(u32),
    ) -> Result<(ArchiveIndex, SourceSpec), VfsError> {
        let container = opened.container.clone();
        let name = opened
            .spec
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let format = self.detect(&opened.spec, &name, &container)?;
        // A zip, a plain tar or a 7z needs seeks, which a stream out of another archive cannot give.
        let needs_seek = matches!(
            format,
            ArchiveFormat::Zip | ArchiveFormat::SevenZ | ArchiveFormat::Tar(TarCompression::None)
        );
        if needs_seek && matches!(opened.spec.path, VfsPath::Archive(_)) {
            opened.spec.spool = Some(Arc::new(self.spool(&opened.spec, &container, cancel)?));
        }
        let spec = opened.spec;
        let mut index = ArchiveIndex::new(format, opened.signature);
        index.spool = spec.spool.clone();
        let slow = format.sequential_listing()
            || (format == ArchiveFormat::Tar(TarCompression::None)
                && !matches!(spec.path, VfsPath::File(_)));
        if slow && spec.size >= self.options.slow_scan_bytes {
            index.slow_scan = true;
            self.notify(ArchiveNotice::SlowListing {
                container: container.clone(),
                format,
                bytes: spec.size,
            });
        }
        let max = self.options.max_entries;
        let secret = self.password(&spec.path);
        let scanned = match format {
            ArchiveFormat::Zip => {
                let mut source = spec.open_seek().map_err(|e| from_io(&e, &container))?;
                zip_scan::scan(&mut source, spec.size, &mut index, max, cancel, progress)
            }
            ArchiveFormat::Tar(TarCompression::None) => {
                let source = spec.open_seek().map_err(|e| from_io(&e, &container))?;
                tar_scan::scan_seek(source, spec.size, &mut index, max, cancel, progress)
            }
            ArchiveFormat::Tar(compression) => {
                let stream = spec.open_stream()?;
                let decoded = decoder(compression, stream).map_err(|e| from_io(&e, &container))?;
                let reader = CancelReader {
                    inner: decoded,
                    cancel: cancel.clone(),
                };
                tar_scan::scan_stream(reader, &mut index, max, cancel, progress)
            }
            ArchiveFormat::SevenZ => {
                let mut source = spec.open_seek().map_err(|e| from_io(&e, &container))?;
                match sevenz::scan(
                    &mut source,
                    secret.as_ref(),
                    &mut index,
                    max,
                    cancel,
                    progress,
                ) {
                    Ok(SevenScan::Done) => Ok(()),
                    Ok(SevenScan::Password(error)) => {
                        index.header_encrypted = true;
                        return Err(sevenz::map_error(&error, &container, false));
                    }
                    Err(error) => Err(error),
                }
            }
        };
        scanned.map_err(|error| match error {
            ScanError::Corrupt(why) => {
                log::debug!("archive: {} is damaged: {why}", container.uri);
                corrupt(&container)
            }
            ScanError::Io(error) => from_io(&error, &container),
            ScanError::TooMany(limit) => {
                unsupported(format!("an archive with more than {limit} entries"))
            }
            ScanError::Cancelled => VfsError::Cancelled,
        })?;
        index.finish();
        progress(index.entries.min(u32::MAX as usize) as u32);
        Ok((index, spec))
    }

    /// Which kind of archive the file is: its first bytes, then its name.
    fn detect(
        &self,
        spec: &SourceSpec,
        name: &str,
        container: &Location,
    ) -> Result<ArchiveFormat, VfsError> {
        let head = if spec.size == 0 {
            Vec::new()
        } else {
            let mut stream = spec.provider.open_read_at(&spec.path, 0)?;
            format::read_head(&mut stream).map_err(|e| from_io(&e, container))?
        };
        match format::magic(&head) {
            Magic::Format(format) => Ok(format),
            Magic::Wrapped(compression) => {
                // A compressed file is an archive only if a tar archive is what it holds.
                let inside = spec
                    .provider
                    .open_read(&spec.path)
                    .ok()
                    .and_then(|stream| decoder(compression, stream).ok())
                    .and_then(|mut decoded| format::read_head(&mut decoded).ok());
                if inside.is_some_and(|block| format::is_tar_header(&block)) {
                    return Ok(ArchiveFormat::Tar(compression));
                }
                match format::from_name(name) {
                    Some(found @ ArchiveFormat::Tar(_)) => Ok(found),
                    _ => Err(unsupported("a compressed file that is not an archive")),
                }
            }
            Magic::Unknown => format::from_name(name).ok_or_else(|| {
                if spec.size == 0 {
                    corrupt(container)
                } else {
                    unsupported("this kind of file as an archive")
                }
            }),
        }
    }

    /// Copies an archive that is inside another archive to a scratch file, so it can be read with
    /// seeks.
    fn spool(
        &self,
        spec: &SourceSpec,
        container: &Location,
        cancel: &CancelToken,
    ) -> Result<tempfile::TempPath, VfsError> {
        if spec.size > self.options.max_nested_bytes {
            return Err(unsupported(format!(
                "an archive inside an archive that is bigger than {} MiB",
                self.options.max_nested_bytes / (1024 * 1024)
            )));
        }
        let mut builder = tempfile::Builder::new();
        builder.prefix("waypoint-archive-");
        let file = match &self.options.scratch_dir {
            Some(dir) => builder.tempfile_in(dir),
            None => builder.tempfile(),
        }
        .map_err(|e| from_io(&e, container))?;
        let (mut file, path) = file.into_parts();
        let mut stream = spec.provider.open_read(&spec.path)?;
        let mut buffer = vec![0u8; 256 * 1024];
        let mut copied = 0u64;
        loop {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            let read = stream
                .read(&mut buffer)
                .map_err(|e| from_io(&e, container))?;
            if read == 0 {
                break;
            }
            copied += read as u64;
            if copied > self.options.max_nested_bytes {
                return Err(unsupported("an archive inside an archive that is too big"));
            }
            file.write_all(&buffer[..read])
                .map_err(|e| from_io(&e, container))?;
        }
        file.flush().map_err(|e| from_io(&e, container))?;
        Ok(path)
    }

    /// The archive and the node an `archive:` path names.
    fn locate(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Located, VfsError> {
        let VfsPath::Archive(archive) = path else {
            return Err(VfsError::Unsupported {
                what: path.scheme().to_owned(),
            });
        };
        let container_path = archive.container().clone();
        let (index, spec) = self.index(&container_path, cancel, progress)?;
        let node = index
            .find(archive.inner())
            .ok_or_else(|| VfsError::NotFound {
                location: path.to_location(),
            })?;
        Ok(Located {
            index,
            spec,
            node,
            container_path,
            container: archive.container().to_location(),
            at: path.clone(),
        })
    }

    fn scanned(&self, located: &Located, node: u32) -> ScannedEntry {
        scanned_entry(&located.index, node, || {
            located
                .container_path
                .file_name()
                .unwrap_or_else(|| OsString::from("archive"))
        })
    }

    /// The text a symlink holds: in the header (tar) or in the entry's data (zip and 7z).
    fn link_text(&self, located: &Located, node: u32) -> Result<Vec<u8>, VfsError> {
        if let Some(link) = &located.index.node(node).link {
            return Ok(link.to_vec());
        }
        let mut stream = self.open_node(located, node, 0, false)?;
        let mut text = Vec::new();
        stream
            .by_ref()
            .take(LINK_LIMIT)
            .read_to_end(&mut text)
            .map_err(|e| from_io(&e, &located.container))?;
        Ok(text)
    }

    /// Follows links to the entry whose data a read gets, up to eight deep.
    fn follow_for_read(&self, located: &Located, mut node: u32) -> Result<u32, VfsError> {
        for _ in 0..8 {
            if located.index.node(node).kind != EntryKind::Symlink {
                return Ok(node);
            }
            let text = self.link_text(located, node)?;
            let folder = located.index.path_of(located.index.node(node).parent);
            node = resolve_link_target(&folder, &text)
                .and_then(|path| located.index.find(&path))
                .ok_or_else(|| VfsError::NotFound {
                    location: located.at.to_location(),
                })?;
        }
        Err(VfsError::NotFound {
            location: located.at.to_location(),
        })
    }

    /// Opens an entry's data, skipping `start` bytes. `follow` resolves a symlink to what it
    /// points at, as `open(2)` does.
    fn open_node(
        &self,
        located: &Located,
        node: u32,
        start: u64,
        follow: bool,
    ) -> Result<ReadStream, VfsError> {
        let node = if follow {
            self.follow_for_read(located, node)?
        } else {
            node
        };
        let entry = located.index.node(node);
        let at = || located.at.to_location();
        match entry.kind {
            EntryKind::Directory => return Err(VfsError::IsADirectory { location: at() }),
            EntryKind::Other => return Err(unsupported("reading special files from an archive")),
            EntryKind::File | EntryKind::Symlink => {}
        }
        if entry.sparse {
            return Err(unsupported("reading sparse files from a tar archive"));
        }
        let secret = self.password(&located.container_path);
        if entry.encrypted && secret.is_none() {
            return Err(password_required(&located.container));
        }
        let Some(locator) = entry.locator.clone() else {
            return Err(VfsError::NotFound { location: at() });
        };
        let size = entry.size;
        if size == Some(0)
            || size.is_some_and(|size| start >= size && entry.kind == EntryKind::File)
        {
            return Ok(Box::new(std::io::empty()));
        }
        let spec = located.spec.clone();
        let container = located.container.clone();
        let encrypted = entry.encrypted;
        let cancel = CancelToken::new();
        match (locator, located.index.format) {
            (
                Locator::Zip {
                    ordinal,
                    header_start,
                },
                ArchiveFormat::Zip,
            ) => stream::spawn(&container.clone(), move |pump| {
                zip_read::read_job(
                    pump,
                    &spec,
                    &container,
                    ordinal,
                    header_start,
                    secret,
                    encrypted,
                    start,
                    &cancel,
                )
            }),
            (Locator::Tar { data_offset, .. }, ArchiveFormat::Tar(TarCompression::None)) => {
                let size = size.unwrap_or(0);
                let stream = spec
                    .provider
                    .open_read_at(&spec.path, data_offset + start)?;
                let limited: ReadStream = Box::new(stream.take(size - start));
                Ok(limited)
            }
            (Locator::Tar { data_offset, .. }, ArchiveFormat::Tar(compression)) => {
                let size = size.unwrap_or(0);
                stream::spawn(&container.clone(), move |pump| {
                    let source = match spec.open_stream() {
                        Ok(source) => source,
                        Err(error) => return pump.fail(error),
                    };
                    let mut reader = match decoder(compression, source) {
                        Ok(reader) => reader,
                        Err(error) => return pump.fail(from_io(&error, &container)),
                    };
                    pump.ready();
                    pump.copy_from(
                        &mut reader,
                        data_offset + start,
                        Some(size - start),
                        Some(&cancel),
                    );
                })
            }
            (Locator::SevenZ { file_index: _ }, ArchiveFormat::SevenZ) => {
                let name =
                    String::from_utf8_lossy(&entry_stored_name(&located.index, node)).into_owned();
                stream::spawn(&container.clone(), move |pump| {
                    let source = match spec.open_seek() {
                        Ok(source) => source,
                        Err(error) => return pump.fail(from_io(&error, &container)),
                    };
                    sevenz::read_job(
                        pump, source, &container, secret, &name, encrypted, start, &cancel,
                    )
                })
            }
            _ => Err(corrupt(&container)),
        }
    }

    /// What the archive at `container` is, without reading any entry.
    pub fn inspect(
        &self,
        container: &VfsPath,
        cancel: &CancelToken,
    ) -> Result<ArchiveInfo, VfsError> {
        let (index, _) = self.index(container, cancel, &mut |_| {})?;
        Ok(ArchiveInfo {
            format: index.format,
            entries: index.entries,
            encrypted_entries: index.encrypted_entries,
            unsafe_names: index.unsafe_names,
            slow_listing: index.slow_scan,
            comment: index.comment.clone(),
        })
    }

    /// Every entry of the archive at `container`, in the order the archive lists them (folders the
    /// archive leaves out come before what is in them), with the names as shown and what was unsafe
    /// about them.
    pub fn entries(
        &self,
        container: &VfsPath,
        cancel: &CancelToken,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<EntryInfo>, VfsError> {
        let (index, _) = self.index(container, cancel, progress)?;
        Ok((1..index.len() as u32)
            .map(|node| entry_info(&index, node))
            .collect())
    }

    /// Everything the archive says about one entry.
    pub fn entry_info(&self, path: &VfsPath) -> Result<EntryInfo, VfsError> {
        let located = self.locate(path, &CancelToken::new(), &mut |_| {})?;
        if located.node == 0 {
            return Err(VfsError::NotFound {
                location: path.to_location(),
            });
        }
        Ok(entry_info(&located.index, located.node))
    }
}

struct Located {
    index: Arc<ArchiveIndex>,
    spec: SourceSpec,
    node: u32,
    container_path: VfsPath,
    container: Location,
    at: VfsPath,
}

/// The name an entry has in the archive, which a 7z read looks it up by.
fn entry_stored_name(index: &ArchiveIndex, node: u32) -> Vec<u8> {
    let entry = index.node(node);
    match &entry.raw_name {
        Some(raw) => raw.to_vec(),
        None => index.path_of(node).join(&b'/'),
    }
}

fn entry_info(index: &ArchiveIndex, node: u32) -> EntryInfo {
    let entry = index.node(node);
    let components = index.path_of(node);
    EntryInfo {
        name: String::from_utf8_lossy(&entry.name).into_owned(),
        components,
        kind: entry.kind,
        size: entry.size,
        compressed_size: entry.compressed,
        mode: entry.mode,
        modified_ms: entry.modified_ms,
        link_target: entry.link.as_ref().map(|link| link.to_vec()),
        encrypted: entry.encrypted,
        unsafe_name: entry.unsafe_name,
        raw_name: entry.raw_name.as_ref().map(|raw| raw.to_vec()),
        synthetic: entry.synthetic,
    }
}

fn scanned_entry(
    index: &ArchiveIndex,
    node: u32,
    top_name: impl FnOnce() -> OsString,
) -> ScannedEntry {
    let entry: &Node = index.node(node);
    let name = if node == 0 {
        top_name()
    } else {
        os_name(&entry.name)
    };
    let (link_target, link_pending) = if entry.kind == EntryKind::Symlink {
        if entry.link.is_some() {
            (index.link_target_kind(node), false)
        } else {
            // A zip or 7z link holds its target in its data: resolved later, by `resolve_link`.
            (None, true)
        }
    } else {
        (None, false)
    };
    let executable = entry.mode.is_some_and(|mode| mode & 0o111 != 0);
    ScannedEntry {
        group: group_for_scan(
            &entry.name,
            entry.kind,
            link_target,
            link_pending,
            executable,
        ),
        hidden: entry.name.first() == Some(&b'.'),
        name,
        kind: entry.kind,
        link_target,
        link_pending,
        special: None,
        size: entry.size,
        modified_ms: entry.modified_ms,
        trashed: None,
    }
}

impl Provider for ArchiveProvider {
    fn scheme(&self) -> &'static str {
        waypoint_path::ARCHIVE_SCHEME
    }

    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new(CaseRule::Sensitive);
        caps.permissions = PermissionModel::Unix;
        caps.symlinks = true;
        caps
    }

    fn read_only(&self) -> bool {
        true
    }

    fn stat(&self, path: &VfsPath) -> Result<ScannedEntry, VfsError> {
        let located = self.locate(path, &CancelToken::new(), &mut |_| {})?;
        Ok(self.scanned(&located, located.node))
    }

    fn list(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        _inline_link_budget: usize,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ScannedEntry>, VfsError> {
        let located = self.locate(path, cancel, progress)?;
        if located.index.node(located.node).kind != EntryKind::Directory {
            return Err(VfsError::NotADirectory {
                location: path.to_location(),
            });
        }
        let children = located.index.children(located.node);
        let mut out = Vec::with_capacity(children.len());
        for (at, &child) in children.iter().enumerate() {
            if at % 1024 == 0 && cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            out.push(self.scanned(&located, child));
        }
        progress(out.len().min(u32::MAX as usize) as u32);
        Ok(out)
    }

    fn list_batches(
        &self,
        path: &VfsPath,
        cancel: &CancelToken,
        inline_link_budget: usize,
        sink: &mut dyn FnMut(Vec<ScannedEntry>),
    ) -> Result<(), VfsError> {
        let entries = self.list(path, cancel, inline_link_budget, &mut |_| {})?;
        let batch = self.options.batch.max(1);
        let mut entries = entries.into_iter().peekable();
        if entries.peek().is_none() {
            sink(Vec::new());
            return Ok(());
        }
        while entries.peek().is_some() {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            sink(entries.by_ref().take(batch).collect());
        }
        Ok(())
    }

    fn resolve_link(
        &self,
        folder: &VfsPath,
        entry: &ScannedEntry,
    ) -> Result<ScannedEntry, VfsError> {
        let path = folder
            .join(&entry.name)
            .map_err(|_| VfsError::InvalidLocation {
                input: folder.to_uri(),
            })?;
        let located = self.locate(&path, &CancelToken::new(), &mut |_| {})?;
        let node = located.index.node(located.node);
        if node.kind != EntryKind::Symlink {
            return Ok(self.scanned(&located, located.node));
        }
        let text = self.link_text(&located, located.node)?;
        let folder_parts = located.index.path_of(node.parent);
        let target = resolve_link_target(&folder_parts, &text)
            .and_then(|parts| located.index.find(&parts))
            .map(|target| located.index.node(target));
        let mut resolved = self.scanned(&located, located.node);
        resolved.link_pending = false;
        resolved.link_target = target.map(|target| target.kind);
        if let Some(target) = target {
            if target.kind == EntryKind::File {
                resolved.size = target.size;
                resolved.modified_ms = target.modified_ms;
            }
        }
        resolved.group = group_for_scan(
            &name_bytes(&entry.name),
            EntryKind::Symlink,
            resolved.link_target,
            false,
            false,
        );
        Ok(resolved)
    }

    fn connection_key(&self, path: &VfsPath) -> Option<waypoint_path::ConnectionKey> {
        path.connection_key()
    }

    fn open_read(&self, path: &VfsPath) -> Result<ReadStream, VfsError> {
        self.open_read_at(path, 0)
    }

    fn open_read_at(&self, path: &VfsPath, start: u64) -> Result<ReadStream, VfsError> {
        let located = self.locate(path, &CancelToken::new(), &mut |_| {})?;
        self.open_node(&located, located.node, start, true)
    }

    fn read_link(&self, path: &VfsPath) -> Result<OsString, VfsError> {
        let located = self.locate(path, &CancelToken::new(), &mut |_| {})?;
        if located.index.node(located.node).kind != EntryKind::Symlink {
            return Err(VfsError::Unsupported {
                what: "reading the target of something that is not a link".to_owned(),
            });
        }
        Ok(os_name(&self.link_text(&located, located.node)?))
    }

    fn permissions(&self, path: &VfsPath) -> Result<Permissions, VfsError> {
        let located = self.locate(path, &CancelToken::new(), &mut |_| {})?;
        let node = located.index.node(located.node);
        let mode = node.mode.unwrap_or(if node.kind == EntryKind::Directory {
            DEFAULT_FOLDER_MODE
        } else {
            DEFAULT_FILE_MODE
        });
        Ok(Permissions {
            mode: Some(mode),
            readonly: mode & 0o222 == 0,
        })
    }
}

impl ArchiveCatalog for ArchiveProvider {
    fn archive_entries(
        &self,
        archive: &VfsPath,
        cancel: &CancelToken,
        progress: &mut dyn FnMut(u32),
    ) -> Result<Vec<ArchiveEntryInfo>, VfsError> {
        let VfsPath::Archive(top) = archive else {
            return Err(VfsError::Unsupported {
                what: archive.scheme().to_owned(),
            });
        };
        let (index, _) = self.index(top.container(), cancel, progress)?;
        let mut out = Vec::with_capacity(index.len().saturating_sub(1));
        for node in 1..index.len() as u32 {
            let entry = index.node(node);
            let mut path = top.clone();
            for part in index.path_of(node) {
                path = path
                    .join(os_name(&part))
                    .map_err(|_| VfsError::InvalidLocation {
                        input: archive.to_uri(),
                    })?;
            }
            out.push(ArchiveEntryInfo {
                path: VfsPath::Archive(path),
                kind: entry.kind,
                size: entry.size,
                compressed_size: entry.compressed,
                mode: entry.mode,
                modified_ms: entry.modified_ms,
                link_target: entry.link.as_ref().map(|link| link.to_vec()),
                encrypted: entry.encrypted,
                unsafe_name: entry.unsafe_name,
                synthetic: entry.synthetic,
            });
        }
        Ok(out)
    }
}

impl ArchiveWriters for ArchiveProvider {
    fn kinds(&self) -> Vec<ArchiveKind> {
        ArchiveKind::ALL.to_vec()
    }

    fn begin(
        &self,
        kind: ArchiveKind,
        out: Box<dyn WriteStream>,
        location: Location,
    ) -> Result<Box<dyn ArchiveBuilder>, VfsError> {
        crate::write::begin(kind, out, location, self.options.scratch_dir.as_deref())
    }
}

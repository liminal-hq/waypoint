// The sorted, filtered index a listing holds: records keyed by `EntryId` and the view over them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};

use waypoint_protocol::EntryId;

use crate::group::{runs_of, GroupClock};
use crate::icon::extension;
use crate::model::{
    Entry, EntryKind, Filter, GitMark, GroupBy, GroupRun, IconGroup, KindFilter, PatchOp,
    SelectionSpec, SelectionSummary, SortKey, SortSpec,
};
use crate::order::{compare, natural_key, Sortable};
use crate::overlay::FolderMarks;
use crate::provider::{Change, ScannedEntry, TrashedMeta};
use crate::special::SpecialFolder;

/// Above this many changes in one batch the view is rebuilt and the patch is a `Reset`: editing a
/// huge view in place costs more than sorting it again.
const REBUILD_THRESHOLD: usize = 20_000;

/// One entry as the index stores it. The display name is derived when a range is read, so a
/// 500 000-entry folder keeps one copy of each name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    name: OsString,
    key: Box<[u8]>,
    /// The first 16 bytes of `key`, big-endian, so a full sort compares numbers held inline in the
    /// sort vector and only reaches for the key when two prefixes tie.
    prefix: u128,
    kind: EntryKind,
    link_target: Option<EntryKind>,
    link_pending: bool,
    group: IconGroup,
    special: Option<SpecialFolder>,
    size: Option<u64>,
    modified_ms: Option<i64>,
    hidden: bool,
    /// Set for an item in the Trash, whose `name` is an id and whose sort key is its original name.
    trashed: Option<Box<TrashedMeta>>,
    /// What an overlay says about the entry (`Index::set_marks`); never part of a scan.
    git: Option<GitMark>,
}

/// The first 16 bytes of `bytes` as a big-endian number, zero-padded.
fn prefix_of(bytes: &[u8]) -> u128 {
    let mut padded = [0u8; 16];
    let take = bytes.len().min(16);
    padded[..take].copy_from_slice(&bytes[..take]);
    u128::from_be_bytes(padded)
}

impl From<ScannedEntry> for Record {
    fn from(entry: ScannedEntry) -> Self {
        let key = match &entry.trashed {
            Some(meta) => natural_key(OsStr::new(&meta.display_name)),
            None => natural_key(&entry.name),
        };
        Self {
            prefix: prefix_of(&key),
            key,
            name: entry.name,
            kind: entry.kind,
            link_target: entry.link_target,
            link_pending: entry.link_pending,
            special: entry.special,
            group: entry.group,
            size: entry.size,
            modified_ms: entry.modified_ms,
            hidden: entry.hidden,
            trashed: entry.trashed,
            git: None,
        }
    }
}

impl Record {
    /// A record for a scanned entry, carrying whatever mark the overlay has for its name.
    fn from_scan(entry: ScannedEntry, marks: &FolderMarks) -> Self {
        let mut record = Record::from(entry);
        record.git = marks.mark_for(&record.name);
        record
    }

    /// The name people read: the original name of a trashed item, the entry's own otherwise.
    fn label(&self) -> &OsStr {
        match &self.trashed {
            Some(meta) => OsStr::new(&meta.display_name),
            None => &self.name,
        }
    }

    fn sortable(&self) -> Sortable<'_> {
        Sortable {
            name: &self.name,
            label: self.label(),
            key: &self.key,
            kind: self.kind,
            link_target: self.link_target,
            group: self.group.kind_class() as u8,
            size: self.size,
            modified_ms: self.modified_ms,
            deleted_ms: self.trashed.as_ref().map(|t| t.deleted_ms),
            git_rank: GitMark::sort_rank(self.git.as_ref()),
        }
    }

    fn is_directory(&self) -> bool {
        self.kind == EntryKind::Directory || self.link_target == Some(EntryKind::Directory)
    }

    fn visible(&self, filter: &Filter) -> bool {
        (filter.show_hidden || !self.hidden)
            && match filter.only {
                None => true,
                Some(KindFilter::Directories) => self.is_directory(),
                Some(KindFilter::Files) => !self.is_directory(),
            }
    }

    /// A number whose order is the order `compare` gives this entry's sort column, for the first
    /// bytes of it. Entries whose numbers tie are ordered by the full comparison.
    fn rank(&self, sort: SortSpec) -> u128 {
        let ascending = match sort.key {
            SortKey::Name => self.prefix,
            SortKey::Size => u128::from(self.size.unwrap_or(0)),
            SortKey::Modified => {
                u128::from((self.modified_ms.unwrap_or(i64::MIN) as u64) ^ (1 << 63))
            }
            SortKey::Deleted => u128::from(
                (self.trashed.as_ref().map_or(i64::MIN, |t| t.deleted_ms) as u64) ^ (1 << 63),
            ),
            SortKey::Git => u128::from(GitMark::sort_rank(self.git.as_ref())),
            SortKey::Kind => {
                let mut ext = [0u8; 8];
                let raw = extension(self.label().as_encoded_bytes());
                let take = raw.len().min(8);
                ext[..take].copy_from_slice(&raw[..take]);
                ext.make_ascii_lowercase();
                (u128::from(self.group.kind_class() as u8) << 64)
                    | u128::from(u64::from_be_bytes(ext))
            }
        };
        if sort.descending {
            !ascending
        } else {
            ascending
        }
    }

    fn to_entry(&self, id: u32) -> Entry {
        Entry {
            id: EntryId(id),
            name: match &self.trashed {
                Some(meta) => meta.display_name.clone(),
                None => self.name.to_string_lossy().into_owned(),
            },
            kind: self.kind,
            link_target: self.link_target,
            group: self.group,
            special: self.special,
            size: self.size,
            modified_ms: self.modified_ms,
            hidden: self.hidden,
            original_path: self.trashed.as_ref().map(|t| t.original_path.clone()),
            deleted_ms: self.trashed.as_ref().map(|t| t.deleted_ms),
            git: self.git,
        }
    }

    fn to_scanned(&self) -> ScannedEntry {
        ScannedEntry {
            name: self.name.clone(),
            kind: self.kind,
            link_target: self.link_target,
            link_pending: self.link_pending,
            group: self.group,
            special: self.special,
            size: self.size,
            modified_ms: self.modified_ms,
            hidden: self.hidden,
            trashed: self.trashed.clone(),
        }
    }
}

/// Entries by id, and the ordered, filtered list of ids a person sees.
///
/// Ids are positions in `records` and are never reused, so a removed entry leaves a hole and an id
/// stays unambiguous for the life of the listing. Names map back to ids lazily, because only a
/// listing that changes needs the map.
pub(crate) struct Index {
    records: Vec<Option<Record>>,
    view: Vec<u32>,
    sort: SortSpec,
    filter: Filter,
    names: Option<HashMap<OsString, u32>>,
    /// What an overlay says about the entries, applied to every record that arrives.
    marks: FolderMarks,
    /// What "today" is for the Modified groups, fixed for as long as the order stands and read
    /// again from `clock_source` whenever the view is rebuilt.
    clock: GroupClock,
    clock_source: fn() -> GroupClock,
}

/// The entry a staged change touches: what it was before the batch and what it becomes.
struct Staged {
    old: Option<Record>,
    new: Option<Record>,
}

fn runs(sorted: &[u32]) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for &at in sorted {
        match out.last_mut() {
            Some((start, count)) if *start + *count == at => *count += 1,
            _ => out.push((at, 1)),
        }
    }
    out
}

impl Index {
    pub fn new(sort: SortSpec, filter: Filter) -> Self {
        Self::with_clock(sort, filter, GroupClock::now)
    }

    /// An index that reads the time from `clock_source` (a test's fixed one, for one).
    pub fn with_clock(sort: SortSpec, filter: Filter, clock_source: fn() -> GroupClock) -> Self {
        Self {
            records: Vec::new(),
            view: Vec::new(),
            sort,
            filter,
            names: None,
            marks: FolderMarks::default(),
            clock: clock_source(),
            clock_source,
        }
    }

    pub fn sort(&self) -> SortSpec {
        self.sort
    }

    pub fn filter(&self) -> Filter {
        self.filter
    }

    /// How many entries are in the view.
    pub fn count(&self) -> u32 {
        self.view.len() as u32
    }

    /// Replaces the contents with a scan's entries, numbering them from zero.
    pub fn load(&mut self, entries: Vec<ScannedEntry>) {
        let marks = &self.marks;
        self.records = entries
            .into_iter()
            .map(|e| Some(Record::from_scan(e, marks)))
            .collect();
        self.names = None;
        self.rebuild();
    }

    pub fn set_sort(&mut self, sort: SortSpec) {
        self.sort = sort;
        self.rebuild();
    }

    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
        self.rebuild();
    }

    fn rec(&self, id: u32) -> &Record {
        self.records[id as usize]
            .as_ref()
            .expect("the view only holds live entries")
    }

    fn cmp_records(&self, a: &Record, b: &Record) -> Ordering {
        compare(self.sort, &self.clock, &a.sortable(), &b.sortable())
    }

    fn cmp_ids(&self, a: u32, b: u32) -> Ordering {
        self.cmp_records(self.rec(a), self.rec(b))
    }

    /// Filters and sorts every record into a fresh view.
    ///
    /// The sort runs over small items that carry the folder flag and a numeric rank inline, which
    /// keeps the comparison out of the records (and the cache) for nearly every pair.
    fn rebuild(&mut self) {
        if self.sort.group_by == GroupBy::Modified {
            self.clock = (self.clock_source)();
        }
        if self.sort.group_by != GroupBy::None {
            // The group comes before the sort column, so the numeric ranks below do not apply.
            let (sort, filter, clock) = (self.sort, self.filter, self.clock);
            let records = &self.records;
            let mut ids: Vec<u32> = records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.as_ref().is_some_and(|r| r.visible(&filter)))
                .map(|(id, _)| id as u32)
                .collect();
            ids.sort_unstable_by(|&a, &b| {
                let (a, b) = (
                    records[a as usize].as_ref().expect("live"),
                    records[b as usize].as_ref().expect("live"),
                );
                compare(sort, &clock, &a.sortable(), &b.sortable())
            });
            self.view = ids;
            return;
        }
        struct Item {
            rank: u128,
            /// The first 16 bytes of the name key: the tie-break every column shares.
            tie: u128,
            id: u32,
            folder: bool,
        }
        let (sort, filter, clock) = (self.sort, self.filter, self.clock);
        let mut items: Vec<Item> = self
            .records
            .iter()
            .enumerate()
            .filter_map(|(id, r)| {
                let r = r.as_ref().filter(|r| r.visible(&filter))?;
                Some(Item {
                    rank: r.rank(sort),
                    tie: r.prefix,
                    id: id as u32,
                    folder: r.is_directory(),
                })
            })
            .collect();
        let records = &self.records;
        items.sort_unstable_by(|a, b| {
            if sort.directories_first && a.folder != b.folder {
                return b.folder.cmp(&a.folder);
            }
            // The name prefix is only a valid tie-break when the rank already covers the whole
            // sort column; a Kind rank sees just the first bytes of the extension.
            let tie = if sort.key == SortKey::Kind {
                Ordering::Equal
            } else {
                a.tie.cmp(&b.tie)
            };
            a.rank.cmp(&b.rank).then(tie).then_with(|| {
                let (a, b) = (
                    records[a.id as usize].as_ref().expect("live"),
                    records[b.id as usize].as_ref().expect("live"),
                );
                compare(sort, &clock, &a.sortable(), &b.sortable())
            })
        });
        self.view = items.into_iter().map(|item| item.id).collect();
    }

    /// The groups of the view in order, each a contiguous run of rows; empty when the sort does not
    /// group. One pass over the view, so a listing sends them whole instead of patching them.
    pub fn groups(&self) -> Vec<GroupRun> {
        runs_of(
            self.sort,
            &self.clock,
            self.view.iter().map(|&id| self.rec(id).sortable()),
        )
    }

    /// Up to `count` entries from view position `start`.
    pub fn range(&self, start: u32, count: u32) -> Vec<Entry> {
        let start = (start as usize).min(self.view.len());
        let end = start.saturating_add(count as usize).min(self.view.len());
        self.view[start..end]
            .iter()
            .map(|&id| self.rec(id).to_entry(id))
            .collect()
    }

    /// The names a selection covers, in view order. Ids the view does not hold do not count, and an
    /// id given twice counts once.
    pub fn selected_names(&self, selection: &SelectionSpec) -> Vec<&OsStr> {
        let (ids, chosen) = match selection {
            SelectionSpec::Chosen { ids } => (ids, true),
            SelectionSpec::AllExcept { ids } => (ids, false),
        };
        let listed: std::collections::HashSet<u32> = ids.iter().map(|id| id.0).collect();
        self.view
            .iter()
            .filter(|id| listed.contains(id) == chosen)
            .map(|&id| self.rec(id).name.as_os_str())
            .collect()
    }

    /// What a selection adds up to over the current view. Ids the view does not hold (gone, or
    /// filtered out) do not count. O(n) over the view for "all except", O(chosen) otherwise, and it
    /// allocates nothing per row.
    pub fn summarise(&self, selection: &SelectionSpec) -> SelectionSummary {
        let in_view = |id: EntryId| {
            self.records
                .get(id.0 as usize)
                .and_then(Option::as_ref)
                .filter(|r| r.visible(&self.filter))
        };
        let distinct = |ids: &[EntryId]| -> Vec<EntryId> {
            let mut ids = ids.to_vec();
            ids.sort_unstable();
            ids.dedup();
            ids
        };
        match selection {
            SelectionSpec::Chosen { ids } => {
                let (mut count, mut total_size) = (0u64, 0u64);
                for id in distinct(ids) {
                    if let Some(record) = in_view(id) {
                        count += 1;
                        total_size = total_size.saturating_add(record.size.unwrap_or(0));
                    }
                }
                SelectionSummary { count, total_size }
            }
            SelectionSpec::AllExcept { ids } => {
                let mut count = self.view.len() as u64;
                let mut total_size = 0u64;
                for &id in &self.view {
                    total_size = total_size.saturating_add(self.rec(id).size.unwrap_or(0));
                }
                for id in distinct(ids) {
                    if let Some(record) = in_view(id) {
                        count -= 1;
                        total_size -= record.size.unwrap_or(0).min(total_size);
                    }
                }
                SelectionSummary { count, total_size }
            }
        }
    }

    /// The real name of a live entry.
    pub fn name_of(&self, id: u32) -> Option<&OsStr> {
        self.records
            .get(id as usize)?
            .as_ref()
            .map(|r| r.name.as_os_str())
    }

    /// The position of an entry in the view, or `None` if it is filtered out or gone.
    pub fn position_of(&self, id: u32) -> Option<u32> {
        let record = self.records.get(id as usize)?.as_ref()?;
        if !record.visible(&self.filter) {
            return None;
        }
        self.find(record).map(|at| at as u32)
    }

    fn find(&self, record: &Record) -> Option<usize> {
        self.view
            .binary_search_by(|&id| self.cmp_records(self.rec(id), record))
            .ok()
    }

    pub fn id_of(&mut self, name: &OsStr) -> Option<u32> {
        self.ensure_names();
        self.names.as_ref().and_then(|n| n.get(name).copied())
    }

    fn ensure_names(&mut self) {
        if self.names.is_none() {
            self.names = Some(
                self.records
                    .iter()
                    .enumerate()
                    .filter_map(|(id, r)| r.as_ref().map(|r| (r.name.clone(), id as u32)))
                    .collect(),
            );
        }
    }

    /// What changed between this index and a fresh scan of the same folder, by name.
    ///
    /// A symlink the scan left unresolved (past its budget) does not undo a target this index has
    /// already resolved.
    pub fn diff(&self, fresh: Vec<ScannedEntry>) -> Vec<Change> {
        let mut fresh: HashMap<OsString, ScannedEntry> =
            fresh.into_iter().map(|e| (e.name.clone(), e)).collect();
        let mut changes = Vec::new();
        for record in self.records.iter().flatten() {
            match fresh.remove(&record.name) {
                None => changes.push(Change::Remove(record.name.clone())),
                Some(mut now) => {
                    let before = record.to_scanned();
                    if now.link_pending && !before.link_pending {
                        now.link_pending = false;
                        now.link_target = before.link_target;
                        now.size = before.size;
                        now.modified_ms = before.modified_ms;
                        now.group = before.group;
                    }
                    if now != before {
                        changes.push(Change::Upsert(now));
                    }
                }
            }
        }
        changes.extend(fresh.into_values().map(Change::Upsert));
        changes
    }

    /// Entries whose symlink target has not been resolved yet.
    pub fn pending_links(&self) -> Vec<ScannedEntry> {
        self.records
            .iter()
            .flatten()
            .filter(|r| r.link_pending)
            .map(Record::to_scanned)
            .collect()
    }

    /// Applies a batch of changes by name and returns the patch that takes a cache of the old view
    /// to the new one, in the order a cache applies it: removals from the highest position down,
    /// then insertions from the lowest up (each at its final position), then in-place updates (at
    /// final positions). An entry whose place in the order changes (a rename, or a new size under a
    /// size sort) is a removal plus an insertion with the same `EntryId`, so a cache keyed by id
    /// keeps it without a `Reset`.
    #[cfg(test)]
    pub fn apply(&mut self, changes: Vec<Change>) -> Vec<PatchOp> {
        self.apply_tracking(changes, &mut Vec::new())
    }

    /// `apply`, also reporting in `moved` the ids of entries that were removed and inserted again
    /// by this patch because their place in the order changed, so a consumer can tell "this entry
    /// moved" from "this entry is gone" and keep, for example, its selection. A `Reset` moves none.
    pub fn apply_tracking(&mut self, changes: Vec<Change>, moved: &mut Vec<u32>) -> Vec<PatchOp> {
        moved.clear();
        if changes.is_empty() {
            return Vec::new();
        }
        self.ensure_names();
        let mut names = self.names.take().expect("built above");
        let mut staged: HashMap<u32, Staged> = HashMap::new();

        let stage = |records: &Vec<Option<Record>>,
                     staged: &mut HashMap<u32, Staged>,
                     id: u32,
                     new: Option<Record>| {
            staged
                .entry(id)
                .and_modify(|s| s.new = new.clone())
                .or_insert_with(|| Staged {
                    old: records[id as usize].clone(),
                    new,
                });
        };

        let total = changes.len();
        for change in changes {
            match change {
                Change::Upsert(entry) => {
                    let id = *names.entry(entry.name.clone()).or_insert_with(|| {
                        self.records.push(None);
                        (self.records.len() - 1) as u32
                    });
                    stage(
                        &self.records,
                        &mut staged,
                        id,
                        Some(Record::from_scan(entry, &self.marks)),
                    );
                }
                Change::Remove(name) => {
                    if let Some(id) = names.remove(&name) {
                        stage(&self.records, &mut staged, id, None);
                    }
                }
                Change::Rename { from, to } => match names.remove(&from) {
                    None => {
                        let id = *names.entry(to.name.clone()).or_insert_with(|| {
                            self.records.push(None);
                            (self.records.len() - 1) as u32
                        });
                        stage(
                            &self.records,
                            &mut staged,
                            id,
                            Some(Record::from_scan(to, &self.marks)),
                        );
                    }
                    Some(id) => {
                        // Renaming over an existing name replaces that entry.
                        if let Some(replaced) = names.insert(to.name.clone(), id) {
                            if replaced != id {
                                stage(&self.records, &mut staged, replaced, None);
                            }
                        }
                        stage(
                            &self.records,
                            &mut staged,
                            id,
                            Some(Record::from_scan(to, &self.marks)),
                        );
                    }
                },
            }
        }
        self.names = Some(names);
        self.commit(staged, total, moved)
    }

    /// Replaces the overlay's marks and returns the patch (see `apply_tracking`) for the entries
    /// whose mark changed: a changed mark is an update in place, or a move when the sort is by
    /// Git.
    pub fn set_marks(&mut self, marks: FolderMarks, moved: &mut Vec<u32>) -> Vec<PatchOp> {
        moved.clear();
        if self.marks == marks {
            return Vec::new();
        }
        let default_changed = self.marks.default != marks.default;
        let mut touched: Vec<OsString> = Vec::new();
        if default_changed {
            touched.extend(self.records.iter().flatten().map(|r| r.name.clone()));
        } else {
            for (name, mark) in &self.marks.names {
                if marks.names.get(name) != Some(mark) {
                    touched.push(name.clone());
                }
            }
            for (name, mark) in &marks.names {
                if self.marks.names.get(name) != Some(mark) {
                    touched.push(name.clone());
                }
            }
        }
        self.ensure_names();
        let names = self.names.take().expect("built above");
        let mut staged: HashMap<u32, Staged> = HashMap::new();
        for name in touched {
            let Some(&id) = names.get(&name) else {
                continue;
            };
            let Some(old) = self.records[id as usize].clone() else {
                continue;
            };
            let mut new = old.clone();
            new.git = marks.mark_for(&new.name);
            if new != old {
                staged.insert(
                    id,
                    Staged {
                        old: Some(old),
                        new: Some(new),
                    },
                );
            }
        }
        self.names = Some(names);
        self.marks = marks;
        let total = staged.len();
        if total == 0 {
            return Vec::new();
        }
        self.commit(staged, total, moved)
    }

    /// Works out the patch for staged edits, applies them to the records and the view.
    fn commit(
        &mut self,
        staged: HashMap<u32, Staged>,
        total: usize,
        moved: &mut Vec<u32>,
    ) -> Vec<PatchOp> {
        // Work out, against the old view, who leaves, who arrives and who changes in place.
        let filter = self.filter;
        let mut remove_at: Vec<u32> = Vec::new();
        let mut insert_ids: Vec<u32> = Vec::new();
        let mut update_ids: Vec<u32> = Vec::new();
        let mut consistent = true;
        for (&id, st) in &staged {
            let old = st.old.as_ref().filter(|r| r.visible(&filter));
            let new = st.new.as_ref().filter(|r| r.visible(&filter));
            match (old, new) {
                (None, None) => {}
                (Some(old), None) => match self.find(old) {
                    Some(at) => remove_at.push(at as u32),
                    None => consistent = false,
                },
                (None, Some(_)) => insert_ids.push(id),
                (Some(old), Some(new)) => {
                    if self.cmp_records(old, new) == Ordering::Equal {
                        if old != new {
                            update_ids.push(id);
                        }
                    } else {
                        match self.find(old) {
                            Some(at) => remove_at.push(at as u32),
                            None => consistent = false,
                        }
                        moved.push(id);
                        insert_ids.push(id);
                    }
                }
            }
        }
        for (id, st) in staged {
            self.records[id as usize] = st.new;
        }

        if !consistent || total > REBUILD_THRESHOLD {
            moved.clear();
            self.rebuild();
            return vec![PatchOp::Reset];
        }

        remove_at.sort_unstable();
        remove_at.dedup();
        let removals = runs(&remove_at);
        if !remove_at.is_empty() {
            let mut gone = remove_at.iter().copied().peekable();
            let mut position = 0u32;
            self.view.retain(|_| {
                let drop = gone.peek() == Some(&position);
                if drop {
                    gone.next();
                }
                position += 1;
                !drop
            });
        }

        // Merge the arrivals into the view, noting where each lands.
        insert_ids.sort_unstable_by(|&a, &b| self.cmp_ids(a, b));
        let mut landed: Vec<u32> = Vec::with_capacity(insert_ids.len());
        if !insert_ids.is_empty() {
            let mut merged = Vec::with_capacity(self.view.len() + insert_ids.len());
            let mut taken = 0;
            for &id in &insert_ids {
                let upto = taken
                    + self.view[taken..]
                        .partition_point(|&v| self.cmp_ids(v, id) == Ordering::Less);
                merged.extend_from_slice(&self.view[taken..upto]);
                landed.push(merged.len() as u32);
                merged.push(id);
                taken = upto;
            }
            merged.extend_from_slice(&self.view[taken..]);
            self.view = merged;
        }

        let mut updated: Vec<u32> = Vec::with_capacity(update_ids.len());
        for id in update_ids {
            match self.find(self.rec(id)) {
                Some(at) => updated.push(at as u32),
                None => {
                    moved.clear();
                    self.rebuild();
                    return vec![PatchOp::Reset];
                }
            }
        }
        updated.sort_unstable();

        let mut ops = Vec::new();
        for (at, count) in removals.into_iter().rev() {
            ops.push(PatchOp::Remove { at, count });
        }
        for (at, count) in runs(&landed) {
            ops.push(PatchOp::Insert { at, count });
        }
        for (at, count) in runs(&updated) {
            ops.push(PatchOp::Update { at, count });
        }
        ops
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SortKey;

    fn file(name: &str, size: u64) -> ScannedEntry {
        ScannedEntry {
            name: name.into(),
            kind: EntryKind::File,
            link_target: None,
            link_pending: false,
            special: None,
            group: IconGroup::Other,
            size: Some(size),
            modified_ms: Some(0),
            hidden: name.starts_with('.'),
            trashed: None,
        }
    }

    fn folder(name: &str) -> ScannedEntry {
        ScannedEntry {
            kind: EntryKind::Directory,
            group: IconGroup::Folder,
            size: None,
            ..file(name, 0)
        }
    }

    fn names(index: &Index) -> Vec<String> {
        index
            .range(0, u32::MAX)
            .into_iter()
            .map(|e| e.name)
            .collect()
    }

    fn loaded(entries: &[ScannedEntry]) -> Index {
        let mut index = Index::new(SortSpec::default(), Filter::default());
        index.load(entries.to_vec());
        index
    }

    /// Replays `ops` on a list of names the way a cache does, using `after` for inserted names.
    fn replay(before: &[String], ops: &[PatchOp], after: &[String]) -> Vec<String> {
        let mut cache: Vec<Option<String>> = before.iter().cloned().map(Some).collect();
        for op in ops {
            match *op {
                PatchOp::Remove { at, count } => {
                    cache.drain(at as usize..(at + count) as usize);
                }
                PatchOp::Insert { at, count } => {
                    for i in 0..count as usize {
                        cache.insert(at as usize + i, None);
                    }
                }
                PatchOp::Update { at, count } => {
                    for i in at..at + count {
                        cache[i as usize] = None;
                    }
                }
                PatchOp::Reset => cache = vec![None; after.len()],
            }
        }
        // Stale or missing slots are refetched from the final view.
        cache
            .into_iter()
            .enumerate()
            .map(|(i, slot)| slot.unwrap_or_else(|| after[i].clone()))
            .collect()
    }

    #[test]
    fn a_kind_sort_with_long_shared_extension_prefixes_stays_searchable() {
        let mut index = Index::new(
            SortSpec {
                key: SortKey::Kind,
                descending: false,
                directories_first: true,
                ..SortSpec::default()
            },
            Filter::default(),
        );
        index.load(vec![
            file("zzz.abcdefghY", 1),
            file("aaa.abcdefghX", 1),
            file("mmm.abcdefghZ", 1),
            file("bbb.abcdefghX", 1),
        ]);
        assert_eq!(
            names(&index),
            [
                "aaa.abcdefghX",
                "bbb.abcdefghX",
                "zzz.abcdefghY",
                "mmm.abcdefghZ"
            ]
        );
        for id in 0..4 {
            assert_eq!(
                index.position_of(id).map(|at| index.view[at as usize]),
                Some(id)
            );
        }
        let before = names(&index);
        let ops = index.apply(vec![Change::Upsert(file("ccc.abcdefghX", 2))]);
        let after = names(&index);
        assert_eq!(after[2], "ccc.abcdefghX");
        assert_eq!(replay(&before, &ops, &after), after);
    }

    #[test]
    fn folders_come_first_and_names_are_natural() {
        let index = loaded(&[file("b10", 1), folder("z"), file("b9", 1), folder("A")]);
        assert_eq!(names(&index), ["A", "z", "b9", "b10"]);
    }

    #[test]
    fn hidden_entries_are_filtered_and_the_filter_can_change() {
        let mut index = loaded(&[file(".git", 1), file("a", 1), folder(".cache"), folder("d")]);
        assert_eq!(names(&index), ["d", "a"]);
        index.set_filter(Filter {
            show_hidden: true,
            only: None,
        });
        assert_eq!(names(&index), [".cache", "d", ".git", "a"]);
        index.set_filter(Filter {
            show_hidden: true,
            only: Some(KindFilter::Directories),
        });
        assert_eq!(names(&index), [".cache", "d"]);
        index.set_filter(Filter {
            show_hidden: false,
            only: Some(KindFilter::Files),
        });
        assert_eq!(names(&index), ["a"]);
    }

    #[test]
    fn an_upsert_of_a_new_name_inserts_at_its_sorted_place() {
        let mut index = loaded(&[file("a", 1), file("c", 1), file("e", 1)]);
        let before = names(&index);
        let ops = index.apply(vec![
            Change::Upsert(file("b", 1)),
            Change::Upsert(file("d", 1)),
            Change::Upsert(file("f", 1)),
        ]);
        assert_eq!(
            ops,
            [
                PatchOp::Insert { at: 1, count: 1 },
                PatchOp::Insert { at: 3, count: 1 },
                PatchOp::Insert { at: 5, count: 1 },
            ]
        );
        assert_eq!(names(&index), ["a", "b", "c", "d", "e", "f"]);
        assert_eq!(replay(&before, &ops, &names(&index)), names(&index));
    }

    #[test]
    fn removals_run_from_the_highest_position_down_and_coalesce() {
        let mut index = loaded(&[
            file("a", 1),
            file("b", 1),
            file("c", 1),
            file("d", 1),
            file("e", 1),
        ]);
        let ops = index.apply(vec![
            Change::Remove("b".into()),
            Change::Remove("c".into()),
            Change::Remove("e".into()),
            Change::Remove("missing".into()),
        ]);
        assert_eq!(
            ops,
            [
                PatchOp::Remove { at: 4, count: 1 },
                PatchOp::Remove { at: 1, count: 2 },
            ]
        );
        assert_eq!(names(&index), ["a", "d"]);
    }

    #[test]
    fn a_change_that_keeps_the_order_is_an_update_in_place() {
        let mut index = loaded(&[file("a", 1), file("b", 1)]);
        let ops = index.apply(vec![Change::Upsert(file("b", 99))]);
        assert_eq!(ops, [PatchOp::Update { at: 1, count: 1 }]);
        // Setting the same attributes again is not a change at all.
        assert!(index.apply(vec![Change::Upsert(file("b", 99))]).is_empty());
    }

    #[test]
    fn a_rename_keeps_its_id_and_moves_with_a_remove_and_an_insert() {
        let mut index = loaded(&[file("a", 1), file("b", 1), file("c", 1)]);
        let id = index.id_of(OsStr::new("a")).unwrap();
        let before = names(&index);
        let ops = index.apply(vec![Change::Rename {
            from: "a".into(),
            to: file("z", 1),
        }]);
        assert_eq!(
            ops,
            [
                PatchOp::Remove { at: 0, count: 1 },
                PatchOp::Insert { at: 2, count: 1 },
            ]
        );
        assert_eq!(names(&index), ["b", "c", "z"]);
        assert_eq!(index.id_of(OsStr::new("z")), Some(id));
        assert_eq!(index.id_of(OsStr::new("a")), None);
        assert_eq!(index.position_of(id), Some(2));
        assert_eq!(replay(&before, &ops, &names(&index)), names(&index));
    }

    #[test]
    fn a_move_in_the_order_is_reported_but_a_removal_an_arrival_and_an_update_are_not() {
        let mut index = loaded(&[file("a", 1), file("b", 1), file("c", 1)]);
        let a = index.id_of(OsStr::new("a")).unwrap();
        let mut moved = Vec::new();
        index.apply_tracking(
            vec![Change::Rename {
                from: "a".into(),
                to: file("z", 1),
            }],
            &mut moved,
        );
        assert_eq!(moved, [a]);
        // Gone, new, and changed in place: none of them moved.
        index.apply_tracking(
            vec![
                Change::Remove("b".into()),
                Change::Upsert(file("d", 1)),
                Change::Upsert(file("c", 99)),
            ],
            &mut moved,
        );
        assert!(moved.is_empty());
    }

    #[test]
    fn renaming_over_an_existing_name_replaces_it() {
        let mut index = loaded(&[file("a", 1), file("b", 1)]);
        let a = index.id_of(OsStr::new("a")).unwrap();
        index.apply(vec![Change::Rename {
            from: "a".into(),
            to: file("b", 7),
        }]);
        assert_eq!(names(&index), ["b"]);
        assert_eq!(index.id_of(OsStr::new("b")), Some(a));
    }

    #[test]
    fn changing_the_sort_column_value_moves_the_entry() {
        let mut index = loaded(&[file("a", 10), file("b", 20), file("c", 30)]);
        index.set_sort(SortSpec {
            key: SortKey::Size,
            ..SortSpec::default()
        });
        assert_eq!(names(&index), ["a", "b", "c"]);
        let ops = index.apply(vec![Change::Upsert(file("a", 25))]);
        assert_eq!(
            ops,
            [
                PatchOp::Remove { at: 0, count: 1 },
                PatchOp::Insert { at: 1, count: 1 },
            ]
        );
        assert_eq!(names(&index), ["b", "a", "c"]);
    }

    #[test]
    fn a_hidden_entry_appears_when_it_stops_being_hidden() {
        let mut index = loaded(&[file("a", 1), file(".x", 1)]);
        let ops = index.apply(vec![Change::Upsert(ScannedEntry {
            hidden: false,
            ..file(".x", 1)
        })]);
        assert_eq!(ops, [PatchOp::Insert { at: 0, count: 1 }]);
        // A new hidden file changes nothing the view shows.
        assert!(index.apply(vec![Change::Upsert(file(".y", 1))]).is_empty());
        assert_eq!(names(&index), [".x", "a"]);
    }

    #[test]
    fn a_large_batch_rebuilds_with_a_reset() {
        let mut index = loaded(&[file("a", 1)]);
        let changes = (0..REBUILD_THRESHOLD + 1)
            .map(|i| Change::Upsert(file(&format!("f{i}"), 1)))
            .collect();
        assert_eq!(index.apply(changes), [PatchOp::Reset]);
        assert_eq!(index.count() as usize, REBUILD_THRESHOLD + 2);
    }

    #[test]
    fn diffing_a_fresh_scan_yields_additions_removals_and_changes_only() {
        let index = loaded(&[file("keep", 1), file("gone", 1), file("grown", 1)]);
        let mut changes = index.diff(vec![file("keep", 1), file("grown", 9), file("new", 1)]);
        changes.sort_by_key(|c| format!("{c:?}"));
        assert_eq!(changes.len(), 3);
        assert!(changes.contains(&Change::Remove("gone".into())));
        assert!(changes.contains(&Change::Upsert(file("grown", 9))));
        assert!(changes.contains(&Change::Upsert(file("new", 1))));
    }

    #[test]
    fn an_unresolved_link_in_a_fresh_scan_keeps_the_resolved_target() {
        let resolved = ScannedEntry {
            kind: EntryKind::Symlink,
            link_target: Some(EntryKind::Directory),
            ..file("link", 0)
        };
        let index = loaded(std::slice::from_ref(&resolved));
        let pending = ScannedEntry {
            link_target: None,
            link_pending: true,
            special: None,
            ..resolved
        };
        assert!(index.diff(vec![pending]).is_empty());
    }

    #[test]
    fn ids_are_never_reused() {
        let mut index = loaded(&[file("a", 1)]);
        let a = index.id_of(OsStr::new("a")).unwrap();
        index.apply(vec![Change::Remove("a".into())]);
        index.apply(vec![Change::Upsert(file("a", 1))]);
        assert_ne!(index.id_of(OsStr::new("a")), Some(a));
    }

    #[test]
    fn a_rebuilt_view_is_sorted_under_every_sort_and_direction() {
        let mut seed = 0x1357_9BDF_2468_ACE0u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let exts = [
            "txt",
            "TXT",
            "png",
            "rs",
            "",
            "tar.gz",
            "averyveryverylongextension",
        ];
        let entries: Vec<ScannedEntry> = (0..600)
            .map(|i| {
                let name = format!(
                    "{}{}_{}.{}",
                    ["a", "B", "é", "file", "Zed"][next() as usize % 5],
                    next() % 30,
                    i,
                    exts[next() as usize % exts.len()]
                );
                ScannedEntry {
                    size: (next() % 3 != 0).then(|| next() % 50),
                    modified_ms: (next() % 4 != 0).then(|| (next() % 40) as i64 - 20),
                    kind: if next() % 6 == 0 {
                        EntryKind::Directory
                    } else {
                        EntryKind::File
                    },
                    group: [IconGroup::Image, IconGroup::Code, IconGroup::Other]
                        [next() as usize % 3],
                    ..file(&name, 0)
                }
            })
            .collect();
        for key in [
            SortKey::Name,
            SortKey::Size,
            SortKey::Modified,
            SortKey::Kind,
            SortKey::Git,
        ] {
            for descending in [false, true] {
                for directories_first in [false, true] {
                    let sort = SortSpec {
                        key,
                        descending,
                        directories_first,
                        ..SortSpec::default()
                    };
                    let mut index = Index::new(
                        sort,
                        Filter {
                            show_hidden: true,
                            only: None,
                        },
                    );
                    index.load(entries.clone());
                    for pair in index.view.windows(2) {
                        assert_eq!(
                            index.cmp_ids(pair[0], pair[1]),
                            Ordering::Less,
                            "{sort:?}: {:?} then {:?}",
                            index.rec(pair[0]).name,
                            index.rec(pair[1]).name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn random_batches_always_replay_to_the_new_view() {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut index = loaded(&[]);
        for round in 0..300 {
            let before = names(&index);
            let mut changes = Vec::new();
            for _ in 0..(next() % 6) {
                let name = format!("n{}", next() % 40);
                changes.push(match next() % 4 {
                    0 => Change::Remove(name.into()),
                    1 => Change::Rename {
                        from: name.into(),
                        to: file(&format!("n{}", next() % 40), next() % 5),
                    },
                    _ => Change::Upsert(file(&name, next() % 5)),
                });
            }
            let ops = index.apply(changes);
            let after = names(&index);
            // Updates refetch from the final view, so compare the structure: same names, same order.
            assert_eq!(
                replay(&before, &ops, &after),
                after,
                "round {round}: {ops:?}"
            );
            let mut sorted = after.clone();
            sorted.dedup();
            assert_eq!(sorted.len(), after.len(), "names stay unique");
        }
    }

    // -- overlay marks ----------------------------------------------------------------------

    use crate::model::GitChange;

    fn marked(change: GitChange) -> GitMark {
        GitMark {
            unstaged: Some(change),
            ..GitMark::default()
        }
    }

    fn marks(list: &[(&str, GitChange)]) -> FolderMarks {
        FolderMarks {
            default: None,
            names: list
                .iter()
                .map(|(name, change)| (OsString::from(name), marked(*change)))
                .collect(),
        }
    }

    fn gits(index: &Index) -> Vec<(String, Option<GitMark>)> {
        index
            .range(0, u32::MAX)
            .into_iter()
            .map(|e| (e.name, e.git))
            .collect()
    }

    fn by_git() -> SortSpec {
        SortSpec {
            key: SortKey::Git,
            ..SortSpec::default()
        }
    }

    #[test]
    fn marks_decorate_rows_in_place_when_the_sort_ignores_them() {
        let mut index = loaded(&[file("a", 1), file("b", 1), file("c", 1)]);
        let before = names(&index);
        let ops = index.set_marks(marks(&[("b", GitChange::Modified)]), &mut Vec::new());
        assert_eq!(ops, vec![PatchOp::Update { at: 1, count: 1 }]);
        assert_eq!(names(&index), before);
        assert_eq!(
            gits(&index),
            [
                ("a".to_owned(), None),
                ("b".to_owned(), Some(marked(GitChange::Modified))),
                ("c".to_owned(), None)
            ]
        );
        // The same marks again change nothing.
        assert!(index
            .set_marks(marks(&[("b", GitChange::Modified)]), &mut Vec::new())
            .is_empty());
        // Taking one away updates just that row.
        let ops = index.set_marks(FolderMarks::default(), &mut Vec::new());
        assert_eq!(ops, vec![PatchOp::Update { at: 1, count: 1 }]);
    }

    #[test]
    fn a_default_mark_decorates_every_row_and_a_named_one_wins() {
        let mut index = loaded(&[file("a", 1), file("b", 1)]);
        let mut set = marks(&[("b", GitChange::Modified)]);
        set.default = Some(marked(GitChange::Ignored));
        index.set_marks(set, &mut Vec::new());
        assert_eq!(
            gits(&index),
            [
                ("a".to_owned(), Some(marked(GitChange::Ignored))),
                ("b".to_owned(), Some(marked(GitChange::Modified)))
            ]
        );
    }

    #[test]
    fn a_git_sort_puts_conflicts_and_edits_first_clean_next_and_ignored_last() {
        let mut index = Index::new(by_git(), Filter::default());
        index.load(vec![
            file("a-clean", 1),
            file("b-ignored", 1),
            file("c-modified", 1),
            file("d-conflict", 1),
            file("e-new", 1),
        ]);
        let before = names(&index);
        let mut moved = Vec::new();
        let ops = index.set_marks(
            marks(&[
                ("b-ignored", GitChange::Ignored),
                ("c-modified", GitChange::Modified),
                ("d-conflict", GitChange::Conflicted),
                ("e-new", GitChange::Untracked),
            ]),
            &mut moved,
        );
        let after = names(&index);
        assert_eq!(
            after,
            ["d-conflict", "c-modified", "e-new", "a-clean", "b-ignored"]
        );
        assert_eq!(replay(&before, &ops, &after), after);
        assert!(
            !moved.is_empty(),
            "rows that changed place are reported as moved"
        );
    }

    #[test]
    fn a_row_that_arrives_after_the_marks_takes_its_mark() {
        let mut index = loaded(&[file("a", 1)]);
        index.set_marks(marks(&[("later", GitChange::Untracked)]), &mut Vec::new());
        index.apply(vec![Change::Upsert(file("later", 2))]);
        let later = gits(&index)
            .into_iter()
            .find(|(name, _)| name == "later")
            .unwrap();
        assert_eq!(later.1, Some(marked(GitChange::Untracked)));
        // A rescan that changes the row keeps its mark.
        index.apply(vec![Change::Upsert(file("later", 3))]);
        assert!(gits(&index)
            .iter()
            .any(|(n, g)| n == "later" && g.is_some()));
    }

    #[test]
    fn a_scan_after_the_marks_arrived_applies_them() {
        let mut index = Index::new(SortSpec::default(), Filter::default());
        index.set_marks(marks(&[("b", GitChange::Added)]), &mut Vec::new());
        index.load(vec![file("a", 1), file("b", 1)]);
        assert_eq!(gits(&index)[1].1, Some(marked(GitChange::Added)));
        assert_eq!(gits(&index)[0].1, None);
    }

    #[test]
    fn a_diff_against_a_fresh_scan_ignores_the_marks() {
        let mut index = loaded(&[file("a", 1), file("b", 1)]);
        index.set_marks(marks(&[("a", GitChange::Modified)]), &mut Vec::new());
        assert!(index.diff(vec![file("a", 1), file("b", 1)]).is_empty());
    }

    #[test]
    fn random_marks_always_replay_to_the_new_view_under_every_sort() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let changes = [
            GitChange::Modified,
            GitChange::Added,
            GitChange::Deleted,
            GitChange::Untracked,
            GitChange::Ignored,
            GitChange::Conflicted,
        ];
        for sort in [SortSpec::default(), by_git()] {
            let entries: Vec<ScannedEntry> =
                (0..30).map(|n| file(&format!("n{n:02}"), n % 4)).collect();
            let mut index = Index::new(sort, Filter::default());
            index.load(entries.clone());
            for round in 0..200 {
                let before = names(&index);
                let mut set = FolderMarks::default();
                if next() % 8 == 0 {
                    set.default = Some(marked(changes[(next() % 6) as usize]));
                }
                for _ in 0..(next() % 12) {
                    set.names.insert(
                        OsString::from(format!("n{:02}", next() % 30)),
                        marked(changes[(next() % 6) as usize]),
                    );
                }
                let ops = index.set_marks(set.clone(), &mut Vec::new());
                let after = names(&index);
                assert_eq!(replay(&before, &ops, &after), after, "round {round}");
                // The view is what a fresh load with the same marks would build.
                let mut fresh = Index::new(sort, Filter::default());
                fresh.set_marks(set, &mut Vec::new());
                fresh.load(entries.clone());
                assert_eq!(after, names(&fresh), "round {round}");
                assert_eq!(gits(&index), gits(&fresh), "round {round}");
            }
        }
    }

    // -- grouping ---------------------------------------------------------------------------

    use crate::model::{GroupKey, ModifiedBucket, SizeBand};

    // Wednesday 2026-10-07 at noon UTC.
    const NOW: i64 = 1_791_374_400_000;
    const DAY: i64 = 86_400_000;

    fn fixed() -> GroupClock {
        GroupClock::new(NOW, 0)
    }

    fn by(group_by: GroupBy) -> SortSpec {
        SortSpec {
            group_by,
            ..SortSpec::default()
        }
    }

    fn grouped(sort: SortSpec, entries: &[ScannedEntry]) -> Index {
        let mut index = Index::with_clock(sort, Filter::default(), fixed);
        index.load(entries.to_vec());
        index
    }

    /// The groups as `(key, names)` so a test reads what each header holds.
    fn shape(index: &Index) -> Vec<(GroupKey, Vec<String>)> {
        let all = names(index);
        index
            .groups()
            .into_iter()
            .map(|g| {
                (
                    g.key,
                    all[g.start as usize..(g.start + g.count) as usize].to_vec(),
                )
            })
            .collect()
    }

    fn size_group(band: SizeBand) -> GroupKey {
        GroupKey::Size { band }
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn an_ungrouped_view_has_no_groups() {
        let index = loaded(&[file("a", 1), file("b", 2)]);
        assert!(index.groups().is_empty());
        assert!(loaded(&[]).groups().is_empty());
    }

    #[test]
    fn an_empty_folder_has_no_groups_under_any_grouping() {
        for group_by in [
            GroupBy::Kind,
            GroupBy::Modified,
            GroupBy::Size,
            GroupBy::Name,
            GroupBy::Type,
        ] {
            assert!(grouped(by(group_by), &[]).groups().is_empty());
        }
    }

    #[test]
    fn size_groups_are_runs_in_band_order_with_the_sort_inside() {
        let index = grouped(
            by(GroupBy::Size),
            &[
                file("big", 200_000_000),
                file("b", 5),
                folder("dir"),
                file("a", 7),
                file("empty", 0),
                file("mid", 500_000),
            ],
        );
        assert_eq!(
            shape(&index),
            [
                (size_group(SizeBand::Unspecified), strings(&["dir"])),
                (size_group(SizeBand::Empty), strings(&["empty"])),
                (size_group(SizeBand::Tiny), strings(&["a", "b"])),
                (size_group(SizeBand::Medium), strings(&["mid"])),
                (size_group(SizeBand::Gigantic), strings(&["big"])),
            ]
        );
    }

    #[test]
    fn name_groups_use_the_initial_and_put_symbols_first() {
        let index = grouped(
            by(GroupBy::Name),
            &[
                file("banana", 1),
                file("Apple", 1),
                file("avocado", 1),
                file("9lives", 1),
                file("_x", 1),
                folder("Zoo"),
            ],
        );
        let initials: Vec<String> = shape(&index)
            .into_iter()
            .map(|(key, _)| match key {
                GroupKey::Name { initial } => initial,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(initials, ["#", "A", "B", "Z"]);
        assert_eq!(shape(&index)[1].1, ["Apple", "avocado"]);
    }

    #[test]
    fn a_descending_name_sort_runs_the_name_groups_backwards() {
        let sort = SortSpec {
            descending: true,
            ..by(GroupBy::Name)
        };
        let index = grouped(sort, &[file("a1", 1), file("b1", 1), file("b2", 1)]);
        assert_eq!(
            shape(&index),
            [
                (
                    GroupKey::Name {
                        initial: "B".into()
                    },
                    strings(&["b2", "b1"])
                ),
                (
                    GroupKey::Name {
                        initial: "A".into()
                    },
                    strings(&["a1"])
                ),
            ]
        );
    }

    #[test]
    fn kind_groups_follow_the_icon_groups() {
        let image = ScannedEntry {
            group: IconGroup::Image,
            ..file("pic.png", 1)
        };
        let index = grouped(by(GroupBy::Kind), &[file("z", 1), image, folder("d")]);
        assert_eq!(
            shape(&index),
            [
                (
                    GroupKey::Kind {
                        group: IconGroup::Folder
                    },
                    strings(&["d"])
                ),
                (
                    GroupKey::Kind {
                        group: IconGroup::Image
                    },
                    strings(&["pic.png"])
                ),
                (
                    GroupKey::Kind {
                        group: IconGroup::Other
                    },
                    strings(&["z"])
                ),
            ]
        );
    }

    #[test]
    fn type_groups_put_folders_and_extensionless_names_ahead_of_the_extensions() {
        let index = grouped(
            by(GroupBy::Type),
            &[
                file("b.TXT", 1),
                file("a.rs", 1),
                file("README", 1),
                folder("d"),
                file("c.txt", 1),
            ],
        );
        let t = |extension: &str| GroupKey::Type {
            extension: extension.into(),
        };
        assert_eq!(
            shape(&index),
            [
                (
                    GroupKey::Kind {
                        group: IconGroup::Folder
                    },
                    strings(&["d"])
                ),
                (t(""), strings(&["README"])),
                (t("rs"), strings(&["a.rs"])),
                (t("txt"), strings(&["b.TXT", "c.txt"])),
            ]
        );
    }

    fn aged(name: &str, days: i64) -> ScannedEntry {
        ScannedEntry {
            modified_ms: Some(NOW - days * DAY),
            ..file(name, 1)
        }
    }

    #[test]
    fn modified_groups_run_newest_first_and_follow_an_ascending_modified_sort_backwards() {
        let entries = [
            aged("old", 400),
            aged("now", 0),
            aged("week", 4),
            aged("yday", 1),
        ];
        let bucket = |bucket| GroupKey::Modified { bucket };
        let newest_first = grouped(by(GroupBy::Modified), &entries);
        assert_eq!(
            shape(&newest_first),
            [
                (bucket(ModifiedBucket::Today), strings(&["now"])),
                (bucket(ModifiedBucket::Yesterday), strings(&["yday"])),
                (bucket(ModifiedBucket::Last7Days), strings(&["week"])),
                (GroupKey::Year { year: 2025 }, strings(&["old"])),
            ]
        );
        let oldest_first = grouped(
            SortSpec {
                key: SortKey::Modified,
                ..by(GroupBy::Modified)
            },
            &entries,
        );
        assert_eq!(
            shape(&oldest_first)
                .into_iter()
                .map(|(_, rows)| rows[0].clone())
                .collect::<Vec<_>>(),
            ["old", "week", "yday", "now"]
        );
    }

    #[test]
    fn a_row_that_changes_group_moves_and_the_boundaries_follow() {
        let mut index = grouped(
            by(GroupBy::Size),
            &[file("a", 5), file("b", 6), file("c", 5_000_000)],
        );
        let id = index.id_of(OsStr::new("a")).unwrap();
        let before = names(&index);
        let mut moved = Vec::new();
        let ops = index.apply_tracking(vec![Change::Upsert(file("a", 2_000_000))], &mut moved);
        assert_eq!(moved, [id]);
        assert_eq!(replay(&before, &ops, &names(&index)), names(&index));
        assert_eq!(
            shape(&index),
            [
                (size_group(SizeBand::Tiny), strings(&["b"])),
                (size_group(SizeBand::Large), strings(&["a", "c"])),
            ]
        );
        // The last row of a group leaving takes its header with it.
        index.apply(vec![Change::Upsert(file("b", 2_000_000))]);
        assert_eq!(
            shape(&index),
            [(size_group(SizeBand::Large), strings(&["a", "b", "c"]))]
        );
        // A change inside a group updates in place and leaves the boundaries alone.
        let ops = index.apply(vec![Change::Upsert(file("a", 2_000_001))]);
        assert_eq!(ops, [PatchOp::Update { at: 0, count: 1 }]);
        assert_eq!(index.groups().len(), 1);
    }

    #[test]
    fn a_new_row_can_open_a_group_and_the_last_removal_empties_the_view() {
        let mut index = grouped(by(GroupBy::Size), &[file("a", 5)]);
        index.apply(vec![Change::Upsert(file("z", 0))]);
        assert_eq!(
            shape(&index),
            [
                (size_group(SizeBand::Empty), strings(&["z"])),
                (size_group(SizeBand::Tiny), strings(&["a"])),
            ]
        );
        index.apply(vec![Change::Remove("a".into()), Change::Remove("z".into())]);
        assert!(index.groups().is_empty());
    }

    #[test]
    fn showing_and_hiding_hidden_files_adds_and_removes_their_groups() {
        let mut index = grouped(by(GroupBy::Name), &[file("a", 1), file(".secret", 1)]);
        assert_eq!(index.groups().len(), 1);
        index.set_filter(Filter {
            show_hidden: true,
            only: None,
        });
        let keys: Vec<GroupKey> = index.groups().into_iter().map(|g| g.key).collect();
        assert_eq!(
            keys,
            [
                GroupKey::Name {
                    initial: "#".into()
                },
                GroupKey::Name {
                    initial: "A".into()
                }
            ]
        );
        index.set_filter(Filter::default());
        assert_eq!(index.groups().len(), 1);
        assert_eq!(index.groups()[0].count, 1);
    }

    #[test]
    fn changing_the_group_re_sorts_the_view() {
        let mut index = grouped(SortSpec::default(), &[file("a", 50_000), file("b", 1)]);
        assert_eq!(names(&index), ["a", "b"]);
        index.set_sort(SortSpec {
            key: SortKey::Size,
            descending: true,
            ..by(GroupBy::Size)
        });
        assert_eq!(names(&index), ["a", "b"]);
        index.set_sort(by(GroupBy::Size));
        assert_eq!(names(&index), ["b", "a"]);
        index.set_sort(SortSpec::default());
        assert_eq!(names(&index), ["a", "b"]);
        assert!(index.groups().is_empty());
    }

    #[test]
    fn groups_are_contiguous_and_distinct_under_every_grouping_sort_and_direction() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let entries: Vec<ScannedEntry> = (0..500)
            .map(|i| {
                let name = format!(
                    "{}{}_{}.{}",
                    ["a", "B", "é", "9", "_", "Zed"][next() as usize % 6],
                    next() % 20,
                    i,
                    ["txt", "TXT", "png", "rs", "", "gz"][next() as usize % 6]
                );
                ScannedEntry {
                    size: (next() % 4 != 0).then(|| {
                        [0, 5, 20_000, 400_000, 3_000_000, 90_000_000, 5_000_000_000]
                            [next() as usize % 7]
                    }),
                    modified_ms: (next() % 5 != 0)
                        .then(|| NOW - ((next() % 900) as i64 - 20) * DAY),
                    kind: if next() % 6 == 0 {
                        EntryKind::Directory
                    } else {
                        EntryKind::File
                    },
                    group: [IconGroup::Image, IconGroup::Code, IconGroup::Other]
                        [next() as usize % 3],
                    ..file(&name, 0)
                }
            })
            .collect();
        for group_by in [
            GroupBy::Kind,
            GroupBy::Modified,
            GroupBy::Size,
            GroupBy::Name,
            GroupBy::Type,
        ] {
            for key in [
                SortKey::Name,
                SortKey::Size,
                SortKey::Modified,
                SortKey::Kind,
                SortKey::Git,
            ] {
                for descending in [false, true] {
                    for directories_first in [false, true] {
                        let sort = SortSpec {
                            key,
                            descending,
                            directories_first,
                            group_by,
                        };
                        let mut index = Index::with_clock(
                            sort,
                            Filter {
                                show_hidden: true,
                                only: None,
                            },
                            fixed,
                        );
                        index.load(entries.clone());
                        for pair in index.view.windows(2) {
                            assert_eq!(index.cmp_ids(pair[0], pair[1]), Ordering::Less, "{sort:?}");
                        }
                        let groups = index.groups();
                        let mut at = 0;
                        for group in &groups {
                            assert_eq!(group.start, at, "{sort:?}: runs are contiguous");
                            assert!(group.count > 0);
                            at += group.count;
                        }
                        assert_eq!(at, index.count(), "{sort:?}: runs cover the view");
                        let mut keys: Vec<String> =
                            groups.iter().map(|g| format!("{:?}", g.key)).collect();
                        let total = keys.len();
                        keys.sort();
                        keys.dedup();
                        assert_eq!(keys.len(), total, "{sort:?}: a group never recurs");
                    }
                }
            }
        }
    }

    #[test]
    fn random_batches_under_a_grouping_replay_and_match_a_fresh_build() {
        let mut seed = 0x1234_5678_9ABC_DEF1u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let sizes = [0u64, 3, 20_000, 400_000, 3_000_000];
        let mut index = grouped(by(GroupBy::Size), &[]);
        for round in 0..300 {
            let before = names(&index);
            let mut changes = Vec::new();
            for _ in 0..(next() % 6) {
                let name = format!("n{}", next() % 30);
                changes.push(match next() % 4 {
                    0 => Change::Remove(name.into()),
                    1 => Change::Rename {
                        from: name.into(),
                        to: file(&format!("n{}", next() % 30), sizes[next() as usize % 5]),
                    },
                    _ => Change::Upsert(file(&name, sizes[next() as usize % 5])),
                });
            }
            let ops = index.apply(changes);
            let after = names(&index);
            assert_eq!(
                replay(&before, &ops, &after),
                after,
                "round {round}: {ops:?}"
            );
            let live: Vec<ScannedEntry> = index
                .records
                .iter()
                .flatten()
                .map(Record::to_scanned)
                .collect();
            let fresh = grouped(by(GroupBy::Size), &live);
            assert_eq!(
                names(&fresh),
                after,
                "round {round}: same order as a rebuild"
            );
            assert_eq!(fresh.groups(), index.groups(), "round {round}");
        }
    }
}

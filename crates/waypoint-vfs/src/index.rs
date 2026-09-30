// The sorted, filtered index a listing holds: records keyed by `EntryId` and the view over them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};

use waypoint_protocol::EntryId;

use crate::icon::extension;
use crate::model::{
    Entry, EntryKind, Filter, IconGroup, KindFilter, PatchOp, SelectionSpec, SelectionSummary,
    SortKey, SortSpec,
};
use crate::order::{compare, natural_key, Sortable};
use crate::provider::{Change, ScannedEntry};

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
    size: Option<u64>,
    modified_ms: Option<i64>,
    hidden: bool,
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
        let key = natural_key(&entry.name);
        Self {
            prefix: prefix_of(&key),
            key,
            name: entry.name,
            kind: entry.kind,
            link_target: entry.link_target,
            link_pending: entry.link_pending,
            group: entry.group,
            size: entry.size,
            modified_ms: entry.modified_ms,
            hidden: entry.hidden,
        }
    }
}

impl Record {
    fn sortable(&self) -> Sortable<'_> {
        Sortable {
            name: &self.name,
            key: &self.key,
            kind: self.kind,
            link_target: self.link_target,
            group: self.group as u8,
            size: self.size,
            modified_ms: self.modified_ms,
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
            SortKey::Kind => {
                let mut ext = [0u8; 8];
                let raw = extension(self.name.as_encoded_bytes());
                let take = raw.len().min(8);
                ext[..take].copy_from_slice(&raw[..take]);
                ext.make_ascii_lowercase();
                (u128::from(self.group as u8) << 64) | u128::from(u64::from_be_bytes(ext))
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
            name: self.name.to_string_lossy().into_owned(),
            kind: self.kind,
            link_target: self.link_target,
            group: self.group,
            size: self.size,
            modified_ms: self.modified_ms,
            hidden: self.hidden,
        }
    }

    fn to_scanned(&self) -> ScannedEntry {
        ScannedEntry {
            name: self.name.clone(),
            kind: self.kind,
            link_target: self.link_target,
            link_pending: self.link_pending,
            group: self.group,
            size: self.size,
            modified_ms: self.modified_ms,
            hidden: self.hidden,
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
        Self {
            records: Vec::new(),
            view: Vec::new(),
            sort,
            filter,
            names: None,
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
        self.records = entries.into_iter().map(|e| Some(Record::from(e))).collect();
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
        compare(self.sort, &a.sortable(), &b.sortable())
    }

    fn cmp_ids(&self, a: u32, b: u32) -> Ordering {
        self.cmp_records(self.rec(a), self.rec(b))
    }

    /// Filters and sorts every record into a fresh view.
    ///
    /// The sort runs over small items that carry the folder flag and a numeric rank inline, which
    /// keeps the comparison out of the records (and the cache) for nearly every pair.
    fn rebuild(&mut self) {
        struct Item {
            rank: u128,
            /// The first 16 bytes of the name key: the tie-break every column shares.
            tie: u128,
            id: u32,
            folder: bool,
        }
        let (sort, filter) = (self.sort, self.filter);
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
                compare(sort, &a.sortable(), &b.sortable())
            })
        });
        self.view = items.into_iter().map(|item| item.id).collect();
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
    pub fn apply(&mut self, changes: Vec<Change>) -> Vec<PatchOp> {
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
                    stage(&self.records, &mut staged, id, Some(entry.into()));
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
                        stage(&self.records, &mut staged, id, Some(to.into()));
                    }
                    Some(id) => {
                        // Renaming over an existing name replaces that entry.
                        if let Some(replaced) = names.insert(to.name.clone(), id) {
                            if replaced != id {
                                stage(&self.records, &mut staged, replaced, None);
                            }
                        }
                        stage(&self.records, &mut staged, id, Some(to.into()));
                    }
                },
            }
        }
        self.names = Some(names);

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
                        insert_ids.push(id);
                    }
                }
            }
        }
        for (id, st) in staged {
            self.records[id as usize] = st.new;
        }

        if !consistent || total > REBUILD_THRESHOLD {
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
            group: IconGroup::Other,
            size: Some(size),
            modified_ms: Some(0),
            hidden: name.starts_with('.'),
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
        ] {
            for descending in [false, true] {
                for directories_first in [false, true] {
                    let sort = SortSpec {
                        key,
                        descending,
                        directories_first,
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
}

// A Trash that keeps its items in a folder of a provider, for tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::Provider;

use crate::exec::remove_all;
use crate::model::OpsError;
use crate::traits::{Clock, IdSource, Trash, TrashReceipt};

/// Moves trashed items into `dir` under a number and remembers where they came from. Everything it
/// does goes through the provider, so it sees the same faults and the same sandbox as the
/// operations.
pub struct FakeTrash {
    provider: Arc<dyn Provider>,
    dir: VfsPath,
    ids: Arc<dyn IdSource>,
    clock: Arc<dyn Clock>,
    entries: Mutex<BTreeMap<String, TrashReceipt>>,
    available: Mutex<Result<(), String>>,
}

impl FakeTrash {
    /// `dir` must be on the same volume as what will be trashed, and need not exist yet.
    pub fn new(
        provider: Arc<dyn Provider>,
        dir: VfsPath,
        ids: Arc<dyn IdSource>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            provider,
            dir,
            ids,
            clock,
            entries: Mutex::new(BTreeMap::new()),
            available: Mutex::new(Ok(())),
        }
    }

    /// Makes `available` report this, to test the refusal when there is no Trash.
    pub fn set_available(&self, state: Result<(), String>) {
        *self.available.lock().unwrap() = state;
    }

    /// How many items are in the Trash.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The receipts of every item in the Trash, in the order of their ids.
    pub fn receipts(&self) -> Vec<TrashReceipt> {
        self.entries.lock().unwrap().values().cloned().collect()
    }

    /// Where a trashed item sits, as the Trash view would show it.
    pub fn trashed_path(&self, receipt: &TrashReceipt) -> VfsPath {
        self.dir
            .join(&receipt.id)
            .expect("a receipt id is a plain name")
    }

    pub fn trashed_location(&self, receipt: &TrashReceipt) -> Location {
        self.trashed_path(receipt).to_location()
    }

    fn parse(&self, location: &Location) -> Result<VfsPath, OpsError> {
        VfsPath::from_location(location).map_err(|_| {
            OpsError::from(VfsError::InvalidLocation {
                input: location.uri.clone(),
            })
        })
    }
}

impl Trash for FakeTrash {
    fn available(&self) -> Result<(), String> {
        self.available.lock().unwrap().clone()
    }

    fn trash(&self, items: &[Location]) -> Vec<Result<TrashReceipt, OpsError>> {
        items
            .iter()
            .map(|item| {
                self.available()
                    .map_err(|reason| OpsError::TrashUnavailable { reason })?;
                let from = self.parse(item)?;
                match self.provider.create_dir(&self.dir) {
                    Ok(()) | Err(VfsError::AlreadyExists { .. }) => {}
                    Err(error) => return Err(error.into()),
                }
                let id = self.ids.next().to_string();
                let receipt = TrashReceipt {
                    id,
                    original: item.clone(),
                    deleted_at: self.clock.now_ms(),
                };
                self.provider
                    .rename(&from, &self.trashed_path(&receipt), false)?;
                self.entries
                    .lock()
                    .unwrap()
                    .insert(receipt.id.clone(), receipt.clone());
                Ok(receipt)
            })
            .collect()
    }

    fn restore(&self, receipt: &TrashReceipt) -> Result<Location, OpsError> {
        let original = self.parse(&receipt.original)?;
        self.provider
            .rename(&self.trashed_path(receipt), &original, false)?;
        self.entries.lock().unwrap().remove(&receipt.id);
        Ok(receipt.original.clone())
    }

    fn delete(&self, receipt: &TrashReceipt) -> Result<(), OpsError> {
        remove_all(self.provider.as_ref(), &self.trashed_path(receipt))?;
        self.entries.lock().unwrap().remove(&receipt.id);
        Ok(())
    }

    fn empty(&self, older_than_days: Option<u32>) -> Result<u64, OpsError> {
        let cutoff = older_than_days.map(|d| self.clock.now_ms() - i64::from(d) * 86_400_000);
        let doomed: Vec<TrashReceipt> = self
            .entries
            .lock()
            .unwrap()
            .values()
            .filter(|r| cutoff.is_none_or(|c| r.deleted_at < c))
            .cloned()
            .collect();
        let mut removed = 0;
        for receipt in doomed {
            self.delete(&receipt)?;
            removed += 1;
        }
        Ok(removed)
    }

    fn contains(&self, receipt: &TrashReceipt) -> Result<bool, OpsError> {
        Ok(self.entries.lock().unwrap().contains_key(&receipt.id))
    }

    fn receipt_for(&self, trashed: &Location) -> Result<TrashReceipt, OpsError> {
        self.entries
            .lock()
            .unwrap()
            .values()
            .find(|r| self.trashed_location(r).uri == trashed.uri)
            .cloned()
            .ok_or_else(|| OpsError::NotFound {
                location: trashed.clone(),
            })
    }
}

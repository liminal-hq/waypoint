// A journal storage that keeps its generations in memory, for tests, with the rotation and the
// corrupt-file handling the app's file-backed storage has.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::journal::{JournalDocument, JournalStorage, Loaded, SaveRequest, StorageError};

#[derive(Default)]
struct Slots {
    current: Option<String>,
    previous: Option<String>,
    saves: usize,
    set_aside: Vec<String>,
    fail_saves: bool,
}

/// Two generations as JSON text, the way the file holds them. `reopen` is a restart: the same
/// text, a new run, so the first save of it rotates again.
pub struct MemoryJournalStorage {
    slots: Arc<Mutex<Slots>>,
    rotated: AtomicBool,
}

impl Default for MemoryJournalStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryJournalStorage {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Mutex::new(Slots::default())),
            rotated: AtomicBool::new(false),
        }
    }

    /// The same stored text seen by a new run.
    pub fn reopen(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            rotated: AtomicBool::new(false),
        }
    }

    /// Replaces the latest generation with arbitrary text, to test what an unreadable file does.
    pub fn put_current_text(&self, text: &str) {
        self.slots.lock().unwrap().current = Some(text.to_owned());
    }

    pub fn current_text(&self) -> Option<String> {
        self.slots.lock().unwrap().current.clone()
    }

    pub fn previous_text(&self) -> Option<String> {
        self.slots.lock().unwrap().previous.clone()
    }

    /// The latest generation parsed, when it is a document.
    pub fn current_document(&self) -> Option<JournalDocument> {
        self.current_text()
            .and_then(|text| serde_json::from_str(&text).ok())
    }

    /// How many times `save` has written.
    pub fn saves(&self) -> usize {
        self.slots.lock().unwrap().saves
    }

    /// The names of the copies kept aside.
    pub fn set_aside_copies(&self) -> Vec<String> {
        self.slots.lock().unwrap().set_aside.clone()
    }

    /// Makes every save fail, as a full disk would.
    pub fn fail_saves(&self, fail: bool) {
        self.slots.lock().unwrap().fail_saves = fail;
    }
}

fn parse(text: &str) -> Result<JournalDocument, String> {
    serde_json::from_str(text).map_err(|e| e.to_string())
}

impl JournalStorage for MemoryJournalStorage {
    fn load(&self) -> Result<Loaded, StorageError> {
        let mut slots = self.slots.lock().unwrap();
        let mut aside = None;
        let mut why = None;
        let mut from_previous = false;
        if let Some(text) = slots.current.clone() {
            match parse(&text) {
                Ok(document) => {
                    return Ok(Loaded {
                        document: Some(document),
                        ..Loaded::default()
                    })
                }
                Err(error) => {
                    let name = format!("journal.json.corrupt-{}", slots.set_aside.len() + 1);
                    slots.set_aside.push(name.clone());
                    aside = Some(name);
                    why = Some(error);
                    from_previous = true;
                }
            }
        }
        if let Some(text) = slots.previous.clone() {
            match parse(&text) {
                Ok(document) => {
                    return Ok(Loaded {
                        document: Some(document),
                        from_previous,
                        unreadable: None,
                        set_aside: aside,
                    })
                }
                Err(error) => {
                    if aside.is_none() {
                        let name = format!("journal.json.corrupt-{}", slots.set_aside.len() + 1);
                        slots.set_aside.push(name.clone());
                        aside = Some(name);
                    }
                    why = Some(error);
                }
            }
        }
        Ok(Loaded {
            document: None,
            from_previous: false,
            unreadable: why,
            set_aside: aside,
        })
    }

    fn save(&self, document: &JournalDocument) -> Result<(), StorageError> {
        let text = serde_json::to_string(document).map_err(|e| StorageError::Io(e.to_string()))?;
        let mut slots = self.slots.lock().unwrap();
        if slots.fail_saves {
            return Err(StorageError::Io("the disk is full".to_owned()));
        }
        if !self.rotated.swap(true, Ordering::AcqRel) {
            // The first save of the run: what is stored now is how the run started. A copy that
            // cannot be read is not worth keeping over an earlier good one.
            if let Some(current) = slots.current.clone() {
                if parse(&current).is_ok() {
                    slots.previous = Some(current);
                }
            }
        }
        slots.current = Some(text);
        slots.saves += 1;
        Ok(())
    }

    fn set_aside(&self) -> Option<String> {
        let mut slots = self.slots.lock().unwrap();
        let name = format!("journal.json.corrupt-{}", slots.set_aside.len() + 1);
        slots.set_aside.push(name.clone());
        Some(name)
    }
}

/// Counts the save requests the journal makes.
#[derive(Debug, Default)]
pub struct CountingSaver(pub AtomicUsize);

impl CountingSaver {
    pub fn count(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

impl SaveRequest for CountingSaver {
    fn save_requested(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

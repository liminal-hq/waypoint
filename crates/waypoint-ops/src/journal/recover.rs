// Start-up recovery: reads the saved journal, cleans up after the jobs that were running when the
// app stopped, and reports what it found. It never resumes anything.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A pending record names the job, what it planned to act on and the folders it wrote partial files
// into. Recovery looks in those folders only, and removes only the entries named
// `.waypoint-partial-{job}-…` for that job. One of those is not always rubbish: a case-only rename
// moves its file aside under such a name between its two steps, and then the partial file is the
// only copy. When the original name is gone and a partial of that name is there, it is renamed
// back instead of removed.
//
// A replacement sets what it replaces aside under `.waypoint-replaced-{job}-{n}-{name}` and removes
// it only once the new entry is in place. A crash between the two leaves that entry as the only
// copy of what the name held, so recovery never removes one: where the name is free again it is
// renamed back (and listed as restored), and otherwise it stays and is listed as left.

use std::collections::HashSet;
use std::ffi::OsStr;

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, Provider};

use super::engine::{Journal, JournalDeps};
use super::model::{
    DiscardReason, Discarded, EntryState, InterruptedJob, JournalBody, PendingRecord,
    RecoveryReport, JOURNAL_VERSION,
};
use super::storage::JournalStorage;
use crate::exec::remove_all;
use crate::names::{file_name_of, same_path};
use crate::traits::Providers;

/// What a start-up load made of the saved journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recovery {
    /// The journal to start from: the saved entries, with the pending records cleared.
    pub body: JournalBody,
    pub report: RecoveryReport,
}

/// Reads the saved journal and cleans up after the jobs it finds pending, through `providers`.
/// A journal that cannot be used is left aside (see `JournalStorage::set_aside`) and the result
/// starts empty.
pub fn recover(storage: &dyn JournalStorage, providers: &Providers) -> Recovery {
    let mut report = RecoveryReport::default();
    let loaded = match storage.load() {
        Ok(loaded) => loaded,
        Err(error) => {
            report.discarded = Some(Discarded {
                reason: DiscardReason::StorageFailed {
                    why: error.to_string(),
                },
                set_aside: None,
            });
            return Recovery {
                body: JournalBody::default(),
                report,
            };
        }
    };
    report.from_previous = loaded.from_previous;
    let Some(document) = loaded.document else {
        if let Some(why) = loaded.unreadable {
            report.discarded = Some(Discarded {
                reason: DiscardReason::Corrupt { why },
                set_aside: loaded.set_aside,
            });
        }
        return Recovery {
            body: JournalBody::default(),
            report,
        };
    };
    if loaded.set_aside.is_some() {
        // The latest copy was bad and the one before it was used: tell the person where the bad
        // one went.
        report.discarded = Some(Discarded {
            reason: DiscardReason::Corrupt {
                why: "the latest copy could not be read".to_owned(),
            },
            set_aside: loaded.set_aside.clone(),
        });
    }
    if document.version != JOURNAL_VERSION {
        report.discarded = Some(Discarded {
            reason: DiscardReason::UnsupportedVersion {
                found: document.version,
                supported: JOURNAL_VERSION,
            },
            set_aside: loaded.set_aside.or_else(|| storage.set_aside()),
        });
        return Recovery {
            body: JournalBody::default(),
            report,
        };
    }
    let mut body = document.body;
    repair(&mut body, &mut report.repairs);
    for record in std::mem::take(&mut body.pending) {
        report.interrupted.push(sweep(providers, &record));
    }
    Recovery { body, report }
}

/// Drops what cannot be right: a second entry with an id already seen, and an applied entry with
/// nothing to undo.
fn repair(body: &mut JournalBody, notes: &mut Vec<String>) {
    let mut seen = HashSet::new();
    let before = body.entries.len();
    body.entries
        .retain(|e| seen.insert(e.id) && !(e.state == EntryState::Applied && e.inverse.is_empty()));
    let dropped = before - body.entries.len();
    if dropped > 0 {
        notes.push(format!(
            "dropped {dropped} undo entries that could not be used"
        ));
    }
}

/// What a job's partial files in one folder are for.
fn partial_rest<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    let after = name.strip_prefix(prefix)?;
    let digits = after.chars().take_while(char::is_ascii_digit).count();
    after[digits..].strip_prefix('-')
}

fn sweep(providers: &Providers, record: &PendingRecord) -> InterruptedJob {
    let mut done = InterruptedJob {
        job: record.job,
        kind: record.kind,
        label: record.label.clone(),
        at_ms: record.at_ms,
        planned: record.items.clone(),
        removed: Vec::new(),
        restored: Vec::new(),
        left: Vec::new(),
    };
    let prefix = format!(".waypoint-partial-{}-", record.job.0);
    let aside_prefix = format!(".waypoint-replaced-{}-", record.job.0);
    let mut visited: Vec<VfsPath> = Vec::new();
    for folder in &record.folders {
        let Ok((path, provider)) = providers.for_location(folder) else {
            done.left.push(folder.clone());
            continue;
        };
        let rule = provider.capabilities().case_rule;
        if visited.iter().any(|v| same_path(v, &path, rule)) {
            continue;
        }
        visited.push(path.clone());
        let entries = match provider.list(&path, &CancelToken::new(), 0, &mut |_| {}) {
            Ok(entries) => entries,
            Err(VfsError::NotFound { .. } | VfsError::NotADirectory { .. }) => continue,
            Err(_) => {
                done.left.push(folder.clone());
                continue;
            }
        };
        for entry in entries {
            let name = entry.name.to_string_lossy().into_owned();
            if let Some(rest) = partial_rest(&name, &aside_prefix) {
                let Ok(child) = path.join(&entry.name) else {
                    continue;
                };
                match restore_replaced(provider.as_ref(), &path, &child, &name, rest) {
                    Ok(original) => done.restored.push(original),
                    Err(()) => done.left.push(child.to_location()),
                }
                continue;
            }
            let Some(rest) = partial_rest(&name, &prefix) else {
                continue;
            };
            let Ok(child) = path.join(&entry.name) else {
                continue;
            };
            let location = child.to_location();
            match put_back(provider.as_ref(), record, &path, &child, rest) {
                Some(Ok(original)) => done.restored.push(original),
                Some(Err(())) => done.left.push(location),
                None => match remove_all(provider.as_ref(), &child) {
                    Ok(()) => done.removed.push(location),
                    Err(_) => done.left.push(location),
                },
            }
        }
    }
    done
}

/// What a replacement set aside under `aside` (named `aside_name`, with the original name `rest`)
/// goes back to when that name is free. `Err` leaves it where it is: the name is taken (the new
/// entry is in, and the old one is not ours to delete), the rename failed, or the waiting name was
/// cut short to fit so the original name is not certain.
fn restore_replaced(
    provider: &dyn Provider,
    folder: &VfsPath,
    aside: &VfsPath,
    aside_name: &str,
    rest: &str,
) -> Result<Location, ()> {
    // `sibling` cuts a long name to keep the whole under 255 bytes (and by up to a character).
    if rest.is_empty() || aside_name.len() + 4 > 255 {
        return Err(());
    }
    let original = folder.join(OsStr::new(rest)).map_err(|_| ())?;
    match provider.stat(&original) {
        Err(VfsError::NotFound { .. }) => provider
            .rename(aside, &original, false)
            .map(|()| original.to_location())
            .map_err(|_| ()),
        _ => Err(()),
    }
}

/// A case-only rename's file that was set aside under a partial name and whose own name is now
/// free is renamed back: `Some(Ok(location))`. `Some(Err)` when that failed, and `None` when the
/// partial is not such a file.
fn put_back(
    provider: &dyn Provider,
    record: &PendingRecord,
    folder: &VfsPath,
    partial: &VfsPath,
    rest: &str,
) -> Option<Result<Location, ()>> {
    let rule = provider.capabilities().case_rule;
    for pair in &record.renames {
        let Ok(from) = VfsPath::from_location(&pair.from) else {
            continue;
        };
        let Some(name) = file_name_of(&from) else {
            continue;
        };
        let beside = from.parent().is_some_and(|p| same_path(&p, folder, rule));
        // A long name is cut to fit when the partial name is made, so `rest` may be a prefix.
        let named = !rest.is_empty() && name.to_string_lossy().starts_with(rest);
        if !beside || !named || rest == OsStr::new("") {
            continue;
        }
        return match provider.stat(&from) {
            Err(VfsError::NotFound { .. }) => Some(
                provider
                    .rename(partial, &from, false)
                    .map(|()| from.to_location())
                    .map_err(|_| ()),
            ),
            _ => None,
        };
    }
    None
}

impl Journal {
    /// Opens the saved journal and recovers from what the last run left, as the app does at
    /// start-up. The journal that comes back has no pending records; the report says what became
    /// of each, and the app turns it into a notice.
    pub fn open(deps: JournalDeps, providers: &Providers) -> (Journal, RecoveryReport) {
        let recovery = recover(deps.storage.as_ref(), providers);
        let changed = recovery.report.needs_notice() || !recovery.report.repairs.is_empty();
        let mut journal = Journal::from_body(deps, recovery.body);
        journal.trim_to_depth();
        if changed {
            journal.request_save();
        }
        (journal, recovery.report)
    }
}

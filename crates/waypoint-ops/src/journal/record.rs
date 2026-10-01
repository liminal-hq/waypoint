// Turning what a job did into what the journal keeps: the label, the forward spec for a redo, and
// the write-ahead record stored before the job starts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use waypoint_path::VfsPath;
use waypoint_protocol::Location;

use super::engine::Recorded;
use super::model::{InverseStep, JournalEntry, PendingRecord, RenamePair};
use crate::exec::ExecReport;
use crate::model::{JobKind, JobOptions, JobRequest, Sources};
use crate::names::file_name_of;
use crate::plan::Plan;

/// The last component of a location, for a label.
fn name_of(location: &Location) -> String {
    VfsPath::from_location(location)
        .ok()
        .and_then(|path| file_name_of(&path))
        .map_or_else(
            || location.display.clone(),
            |name| name.to_string_lossy().into_owned(),
        )
}

fn parent_of(location: &Location) -> Option<Location> {
    VfsPath::from_location(location)
        .ok()
        .and_then(|path| path.parent())
        .map(|parent| parent.to_location())
}

fn quoted(name: &str) -> String {
    format!("\u{201c}{name}\u{201d}")
}

/// "Move 3 items to Trash": the verb and what it acts on, worded once for entries and for the
/// write-ahead record. `names` are the entries' names in order and `count` how many there are.
fn label_for(kind: JobKind, count: usize, names: &[String]) -> String {
    let one = names.first().map(|n| quoted(n));
    let items = |verb: &str, tail: &str| match (&one, count) {
        (Some(name), 1) => format!("{verb} {name}{tail}"),
        _ => format!("{verb} {count} items{tail}"),
    };
    match kind {
        JobKind::CreateFolder => format!("Create folder {}", one.unwrap_or_default()),
        JobKind::CreateFile => format!("Create file {}", one.unwrap_or_default()),
        JobKind::Rename => match (names.first(), names.get(1)) {
            (Some(old), Some(new)) => format!("Rename {} to {}", quoted(old), quoted(new)),
            _ => "Rename".to_owned(),
        },
        JobKind::Duplicate => items("Duplicate", ""),
        JobKind::Trash => items("Move", " to Trash"),
        JobKind::Restore => items("Restore", ""),
        JobKind::Delete => items("Delete", ""),
        JobKind::Copy => items("Copy", ""),
        JobKind::Move => items("Move", ""),
        JobKind::BatchRename => items("Rename", ""),
        JobKind::Undo { .. } => items("Undo", ""),
        JobKind::Redo { .. } => items("Redo", ""),
    }
}

fn locations_of(items: &[crate::plan::PlanItem]) -> Vec<Location> {
    items
        .iter()
        .filter_map(|i| i.source.as_ref().or(i.target.as_ref()))
        .map(VfsPath::to_location)
        .collect()
}

impl Recorded {
    /// What a job that ran (to its end or to where it stopped) leaves in the journal, or `None`
    /// when it did nothing a journal can reverse. `plan` is the plan the job ran.
    pub fn from_run(
        request: &JobRequest,
        plan: Option<&Plan>,
        report: &ExecReport,
    ) -> Option<Recorded> {
        if report.inverse.is_empty() {
            return None;
        }
        let kind = request.kind;
        let mut forward = request.clone();
        forward.options = JobOptions {
            conflict: None,
            ..request.options
        };
        let names: Vec<String>;
        let count: usize;
        match kind {
            JobKind::CreateFolder | JobKind::CreateFile => {
                let created = report.created.first()?;
                forward.sources = Sources::Locations { locations: vec![] };
                forward.destination = parent_of(created);
                forward.name = Some(name_of(created));
                names = vec![name_of(created)];
                count = 1;
            }
            JobKind::Rename => {
                let (from, to) = report.renamed.first()?;
                forward.sources = Sources::Locations {
                    locations: vec![from.clone()],
                };
                forward.destination = None;
                forward.name = Some(name_of(to));
                names = vec![name_of(from), name_of(to)];
                count = 1;
            }
            JobKind::Trash => {
                let originals: Vec<Location> =
                    report.trashed.iter().map(|r| r.original.clone()).collect();
                names = originals.iter().map(name_of).collect();
                count = originals.len();
                forward.sources = Sources::Locations {
                    locations: originals,
                };
            }
            _ => {
                let done: Vec<Location> = match plan {
                    Some(plan) => locations_of(&plan.items),
                    None => Vec::new(),
                };
                if matches!(forward.sources, Sources::Selection { .. }) && !done.is_empty() {
                    forward.sources = Sources::Locations {
                        locations: done.clone(),
                    };
                }
                if kind == JobKind::Duplicate {
                    // Only the sources whose copies exist.
                    if let Some(plan) = plan {
                        let sources: Vec<Location> = plan
                            .items
                            .iter()
                            .filter(|i| {
                                i.target
                                    .as_ref()
                                    .is_some_and(|t| report.created.contains(&t.to_location()))
                            })
                            .filter_map(|i| i.source.as_ref().map(VfsPath::to_location))
                            .collect();
                        if !sources.is_empty() {
                            forward.sources = Sources::Locations { locations: sources };
                        }
                    }
                    count = report.created.len();
                    names = if let Sources::Locations { locations } = &forward.sources {
                        locations.iter().map(name_of).collect()
                    } else {
                        Vec::new()
                    };
                } else {
                    count = report.inverse.len();
                    names = done.iter().map(name_of).collect();
                }
            }
        }
        Some(Recorded {
            kind,
            label: label_for(kind, count, &names),
            forward,
            inverse: report.inverse.clone(),
        })
    }
}

impl PendingRecord {
    /// The record of a job about to run `plan`. Only the folders a job writes partial files into
    /// are named, which is all recovery ever looks in.
    pub fn for_plan(
        job: crate::model::JobId,
        at_ms: i64,
        request: &JobRequest,
        plan: &Plan,
    ) -> PendingRecord {
        let mut folders: Vec<Location> = Vec::new();
        let mut note = |location: Option<Location>| {
            if let Some(location) = location {
                if !folders.contains(&location) {
                    folders.push(location);
                }
            }
        };
        note(plan.destination.as_ref().map(VfsPath::to_location));
        for item in &plan.items {
            note(
                item.target
                    .as_ref()
                    .and_then(VfsPath::parent)
                    .map(|p| p.to_location()),
            );
        }
        let renames = plan
            .items
            .iter()
            .filter(|_| request.kind == JobKind::Rename)
            .filter_map(|i| match (&i.source, &i.target) {
                (Some(from), Some(to)) => Some(RenamePair {
                    from: from.to_location(),
                    to: to.to_location(),
                }),
                _ => None,
            })
            .collect();
        let items = locations_of(&plan.items);
        let names: Vec<String> = match request.kind {
            JobKind::Rename => plan
                .items
                .iter()
                .flat_map(|i| [i.source.as_ref(), i.target.as_ref()])
                .flatten()
                .filter_map(file_name_of)
                .map(|n| n.to_string_lossy().into_owned())
                .collect(),
            _ => items.iter().map(name_of).collect(),
        };
        PendingRecord {
            job,
            at_ms,
            kind: request.kind,
            label: label_for(request.kind, plan.items.len(), &names),
            items,
            folders,
            renames,
        }
    }

    /// The record of an undo job about to apply `steps` of `entry`.
    pub fn for_undo(
        job: crate::model::JobId,
        at_ms: i64,
        entry: &JournalEntry,
        steps: &[InverseStep],
    ) -> PendingRecord {
        let mut folders: Vec<Location> = Vec::new();
        let mut renames = Vec::new();
        for step in steps {
            if let InverseStep::RemoveCreated { location, .. }
            | InverseStep::RemoveEmptyDir { location } = step
            {
                if let Some(place) = parent_of(location) {
                    if !folders.contains(&place) {
                        folders.push(place);
                    }
                }
            }
            if let InverseStep::Rename { from, to } | InverseStep::MoveBack { from, to } = step {
                for place in [parent_of(from), parent_of(to)].into_iter().flatten() {
                    if !folders.contains(&place) {
                        folders.push(place);
                    }
                }
                renames.push(RenamePair {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
        }
        PendingRecord {
            job,
            at_ms,
            kind: JobKind::Undo { of: entry.id },
            label: format!("Undo: {}", entry.label),
            items: steps.iter().map(InverseStep::subject).collect(),
            folders,
            renames,
        }
    }
}

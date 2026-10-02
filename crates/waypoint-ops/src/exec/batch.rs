// Runs a batch rename: the planned renames in their planned order, all or none (A47).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A rename is one atomic provider call that never overwrites, so at every moment each entry is under
// its old name, a temporary name or its new one. If a step fails or the job is cancelled, the steps
// already done are undone in reverse order (ignoring the cancel, as cleanup always does), and the
// job ends as if it had not run. Only when an undo step itself fails does something stay renamed;
// the report then says exactly which entries, and the journal records them, so Undo can finish the
// work.
//
// A temporary name is `.waypoint-rename-{job}-{n}-{name}` beside the entry. It is not a partial
// file: recovery after a crash reports one it finds and never removes it, because it holds the
// entry itself.

use std::collections::HashMap;
use std::ffi::OsStr;

use waypoint_path::VfsPath;
use waypoint_protocol::Location;
use waypoint_vfs::{child_path, CancelToken};

use super::{ExecEnv, ExecFailure, ExecReport, ExecSink};
use crate::journal::InverseStep;
use crate::model::{Counts, JobId, OpsError, Progress};
use crate::names::file_name_of;
use crate::plan::{BatchStep, Plan};
use crate::rename_clash::Place;

/// The prefix of the name an entry has while it is out of the way.
pub const TEMP_PREFIX: &str = ".waypoint-rename-";

struct Done {
    item: usize,
    from: VfsPath,
    to: VfsPath,
    to_place: Place,
}

struct Batch<'a> {
    env: &'a ExecEnv,
    job: JobId,
    plan: &'a Plan,
    temps: HashMap<usize, VfsPath>,
    done: Vec<Done>,
    progress: Progress,
}

impl Batch<'_> {
    fn temp_path(&mut self, item: usize) -> Result<VfsPath, OpsError> {
        if let Some(path) = self.temps.get(&item) {
            return Ok(path.clone());
        }
        let source = self.plan.items[item]
            .source
            .as_ref()
            .expect("a rename has a source");
        let folder = source.parent().ok_or_else(|| OpsError::Protected {
            location: source.to_location(),
        })?;
        let provider = self.env.providers.for_path(source)?;
        let name = file_name_of(source).unwrap_or_default();
        let prefix = format!("{TEMP_PREFIX}{}-{}-", self.job.0, self.env.ids.next());
        let mut text = name.to_string_lossy().into_owned();
        while prefix.len() + text.len() > 255 && text.pop().is_some() {}
        let text = text.trim_end_matches(['.', ' ']);
        let path = child_path(
            &folder,
            OsStr::new(&format!("{prefix}{text}")),
            provider.capabilities().case_rule,
        )?;
        self.temps.insert(item, path.clone());
        Ok(path)
    }

    fn place(&mut self, item: usize, place: Place) -> Result<VfsPath, OpsError> {
        let it = &self.plan.items[item];
        match place {
            Place::Source => Ok(it.source.clone().expect("a rename has a source")),
            Place::Target => Ok(it.target.clone().expect("a rename has a target")),
            Place::Temp => self.temp_path(item),
        }
    }

    fn apply(&mut self, step: &BatchStep) -> Result<(), OpsError> {
        let from = self.place(step.item, step.from)?;
        let to = self.place(step.item, step.to)?;
        let provider = self.env.providers.for_path(&from)?;
        if step.from == Place::Source {
            // The entry must still be what the plan saw.
            let entry = provider.stat(&from)?;
            if entry.kind != self.plan.items[step.item].kind {
                return Err(OpsError::ChangedSince {
                    location: from.to_location(),
                });
            }
        }
        provider.rename(&from, &to, false)?;
        self.done.push(Done {
            item: step.item,
            from,
            to,
            to_place: step.to,
        });
        Ok(())
    }

    /// Undoes the steps done, newest first. What cannot be undone stays in `done`.
    fn roll_back(&mut self) {
        let mut stuck = Vec::new();
        while let Some(step) = self.done.pop() {
            let undone = self
                .env
                .providers
                .for_path(&step.to)
                .map_err(|_| ())
                .and_then(|p| p.rename(&step.to, &step.from, false).map_err(|_| ()));
            if undone.is_err() {
                stuck.push(step);
            }
        }
        stuck.reverse();
        self.done = stuck;
    }

    /// What the steps still in place amount to.
    fn report(&self) -> ExecReport {
        let mut renamed: Vec<(usize, Location, Location)> = self
            .done
            .iter()
            .filter(|d| d.to_place == Place::Target)
            .map(|d| (d.item, d.from.to_location(), d.to.to_location()))
            .collect();
        renamed.sort_by_key(|(item, _, _)| *item);
        ExecReport {
            inverse: self
                .done
                .iter()
                .map(|d| InverseStep::Rename {
                    from: d.to.to_location(),
                    to: d.from.to_location(),
                })
                .collect(),
            renamed: renamed
                .into_iter()
                .map(|(_, from, to)| (from, to))
                .collect(),
            progress: self.progress.clone(),
            counts: Counts::default(),
            ..ExecReport::default()
        }
    }
}

pub(super) fn run(
    env: &ExecEnv,
    job: JobId,
    plan: &Plan,
    cancel: &CancelToken,
    sink: &mut dyn ExecSink,
) -> Result<ExecReport, Box<ExecFailure>> {
    let steps: &[BatchStep] = plan.batch.as_ref().map_or(&[], |b| b.steps.as_slice());
    let mut batch = Batch {
        env,
        job,
        plan,
        temps: HashMap::new(),
        done: Vec::new(),
        progress: Progress {
            items_total: plan.items.len() as u64,
            ..Progress::default()
        },
    };
    for step in steps {
        sink.between_items();
        let outcome = if cancel.is_cancelled() {
            Err(OpsError::Cancelled)
        } else {
            batch.apply(step)
        };
        match outcome {
            Ok(()) => {
                if step.to == Place::Target {
                    batch.progress.items_done += 1;
                    batch.progress.current = batch.done.last().and_then(|d| {
                        file_name_of(&d.to).map(|n| n.to_string_lossy().into_owned())
                    });
                    sink.progress(&batch.progress, &Counts::default());
                }
            }
            Err(error) => {
                let item = plan.items[step.item]
                    .source
                    .as_ref()
                    .map(VfsPath::to_location);
                batch.roll_back();
                batch.progress.items_done = batch.report().renamed.len() as u64;
                let report = batch.report();
                return Err(Box::new(ExecFailure {
                    error,
                    item,
                    done: report.renamed.len() as u64,
                    report,
                }));
            }
        }
    }
    Ok(batch.report())
}

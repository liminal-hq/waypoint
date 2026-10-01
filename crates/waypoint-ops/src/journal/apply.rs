// Undo and redo as jobs: planning them (which checks every step before anything is touched),
// applying the inverse steps, and filling in the fingerprints once a job has finished.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// An undo is all or nothing as far as the checks go: `check_steps` reads the world and refuses with
// a typed `UndoStale` before the first write, so a changed file never sees a partial undo. The steps
// then go in reverse order of how the job did them, each re-checked just before it runs. If one
// still fails (the disk, a permission, a race with another program) the undo stops there and says
// what was done and what remains; it does not try to roll the finished steps forward again, because
// a second round of writes on a file system that has just failed is the likelier way to lose data.
// The journal keeps the entry with only the steps still to do (`Journal::finish_undo`), so undoing
// again finishes the work and never repeats a step.
//
// A redo runs the entry's forward spec through the ordinary planner and executor, so it meets the
// same name checks and conflict handling as the first run.

use waypoint_path::VfsPath;
use waypoint_protocol::{Location, VfsError};
use waypoint_vfs::{CancelToken, EntryKind, Provider};

use super::engine::Journal;
use super::fingerprint::{fingerprint, verify};
use super::model::{InverseStep, JournalId, StaleReason};
use crate::exec::{remove_all, ExecEnv, ExecSink, Executor};
use crate::model::{Counts, JobId, JobKind, JobRequest, OpsError, Progress};
use crate::names::{file_name_of, same_name, same_path};
use crate::plan::{plan_with_progress, Plan, PlanCtx, PlanItem, PlanProgress};
use crate::traits::{Protected, Providers, Trash};

/// An undo, planned: the steps in the order they will be applied, all of them checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoPlan {
    pub entry: JournalId,
    pub steps: Vec<InverseStep>,
    pub plan: Plan,
}

/// A redo, planned: the ordinary plan of the entry's forward spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedoPlan {
    pub entry: JournalId,
    pub forward: JobRequest,
    pub plan: Plan,
}

/// What `prepare` made of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prepared {
    /// Any other job, planned as `plan` plans it.
    Plain(Plan),
    Undo(UndoPlan),
    Redo(RedoPlan),
}

/// Plans a request the way the plugin's worker does when a journal is in play: undo and redo
/// through the journal, everything else through `plan`. Reads only.
pub fn prepare(
    request: &JobRequest,
    ctx: &PlanCtx<'_>,
    journal: &Journal,
    progress: &mut dyn FnMut(&PlanProgress),
) -> Result<Prepared, OpsError> {
    match request.kind {
        JobKind::Undo { of } => {
            let steps = journal.undo_steps(of)?;
            check_steps(ctx.providers, ctx.trash, ctx.protected, &steps)?;
            let items = steps
                .iter()
                .filter_map(|step| {
                    let source = VfsPath::from_location(&step.subject()).ok()?;
                    Some(PlanItem {
                        source: Some(source),
                        target: None,
                        kind: EntryKind::Other,
                        size: None,
                        entries: 1,
                        bytes: 0,
                        case_only: false,
                    })
                })
                .collect::<Vec<_>>();
            let plan = Plan {
                kind: request.kind,
                total_items: steps.len() as u64,
                total_bytes: 0,
                items,
                destination: None,
                same_volume: true,
                conflicts: Vec::new(),
                warnings: Vec::new(),
            };
            Ok(Prepared::Undo(UndoPlan {
                entry: of,
                steps,
                plan,
            }))
        }
        JobKind::Redo { of } => {
            let forward = journal.redo_forward(of)?;
            let plan = plan_with_progress(&forward, ctx, progress)?;
            Ok(Prepared::Redo(RedoPlan {
                entry: of,
                forward,
                plan,
            }))
        }
        _ => Ok(Prepared::Plain(plan_with_progress(request, ctx, progress)?)),
    }
}

fn stale(location: &Location, reason: StaleReason) -> OpsError {
    OpsError::UndoStale {
        location: location.clone(),
        reason,
    }
}

fn exists(provider: &dyn Provider, path: &VfsPath) -> Result<bool, OpsError> {
    match provider.stat(path) {
        Ok(_) => Ok(true),
        Err(VfsError::NotFound { .. } | VfsError::NotADirectory { .. }) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Where a step's names are, for the check that the destination name is free.
fn vacates(step: &InverseStep) -> Option<&Location> {
    match step {
        InverseStep::Rename { from, .. } | InverseStep::MoveBack { from, .. } => Some(from),
        InverseStep::RemoveCreated { location, .. } => Some(location),
        _ => None,
    }
}

/// Checks that `to` can take an entry: its folder is there and the name is free, or is held by
/// the entry itself (a case-only rename) or by one the undo is about to move away.
fn check_destination(
    providers: &Providers,
    from: &VfsPath,
    to: &Location,
    vacated: &[Location],
) -> Result<(), OpsError> {
    let (to_path, provider) = providers.for_location(to)?;
    let rule = provider.capabilities().case_rule;
    if let Some(folder) = to_path.parent() {
        if !exists(provider.as_ref(), &folder)? {
            return Err(stale(&folder.to_location(), StaleReason::Missing));
        }
    }
    let same_entry = same_path(from, &to_path, rule)
        || (from.parent() == to_path.parent()
            && matches!(
                (file_name_of(from), file_name_of(&to_path)),
                (Some(a), Some(b)) if same_name(&a, &b, rule)
            ));
    if same_entry || !exists(provider.as_ref(), &to_path)? {
        return Ok(());
    }
    let going = vacated.iter().any(|v| {
        VfsPath::from_location(v)
            .map(|p| same_path(&p, &to_path, rule))
            .unwrap_or(false)
    });
    if going {
        Ok(())
    } else {
        Err(stale(to, StaleReason::NameTaken))
    }
}

fn check_step(
    providers: &Providers,
    trash: &dyn Trash,
    protected: &Protected,
    step: &InverseStep,
    vacated: &[Location],
) -> Result<(), OpsError> {
    match step {
        InverseStep::RemoveCreated {
            location,
            fingerprint,
        } => {
            let (path, provider) = providers.for_location(location)?;
            if protected.contains(&path, provider.capabilities().case_rule) {
                return Err(OpsError::Protected {
                    location: location.clone(),
                });
            }
            verify(provider.as_ref(), &path, fingerprint.as_ref())
                .map_err(|reason| stale(location, reason))
        }
        InverseStep::Rename { from, to } | InverseStep::MoveBack { from, to } => {
            let (from_path, provider) = providers.for_location(from)?;
            if !exists(provider.as_ref(), &from_path)? {
                return Err(stale(from, StaleReason::Missing));
            }
            let (to_path, _) = providers.for_location(to)?;
            if to_path.scheme() != from_path.scheme() {
                return Err(OpsError::Unsupported {
                    what: "undoing across providers".to_owned(),
                });
            }
            check_destination(providers, &from_path, to, vacated)
        }
        InverseStep::RestoreTrashed { receipt } => {
            if !trash.contains(receipt)? {
                return Err(stale(&receipt.original, StaleReason::TrashEmptied));
            }
            let (original, provider) = providers.for_location(&receipt.original)?;
            if let Some(folder) = original.parent() {
                if !exists(provider.as_ref(), &folder)? {
                    return Err(stale(&folder.to_location(), StaleReason::Missing));
                }
            }
            if exists(provider.as_ref(), &original)? {
                return Err(stale(&receipt.original, StaleReason::NameTaken));
            }
            Ok(())
        }
        InverseStep::RemoveEmptyDir { location } => {
            let (path, provider) = providers.for_location(location)?;
            match provider.stat(&path) {
                Ok(entry) if entry.kind == EntryKind::Directory => {}
                Ok(_) => return Err(stale(location, StaleReason::Changed)),
                Err(VfsError::NotFound { .. }) => {
                    return Err(stale(location, StaleReason::Missing))
                }
                Err(error) => return Err(error.into()),
            }
            let children = provider.list(&path, &CancelToken::new(), 0, &mut |_| {})?;
            if children.is_empty() {
                Ok(())
            } else {
                Err(stale(location, StaleReason::Changed))
            }
        }
    }
}

/// Reads the world and refuses, with a typed error, if any step could not be applied as recorded.
/// Writes nothing. `steps` are in the order they will be applied.
pub fn check_steps(
    providers: &Providers,
    trash: &dyn Trash,
    protected: &Protected,
    steps: &[InverseStep],
) -> Result<(), OpsError> {
    let vacated: Vec<Location> = steps.iter().filter_map(vacates).cloned().collect();
    for step in steps {
        check_step(providers, trash, protected, step, &vacated)?;
    }
    Ok(())
}

/// Fills in the fingerprints of the entries a job made, once it has finished and before its entry
/// is committed. A fingerprint that cannot be taken stays `None`, which makes an undo of that step
/// refuse rather than delete something it cannot vouch for.
pub fn fingerprint_steps(providers: &Providers, steps: &mut [InverseStep]) {
    for step in steps {
        if let InverseStep::RemoveCreated {
            location,
            fingerprint: slot @ None,
        } = step
        {
            if let Ok((path, provider)) = providers.for_location(location) {
                *slot = fingerprint(provider.as_ref(), &path).ok();
            }
        }
    }
}

/// What an undo did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UndoReport {
    /// How many steps were applied: all of them.
    pub applied: usize,
    pub progress: Progress,
}

/// Why an undo stopped before the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoFailure {
    /// `Cancelled` for a cancel.
    pub error: OpsError,
    pub item: Option<Location>,
    /// Steps applied before it stopped, counted from the first.
    pub applied: usize,
    /// The steps still to do, in the order they would run.
    pub remaining: Vec<InverseStep>,
    pub progress: Progress,
}

/// The name an entry is moved aside under while a case-only rename goes through it.
fn aside_path(env: &ExecEnv, job: JobId, from: &VfsPath) -> Result<VfsPath, OpsError> {
    let folder = from.parent().ok_or_else(|| OpsError::Protected {
        location: from.to_location(),
    })?;
    let name = file_name_of(from).unwrap_or_default();
    let prefix = format!(".waypoint-partial-{}-{}-", job.0, env.ids.next());
    let mut text = name.to_string_lossy().into_owned();
    while prefix.len() + text.len() > 255 && text.pop().is_some() {}
    let text = text.trim_end_matches(['.', ' ']);
    folder
        .join(format!("{prefix}{text}"))
        .map_err(|_| OpsError::Io {
            message: "no usable name for a temporary entry".to_owned(),
        })
}

fn apply_step(env: &ExecEnv, job: JobId, step: &InverseStep) -> Result<(), OpsError> {
    match step {
        InverseStep::RemoveCreated { location, .. } => {
            let (path, provider) = env.providers.for_location(location)?;
            match provider.stat(&path)?.kind {
                EntryKind::Directory => {
                    // A tree goes in two steps so that its place is either full or empty, never
                    // half: it is renamed to a partial name (the step is done once that works),
                    // and the rest is cleanup that a failure or a crash leaves for recovery.
                    let aside = aside_path(env, job, &path)?;
                    provider.rename(&path, &aside, false)?;
                    let _ = remove_all(provider.as_ref(), &aside);
                }
                _ => provider.remove_file(&path)?,
            }
            Ok(())
        }
        InverseStep::Rename { from, to } | InverseStep::MoveBack { from, to } => {
            let (from_path, provider) = env.providers.for_location(from)?;
            let (to_path, _) = env.providers.for_location(to)?;
            let rule = provider.capabilities().case_rule;
            let case_only = from_path.parent() == to_path.parent()
                && from_path != to_path
                && matches!(
                    (file_name_of(&from_path), file_name_of(&to_path)),
                    (Some(a), Some(b)) if same_name(&a, &b, rule)
                );
            if case_only {
                let aside = aside_path(env, job, &from_path)?;
                provider.rename(&from_path, &aside, false)?;
                if let Err(error) = provider.rename(&aside, &to_path, false) {
                    let _ = provider.rename(&aside, &from_path, false);
                    return Err(error.into());
                }
            } else {
                provider.rename(&from_path, &to_path, false)?;
            }
            Ok(())
        }
        InverseStep::RestoreTrashed { receipt } => {
            env.trash.restore(receipt)?;
            Ok(())
        }
        InverseStep::RemoveEmptyDir { location } => {
            let (path, provider) = env.providers.for_location(location)?;
            provider.remove_dir(&path)?;
            Ok(())
        }
    }
}

impl Executor {
    /// Applies inverse steps, which are in the order to apply them (`Journal::undo_steps`), each
    /// checked again just before it runs. Stops at the first step that cannot be applied, at a
    /// cancel (noticed between steps), and reports how far it got.
    pub fn run_undo(
        &self,
        job: JobId,
        steps: &[InverseStep],
        cancel: &CancelToken,
        sink: &mut dyn ExecSink,
    ) -> Result<UndoReport, Box<UndoFailure>> {
        let env = self.env();
        let counts = Counts::default();
        let mut progress = Progress {
            items_total: steps.len() as u64,
            ..Progress::default()
        };
        let stop = |error: OpsError, at: usize, progress: &Progress| {
            Box::new(UndoFailure {
                item: steps.get(at).map(InverseStep::subject),
                error,
                applied: at,
                remaining: steps[at..].to_vec(),
                progress: progress.clone(),
            })
        };
        let vacated: Vec<Location> = steps.iter().filter_map(vacates).cloned().collect();
        for (at, step) in steps.iter().enumerate() {
            sink.between_items();
            if cancel.is_cancelled() {
                return Err(stop(OpsError::Cancelled, at, &progress));
            }
            let outcome = check_step(
                &env.providers,
                env.trash.as_ref(),
                &env.protected,
                step,
                &vacated,
            )
            .and_then(|()| apply_step(env, job, step));
            if let Err(error) = outcome {
                return Err(stop(error, at, &progress));
            }
            progress.items_done += 1;
            progress.current = file_name_of_location(&step.subject());
            sink.progress(&progress, &counts);
        }
        Ok(UndoReport {
            applied: steps.len(),
            progress,
        })
    }
}

fn file_name_of_location(location: &Location) -> Option<String> {
    VfsPath::from_location(location)
        .ok()
        .and_then(|p| file_name_of(&p))
        .map(|n| n.to_string_lossy().into_owned())
}

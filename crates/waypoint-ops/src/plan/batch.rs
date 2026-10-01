// Planning a batch rename: reads the entries' names, kinds and times and their folders' names,
// applies the rules, and either refuses with a typed error before anything is written or settles
// the order of renames that gets every entry to its new name (A47). The preview command asks the
// same question without the plan.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::ffi::OsStr;
use waypoint_path::{CaseRule, VfsPath};
use waypoint_vfs::{child_path, EntryKind, ScannedEntry};

use super::{check, failure, Plan, PlanCtx, PlanItem, Planner};
use crate::model::{JobKind, JobRequest, OpsError};
use crate::names::{file_name_of, same_name, same_path};
use crate::rename_clash::{
    plan_batch, preview_batch_rename, BatchEntry, BatchPreview, FolderInputs, Place,
    PreviewRequest, PreviewRow, Problem,
};
use crate::rename_rules::{RenameInput, RenameSpec};
use crate::traits::{Clock, SystemClock};

/// One step of a batch, aimed at a plan item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchStep {
    /// The entry, by its position in `Plan::items`.
    pub item: usize,
    pub from: Place,
    pub to: Place,
}

/// What a batch rename plan adds to the items: the order to rename in and what a redo needs to
/// reproduce the names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPlan {
    /// Every entry the rules ran over, in selection order, whether or not its name changes. A redo
    /// runs the rules over these, so counters count the same entries.
    pub sources: Vec<VfsPath>,
    pub steps: Vec<BatchStep>,
    /// The time "today" meant.
    pub now_ms: i64,
}

struct Group {
    folder: VfsPath,
    rule: CaseRule,
    siblings: Vec<String>,
    /// The selection indices of the entries in the folder.
    members: Vec<usize>,
}

/// What the rules are asked over: the entries as they are now.
struct Gathered {
    sources: Vec<VfsPath>,
    entries: Vec<ScannedEntry>,
    inputs: Vec<RenameInput>,
    groups: Vec<Group>,
    spec: RenameSpec,
    now_ms: i64,
}

impl Gathered {
    fn request(&self) -> PreviewRequest {
        PreviewRequest {
            folders: self
                .groups
                .iter()
                .map(|g| FolderInputs {
                    case_rule: g.rule,
                    siblings: g.siblings.clone(),
                    inputs: g.members.iter().map(|m| self.inputs[*m].clone()).collect(),
                })
                .collect(),
            rules: self.spec.rules.clone(),
            now_ms: self.now_ms,
            utc_offset_minutes: self.spec.utc_offset_minutes,
        }
    }
}

impl Planner<'_, '_> {
    fn gather(&mut self) -> Result<Gathered, OpsError> {
        let spec = self
            .request
            .rename
            .clone()
            .ok_or_else(|| failure("a batch rename needs its rules"))?;
        let resolved = self.sources()?;
        if resolved.is_empty() {
            return Err(failure("a batch rename needs something to rename"));
        }
        let now_ms = spec.now_ms.unwrap_or_else(|| SystemClock.now_ms());
        let mut gathered = Gathered {
            sources: Vec::new(),
            entries: Vec::new(),
            inputs: Vec::new(),
            groups: Vec::new(),
            spec,
            now_ms,
        };
        for (index, (path, provider)) in resolved.into_iter().enumerate() {
            check(self.ctx.cancel)?;
            let rule = provider.capabilities().case_rule;
            let folder = path.parent().ok_or_else(|| OpsError::Protected {
                location: path.to_location(),
            })?;
            let entry = provider.stat(&path)?;
            let at = match gathered
                .groups
                .iter()
                .position(|g| same_path(&g.folder, &folder, rule))
            {
                Some(at) => at,
                None => {
                    let siblings = provider
                        .list(&folder, self.ctx.cancel, 0, &mut |_| {})?
                        .into_iter()
                        .map(|e| e.name.to_string_lossy().into_owned())
                        .collect();
                    gathered.groups.push(Group {
                        folder,
                        rule,
                        siblings,
                        members: Vec::new(),
                    });
                    gathered.groups.len() - 1
                }
            };
            gathered.groups[at].members.push(index);
            gathered.inputs.push(RenameInput {
                name: file_name_of(&path)
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                is_dir: entry.kind == EntryKind::Directory,
                modified_ms: entry.modified_ms,
                created_ms: None,
                index,
            });
            gathered.entries.push(entry);
            gathered.sources.push(path);
        }
        Ok(gathered)
    }

    pub(super) fn batch_rename(&mut self) -> Result<Plan, OpsError> {
        let gathered = self.gather()?;
        let preview = preview_batch_rename(&gathered.request());
        refuse_if_unfit(&gathered, &preview)?;
        if preview.changes == 0 {
            return Err(OpsError::InvalidName {
                name: String::new(),
                reason: "the rules leave every name as it is".to_owned(),
            });
        }

        // The plan's items are the entries whose names change, in selection order.
        let mut item_of = vec![None; gathered.sources.len()];
        let mut items = Vec::new();
        for row in preview.rows.iter().filter(|r| r.changed) {
            let at = row.index;
            let group = gathered
                .groups
                .iter()
                .find(|g| g.members.contains(&at))
                .expect("every entry is in a folder");
            let target = child_path(&group.folder, OsStr::new(&row.to), group.rule)?;
            let entry = &gathered.entries[at];
            item_of[at] = Some(items.len());
            items.push(PlanItem {
                source: Some(gathered.sources[at].clone()),
                target: Some(target),
                kind: entry.kind,
                size: entry.size,
                entries: 1,
                bytes: 0,
                case_only: same_name(OsStr::new(&row.from), OsStr::new(&row.to), group.rule),
            });
        }

        let mut steps = Vec::new();
        for group in &gathered.groups {
            let entries: Vec<BatchEntry> = group
                .members
                .iter()
                .map(|m| BatchEntry {
                    from: preview.rows[*m].from.clone(),
                    to: preview.rows[*m].to.clone(),
                })
                .collect();
            let planned = plan_batch(&entries, &group.siblings, group.rule)
                .map_err(|clash| clash_error(&gathered, group, &clash.problems))?;
            for step in planned {
                let item = item_of[group.members[step.entry]].expect("a moving entry is an item");
                steps.push(BatchStep {
                    item,
                    from: step.from,
                    to: step.to,
                });
            }
        }
        let mut plan = self.finish(items, None, true, Vec::new());
        plan.batch = Some(BatchPlan {
            sources: gathered.sources,
            steps,
            now_ms: gathered.now_ms,
        });
        debug_assert_eq!(plan.kind, JobKind::BatchRename);
        Ok(plan)
    }
}

/// The typed refusal for the first thing wrong with the preview, if anything is.
fn refuse_if_unfit(gathered: &Gathered, preview: &BatchPreview) -> Result<(), OpsError> {
    if let Some(error) = preview.rule_errors.first() {
        return Err(OpsError::InvalidName {
            name: format!("rule {}", error.rule + 1),
            reason: error.reason.clone(),
        });
    }
    for row in &preview.rows {
        if let Some(problem) = row.problems.iter().find(|p| p.blocks()) {
            return Err(problem_error(gathered, row, problem));
        }
    }
    Ok(())
}

fn problem_error(gathered: &Gathered, row: &PreviewRow, problem: &Problem) -> OpsError {
    match problem {
        Problem::Invalid { reason } => OpsError::InvalidName {
            name: row.to.clone(),
            reason: reason.clone(),
        },
        _ => {
            let folder = gathered.sources[row.index].parent();
            let location = folder
                .and_then(|f| f.join(&row.to).ok())
                .unwrap_or_else(|| gathered.sources[row.index].clone());
            OpsError::NameInUse {
                location: location.to_location(),
            }
        }
    }
}

fn clash_error(gathered: &Gathered, group: &Group, problems: &[(usize, Problem)]) -> OpsError {
    match problems.first() {
        Some((at, problem)) => {
            let index = group.members[*at];
            let row = PreviewRow {
                index,
                from: gathered.inputs[index].name.clone(),
                to: String::new(),
                changed: true,
                extension_changed: false,
                problems: Vec::new(),
            };
            problem_error(gathered, &row, problem)
        }
        None => failure("the names clash"),
    }
}

/// What a batch rename of the request's sources would do, without planning it: a row for each
/// entry and the clashes among the results. This is the preview command's core.
pub fn preview_batch(request: &JobRequest, ctx: &PlanCtx<'_>) -> Result<BatchPreview, OpsError> {
    let mut planner = Planner {
        request,
        ctx,
        progress: &mut |_| {},
        walked: super::PlanProgress::default(),
        warnings: Vec::new(),
    };
    let gathered = planner.gather()?;
    Ok(preview_batch_rename(&gathered.request()))
}

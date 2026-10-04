// Implements the IPC commands exposed by the operations plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every command goes through `Ops`, which does the work under the one lock. A command that makes a
// request is told the calling window's label, which a request carries as `origin_window` whatever
// the page sent: a selection handle belongs to the window that opened its listing.

use tauri::ipc::Channel;
use tauri::{Runtime, State, WebviewWindow};
use waypoint_ops::{
    BatchPreview, ConflictPolicy, ConflictPreview, Decision, JobId, JobRequest,
    JournalEntrySummary, JournalId, OpsSettings, OpsSnapshot, RecoveryReport, Resolution,
};
use waypoint_protocol::{Location, PluginStatus};
use waypoint_vfs::{ListingHandle, SelectionSpec};

use crate::models::{Clipboard, ClipboardMode, ClipboardSource, Error, JobProgress, PlanPreview};
use crate::ops::Ops;

/// Reports whether the queue works, and which parts of it do on this system.
#[tauri::command]
pub async fn get_status<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<PluginStatus, Error> {
    let mut features = vec![
        "queue".to_owned(),
        "undo".to_owned(),
        "clipboard".to_owned(),
        "verify".to_owned(),
    ];
    if ops.shared.env.trash.available().is_ok() {
        features.push("trash".to_owned());
    }
    Ok(PluginStatus::available(features))
}

/// The queue and the undo history at their current revisions.
#[tauri::command]
pub async fn get_snapshot<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<OpsSnapshot, Error> {
    Ok(ops.snapshot())
}

/// What a request would do, without queueing it: counts, clashes, and whether it stays on one
/// volume (which a drag uses to choose between a move and a copy).
#[tauri::command]
pub async fn plan<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    request: JobRequest,
) -> Result<PlanPreview, Error> {
    let ops = ops.inner().clone();
    let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || ops.plan(&label, request))
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
}

/// What a batch rename would do, without queueing it: `request` is the `BatchRename` request that
/// `submit` takes (its sources and `rename` rules), and the answer has a row for each entry with
/// its new name and what is wrong with it, if anything.
#[tauri::command]
pub async fn preview_batch_rename<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    request: JobRequest,
) -> Result<BatchPreview, Error> {
    let ops = ops.inner().clone();
    let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || ops.preview_batch_rename(&label, request))
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
}

/// Puts a request on the queue and returns its id.
#[tauri::command]
pub async fn submit<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    request: JobRequest,
) -> Result<JobId, Error> {
    // A selection is resolved as the job is accepted, which reads the listing: not on the runtime.
    let ops = ops.inner().clone();
    let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || ops.submit(&label, request))
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn pause<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
) -> Result<(), Error> {
    ops.pause(job)
}

#[tauri::command]
pub async fn resume<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
) -> Result<(), Error> {
    ops.resume(job)
}

#[tauri::command]
pub async fn cancel<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
) -> Result<(), Error> {
    ops.cancel(job)
}

/// Makes a new job from a failed or cancelled one's request and returns its id.
#[tauri::command]
pub async fn retry<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
) -> Result<JobId, Error> {
    ops.retry(job)
}

/// Removes a finished job from the list.
#[tauri::command]
pub async fn dismiss<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
) -> Result<(), Error> {
    ops.dismiss(job)
}

/// Removes every finished job from the list.
#[tauri::command]
pub async fn dismiss_finished<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<(), Error> {
    ops.dismiss_finished();
    Ok(())
}

/// Moves a queued job to `to` among the queued jobs (0 runs next).
#[tauri::command]
pub async fn reorder<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
    to: usize,
) -> Result<(), Error> {
    ops.reorder(job, to)
}

/// Changes a job's own speed limit (bytes a second; `None` for none) and priority (`None` is
/// normal), on a queued or running job.
#[tauri::command]
pub async fn set_job_limits<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
    speed_limit: Option<u64>,
    priority: Option<waypoint_ops::JobPriority>,
) -> Result<(), Error> {
    ops.set_job_limits(job, speed_limit, priority)
}

/// Sets or clears when a job that has not started may start (`None` is "Run now").
#[tauri::command]
pub async fn set_job_schedule<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
    schedule: Option<waypoint_ops::Schedule>,
) -> Result<(), Error> {
    ops.set_job_schedule(job, schedule)
}

/// Pauses every running job and holds back everything queued.
#[tauri::command]
pub async fn pause_all<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<(), Error> {
    ops.pause_all();
    Ok(())
}

/// Lets queued jobs start again and runs every paused job on.
#[tauri::command]
pub async fn resume_all<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<(), Error> {
    ops.resume_all();
    Ok(())
}

/// Answers the conflicts a job waits on: `decisions` settle one source's clash each, and
/// `apply_to_all` is the policy for every other clash the job meets.
#[tauri::command]
pub async fn resolve<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
    decisions: Vec<Resolution>,
    apply_to_all: Option<ConflictPolicy>,
) -> Result<(), Error> {
    ops.resolve(job, decisions, apply_to_all)
}

/// The two files of one clash a job waits on, side by side: their size and time and, for small
/// text files, a line diff. `item` is the clash's source. It reads both files, so it runs off the
/// runtime, and the job's cancel stops it. Anything that cannot be compared is `unavailable`.
#[tauri::command]
pub async fn conflict_preview<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
    item: Location,
) -> Result<ConflictPreview, Error> {
    let ops = ops.inner().clone();
    tauri::async_runtime::spawn_blocking(move || ops.conflict_preview(job, &item))
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
}

/// Answers the error a job waits on: retry, skip, skip every error of this kind, or cancel.
#[tauri::command]
pub async fn resolve_error<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
    decision: Decision,
) -> Result<(), Error> {
    ops.resolve_error(job, decision)
}

/// Undoes an entry of the journal, or the newest applied one, as a job. Returns the job's id.
#[tauri::command]
pub async fn undo<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    entry: Option<JournalId>,
) -> Result<JobId, Error> {
    ops.undo(window.label(), entry)
}

/// Redoes an entry of the journal, or the one undone most recently, as a job.
#[tauri::command]
pub async fn redo<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    entry: Option<JournalId>,
) -> Result<JobId, Error> {
    ops.redo(window.label(), entry)
}

/// The undo history, newest first, for the menu and the palette.
#[tauri::command]
pub async fn journal_summaries<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<Vec<JournalEntrySummary>, Error> {
    Ok(ops.journal_summaries())
}

/// The journal entry the job made, or `None` while it has none (it has not finished, it changed
/// nothing, or it is not in the queue any more).
#[tauri::command]
pub async fn journal_entry_of<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    job: JobId,
) -> Result<Option<JournalId>, Error> {
    Ok(ops.journal_entry_of(job))
}

/// Sends the calling window the progress of the running jobs on `on_progress`, at the rate the
/// queue's gate allows. A second call from the same window replaces the first. Returns the token of
/// the subscription, which `unsubscribe_progress` takes so that a stop that arrives late cannot end
/// a newer subscription.
#[tauri::command]
pub async fn subscribe_progress<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    on_progress: Channel<JobProgress>,
) -> Result<u64, Error> {
    Ok(ops.subscribe_progress(window.label(), on_progress))
}

/// Stops the window's progress. With the `token` a subscribe returned, only that subscription is
/// stopped: one that has been replaced since is left alone. Without it, whatever the window has.
#[tauri::command]
pub async fn unsubscribe_progress<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    token: Option<u64>,
) -> Result<(), Error> {
    ops.unsubscribe_progress(window.label(), token);
    Ok(())
}

/// Replaces the shared clipboard (an empty list clears it) and tells every window.
#[tauri::command]
pub async fn set_clipboard<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    mode: ClipboardMode,
    items: Vec<Location>,
    source: Option<ClipboardSource>,
) -> Result<Clipboard, Error> {
    Ok(ops.set_clipboard_from(mode, items, source.unwrap_or_default()))
}

/// Puts the entries a selection covers on the shared clipboard. Rust resolves them from the
/// listing the calling window opened (A47), so the page sends a handle and a selection and never a
/// path.
#[tauri::command]
pub async fn set_clipboard_from_selection<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    handle: ListingHandle,
    spec: SelectionSpec,
    mode: ClipboardMode,
) -> Result<Clipboard, Error> {
    let ops = ops.inner().clone();
    let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        ops.set_clipboard_from_selection(&label, handle, &spec, mode)
    })
    .await
    .map_err(|e| Error::Internal(e.to_string()))?
}

/// The locations a selection covers, resolved from the listing the calling window opened (A47),
/// for a drag that leaves the window. The page sends a handle and a selection and gets back the
/// lossless `uri` of each item.
#[tauri::command]
pub async fn resolve_selection<R: Runtime>(
    window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    handle: ListingHandle,
    spec: SelectionSpec,
) -> Result<Vec<Location>, Error> {
    let ops = ops.inner().clone();
    let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || ops.resolve_selection(&label, handle, &spec))
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn get_clipboard<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<Clipboard, Error> {
    Ok(ops.clipboard())
}

/// The unfinished jobs that touch `location`, which the close guard warns about.
#[tauri::command]
pub async fn jobs_targeting<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    location: Location,
) -> Result<Vec<JobId>, Error> {
    Ok(ops.jobs_targeting(&location))
}

#[tauri::command]
pub async fn get_settings<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<OpsSettings, Error> {
    Ok(ops.settings())
}

/// Saves and applies new settings, which the next job reads. Returns what is now in force.
#[tauri::command]
pub async fn set_settings<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
    settings: OpsSettings,
) -> Result<OpsSettings, Error> {
    ops.set_settings(settings)
}

/// What start-up recovery found, once: the jobs the last run left interrupted and what was cleaned
/// up. `null` when there was nothing to tell, and after the first call.
#[tauri::command]
pub async fn take_recovery_report<R: Runtime>(
    _window: WebviewWindow<R>,
    ops: State<'_, Ops<R>>,
) -> Result<Option<RecoveryReport>, Error> {
    Ok(ops.take_recovery_report())
}

if ('__TAURI__' in window) {
var __TAURI_PLUGIN_WAYPOINT_OPS__ = (function (exports, core, event) {
    'use strict';

    // Exposes typed guest-side wrappers for the waypoint-ops plugin
    //
    // (c) Copyright 2026 Liminal HQ, Scott Morris
    // SPDX-License-Identifier: Apache-2.0 OR MIT
    const PREFIX = 'plugin:waypoint-ops|';
    /** Every change to the queue or the undo history, broadcast to every window. */
    const OPS_EVENT = 'waypoint-ops://event';
    /** The shared clipboard changed, broadcast to every window. */
    const CLIPBOARD_EVENT = 'waypoint-ops://clipboard';
    /** A job recorded its journal entry (a `JobJournal`), broadcast to every window. */
    const JOB_JOURNAL_EVENT = 'waypoint-ops://job-journal';
    /** What the last run left interrupted, sent once at start-up. */
    const RECOVERED_EVENT = 'waypoint-ops://recovered';
    function cmd(name, args) {
        return core.invoke(`${PREFIX}${name}`, args);
    }
    /** Reports whether the operations plugin works and which features it offers. */
    function getStatus() {
        return cmd('get_status');
    }
    /** The queue and the undo history at their current revisions. */
    function getSnapshot() {
        return cmd('get_snapshot');
    }
    /**
     * What a request would do, found without queueing it: the counts, the clashes and whether every
     * source is on the destination's volume (a drag moves on one volume and copies across).
     */
    function plan(request) {
        return cmd('plan', { request });
    }
    /**
     * What a batch rename would do, found without queueing it: `request` is the `batchRename` request
     * that `submit` takes (its sources and its `rename` rules). The answer has a row for each entry
     * with its new name and what is wrong with it, and `nowMs`, the time "today" meant, which goes back
     * in the submitted request's `rename.nowMs` so the job writes the names that were previewed.
     */
    function previewBatchRename(request) {
        return cmd('preview_batch_rename', { request });
    }
    /** Puts a request on the queue and returns the job's id. */
    function submit(request) {
        return cmd('submit', { request });
    }
    function pause(job) {
        return cmd('pause', { job });
    }
    function resume(job) {
        return cmd('resume', { job });
    }
    function cancel(job) {
        return cmd('cancel', { job });
    }
    /** Makes a new job from a failed or cancelled one's request and returns its id. */
    function retry(job) {
        return cmd('retry', { job });
    }
    /** Removes a finished job from the list. */
    function dismiss(job) {
        return cmd('dismiss', { job });
    }
    function dismissFinished() {
        return cmd('dismiss_finished');
    }
    /** Moves a queued job to `to` among the queued jobs (0 runs next). */
    function reorder(job, to) {
        return cmd('reorder', { job, to });
    }
    /**
     * Answers the conflicts a job waits on: each decision settles one source's clash, and
     * `applyToAll` is the policy for every other clash the job meets.
     */
    function resolve(job, decisions, applyToAll) {
        return cmd('resolve', { job, decisions, applyToAll: applyToAll ?? null });
    }
    /** Answers the error a job waits on. */
    function resolveError(job, decision) {
        return cmd('resolve_error', { job, decision });
    }
    /** Undoes an entry of the journal (the newest applied one when omitted) and returns the job's id. */
    function undo(entry) {
        return cmd('undo', { entry: entry ?? null });
    }
    /** Redoes an entry of the journal (the one undone most recently when omitted). */
    function redo(entry) {
        return cmd('redo', { entry: entry ?? null });
    }
    /** The undo history, newest first. */
    function journalSummaries() {
        return cmd('journal_summaries');
    }
    /** The journal entry the job made, or `null` while it has none (unfinished, or it changed nothing). */
    function journalEntryOf(job) {
        return cmd('journal_entry_of', { job });
    }
    /**
     * Hears the progress of the running jobs on this window, at the rate the queue's gate allows
     * (every 100 ms and 1 %). Progress is not an event: only a window that subscribes receives it.
     * Returns the function that stops listening: it stops this subscription only, so a stop that runs
     * after the window has subscribed again (a page that mounted twice) leaves the newer one alone.
     */
    async function subscribeProgress(onProgress) {
        const channel = new core.Channel();
        channel.onmessage = onProgress;
        const token = await cmd('subscribe_progress', { onProgress: channel });
        return () => cmd('unsubscribe_progress', { token });
    }
    /**
     * Replaces the shared clipboard; an empty list clears it. Every window is told. `source` says who
     * set it (`app` when omitted; `os` for files adopted from another application's clipboard).
     */
    function setClipboard(mode, items, source) {
        return cmd('set_clipboard', { mode, items, source: source ?? null });
    }
    /**
     * Puts the entries a selection covers on the shared clipboard. Rust resolves them from the listing
     * this window opened, so a selection of a hundred thousand files is still a handle and a spec.
     * Rejects (`unsupported`) for a selection of nothing.
     */
    function setClipboardFromSelection(handle, spec, mode) {
        return cmd('set_clipboard_from_selection', { handle, spec, mode });
    }
    /**
     * The locations a selection covers, resolved by Rust from the listing this window opened, for a
     * drag that leaves the window. Each location's `uri` is the lossless `file://` form. Rejects
     * (`unsupported`) for a selection of nothing.
     */
    function resolveSelection(handle, spec) {
        return cmd('resolve_selection', { handle, spec });
    }
    function getClipboard() {
        return cmd('get_clipboard');
    }
    /** The unfinished jobs that touch `location`, which the close guard warns about. */
    function jobsTargeting(location) {
        return cmd('jobs_targeting', { location });
    }
    function getSettings() {
        return cmd('get_settings');
    }
    /** Saves new settings, which the next job reads, and returns what is in force. */
    function setSettings(settings) {
        return cmd('set_settings', { settings });
    }
    /**
     * What start-up recovery found, once: the jobs the last run left interrupted and what was cleaned
     * up. `null` when there was nothing to tell, and after the first call.
     */
    function takeRecoveryReport() {
        return cmd('take_recovery_report');
    }
    /**
     * Follows every change to the queue and the undo history. Read `getSnapshot()` first and apply the
     * events with a higher revision (a `journalChanged` is judged against the journal's own revision);
     * the revisions have gaps, since progress ticks use them up.
     */
    function onOpsEvent(listener) {
        return event.listen(OPS_EVENT, (e) => listener(e.payload));
    }
    /** Follows the shared clipboard. */
    function onClipboardChanged(listener) {
        return event.listen(CLIPBOARD_EVENT, (e) => listener(e.payload));
    }
    /** Hears the recovery report if a window is listening when it is sent at start-up. */
    function onRecovered(listener) {
        return event.listen(RECOVERED_EVENT, (e) => listener(e.payload));
    }

    exports.CLIPBOARD_EVENT = CLIPBOARD_EVENT;
    exports.JOB_JOURNAL_EVENT = JOB_JOURNAL_EVENT;
    exports.OPS_EVENT = OPS_EVENT;
    exports.RECOVERED_EVENT = RECOVERED_EVENT;
    exports.cancel = cancel;
    exports.dismiss = dismiss;
    exports.dismissFinished = dismissFinished;
    exports.getClipboard = getClipboard;
    exports.getSettings = getSettings;
    exports.getSnapshot = getSnapshot;
    exports.getStatus = getStatus;
    exports.jobsTargeting = jobsTargeting;
    exports.journalEntryOf = journalEntryOf;
    exports.journalSummaries = journalSummaries;
    exports.onClipboardChanged = onClipboardChanged;
    exports.onOpsEvent = onOpsEvent;
    exports.onRecovered = onRecovered;
    exports.pause = pause;
    exports.plan = plan;
    exports.previewBatchRename = previewBatchRename;
    exports.redo = redo;
    exports.reorder = reorder;
    exports.resolve = resolve;
    exports.resolveError = resolveError;
    exports.resolveSelection = resolveSelection;
    exports.resume = resume;
    exports.retry = retry;
    exports.setClipboard = setClipboard;
    exports.setClipboardFromSelection = setClipboardFromSelection;
    exports.setSettings = setSettings;
    exports.submit = submit;
    exports.subscribeProgress = subscribeProgress;
    exports.takeRecoveryReport = takeRecoveryReport;
    exports.undo = undo;

    return exports;

})({}, __TAURI__.core, __TAURI__.event);
Object.defineProperty(window.__TAURI__, 'waypointOps', { value: __TAURI_PLUGIN_WAYPOINT_OPS__ }) }

// The real OpsClient: the waypoint-ops plugin through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as ops from '@liminal-hq/waypoint-plugin-ops';
import type { OpsClient } from './opsClient';
import type { Unsubscribe } from './vfsClient';

/** Turns a listener registration that resolves later into an unsubscribe that works at once. */
function subscribe(registration: Promise<() => void>): Unsubscribe {
	let unlisten: (() => void) | undefined;
	let stopped = false;
	void registration.then(
		(fn) => {
			if (stopped) fn();
			else unlisten = fn;
		},
		(error: unknown) => console.warn('could not listen for an operations event', error),
	);
	return () => {
		stopped = true;
		unlisten?.();
		unlisten = undefined;
	};
}

/** An `OpsClient` over the plugin. Create one per window: progress is sent to the window that subscribed. */
export function createTauriOpsClient(): OpsClient {
	return {
		snapshot: () => ops.getSnapshot(),
		plan: (request) => ops.plan(request),
		submit: (request) => ops.submit(request),
		pause: (job) => ops.pause(job),
		resume: (job) => ops.resume(job),
		cancel: (job) => ops.cancel(job),
		retry: (job) => ops.retry(job),
		dismiss: (job) => ops.dismiss(job),
		dismissFinished: () => ops.dismissFinished(),
		reorder: (job, to) => ops.reorder(job, to),
		resolve: (job, decisions, applyToAll) => ops.resolve(job, decisions, applyToAll),
		conflictPreview: (job, item) => ops.conflictPreview(job, item),
		resolveError: (job, decision) => ops.resolveError(job, decision),
		undo: (entry) => ops.undo(entry),
		redo: (entry) => ops.redo(entry),
		journalSummaries: () => ops.journalSummaries(),
		journalEntryOf: (job) => ops.journalEntryOf(job),
		jobsTargeting: (location) => ops.jobsTargeting(location),
		subscribeProgress: (listener) => ops.subscribeProgress(listener),
		getClipboard: () => ops.getClipboard(),
		setClipboard: (mode, items, source) => ops.setClipboard(mode, items, source),
		setClipboardFromSelection: (handle, spec, mode) =>
			ops.setClipboardFromSelection(handle, spec, mode),
		resolveSelection: (handle, spec) => ops.resolveSelection(handle, spec),
		getSettings: () => ops.getSettings(),
		setSettings: (settings) => ops.setSettings(settings),
		takeRecoveryReport: () => ops.takeRecoveryReport(),
		onEvent: (listener) => subscribe(ops.onOpsEvent(listener)),
		onClipboard: (listener) => subscribe(ops.onClipboardChanged(listener)),
		onRecovered: (listener) => subscribe(ops.onRecovered(listener)),
	};
}

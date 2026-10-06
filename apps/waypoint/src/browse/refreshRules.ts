// When a folder that nothing watches is read again: a window focused, and a job that wrote (D150, A83)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { isFinished } from '../ops/opsSelectors';
import type { OpsHandle } from '../ops/opsStore';

/**
 * Calls `onWrote` once for each job that finishes after this starts, whichever window began it and
 * however it ended (a job that failed or was cancelled part way may have written some): a server's folder cannot
 * report what changed, so the listings of such folders are read again. Jobs already finished when
 * this starts are not counted. Returns what stops it.
 */
export function startRefreshAfterJobs(handle: OpsHandle, onWrote: () => void): () => void {
	const finished = new Set<number>();
	let seeded = false;
	const process = () => {
		const { snapshot } = handle.store.getState();
		if (!snapshot) return;
		let wrote = false;
		for (const job of snapshot.jobs) {
			if (!isFinished(job) || finished.has(job.id)) continue;
			finished.add(job.id);
			if (seeded) wrote = true;
		}
		seeded = true;
		if (wrote) onWrote();
	};
	const stop = handle.store.subscribe(process);
	process();
	return stop;
}

/**
 * Ctrl+R reads the folder again. (F5 copies to the other pane, which came first, so the refresh
 * has a key of its own.) Returns what stops it.
 */
export function startRefreshShortcut(target: Window, onRefresh: () => void): () => void {
	const onKeyDown = (event: KeyboardEvent) => {
		if (event.defaultPrevented || event.isComposing) return;
		if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey) return;
		if (event.key.toLowerCase() !== 'r') return;
		event.preventDefault();
		onRefresh();
	};
	target.addEventListener('keydown', onKeyDown);
	return () => target.removeEventListener('keydown', onKeyDown);
}

/**
 * Calls `onFocus` when the window is focused or shown again, for the rule that reads a stale
 * folder then. Returns what stops it.
 */
export function startRefreshOnFocus(target: Window, onFocus: () => void): () => void {
	const onVisibility = () => {
		if (target.document.visibilityState === 'visible') onFocus();
	};
	target.addEventListener('focus', onFocus);
	target.document.addEventListener('visibilitychange', onVisibility);
	return () => {
		target.removeEventListener('focus', onFocus);
		target.document.removeEventListener('visibilitychange', onVisibility);
	};
}

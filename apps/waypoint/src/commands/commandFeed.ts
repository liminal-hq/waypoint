// Keeps the command facts that come from the panes, the queue, the clipboard and the view current
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JournalEntrySummary } from '@liminal-hq/waypoint-protocol/generated/JournalEntrySummary';
import { batchRenameSelection } from '../ops/batchRename/batchRenameSelection';
import { selectedCount } from '../browse/selection';
import { isFolder } from '../nav/useOpenEntry';
import type { ListingSession } from '../browse/useListingSession';
import type { ViewStore } from '../browse/viewStore';
import type { ClipboardService } from '../ops/clipboardService';
import type { FileCommands } from '../ops/fileCommands';
import type { OpsHandle } from '../ops/opsStore';
import type { SidebarStore } from '../sidebar/sidebarStore';
import type { CommandBridge } from './commandBridge';
import { emptyFacts } from './commandEnv';

export interface CommandFeedSources {
	/** The window's file commands; `null` without a queue. */
	files: FileCommands | null;
	/** The pane the commands act on, read at each refresh. */
	activeSession: () => ListingSession | null;
	ops: OpsHandle | null;
	clipboard: ClipboardService | null;
	view: ViewStore;
	sidebar: SidebarStore;
	/** Hears a pane's listing opening or closing (the listing manager's `subscribe`). */
	subscribePanes: (listener: () => void) => () => void;
}

/** What Open as Administrator would act on: nothing selected, one folder, or something it cannot (a file, or several items). */
function elevateSelectionOf(
	session: ListingSession | null,
	selected: number,
): 'none' | 'folder' | 'other' {
	if (!session || selected === 0) return 'none';
	if (selected > 1) return 'other';
	const { selection } = session.store.getState();
	const id = selection.kind === 'some' ? [...selection.ids][0] : undefined;
	const entry = id === undefined ? undefined : session.model.cachedEntry(id);
	return entry && isFolder(entry) ? 'folder' : 'other';
}

export interface CommandFeed {
	/** Recomputes the facts now: call it when something the feed cannot subscribe to has changed (the active tab). */
	refresh(): void;
	stop(): void;
}

/**
 * Subscribes to every source the file and view facts depend on and keeps `bridge`'s facts current.
 * The work is a handful of reads, and the bridge drops an unchanged result, so a subscriber
 * re-renders only when a fact actually differs. Facts that come from React (the tabs, the
 * settings, the window) are patched in by the components that own them.
 */
export function startCommandFeed(bridge: CommandBridge, sources: CommandFeedSources): CommandFeed {
	let stopped = false;
	let attached: ListingSession | null = null;
	let detach: Array<() => void> = [];
	let journalRevision: number | null = null;
	let historyRequest = 0;

	const attach = (session: ListingSession | null) => {
		if (session === attached) return;
		for (const stop of detach) stop();
		detach = [];
		attached = session;
		if (!session) return;
		// Selection and focus live in the pane's store; its listing's count and patches change what "all" means.
		detach.push(session.store.subscribe(refresh), session.model.subscribe(refresh));
	};

	const loadHistory = () => {
		const request = ++historyRequest;
		const files = sources.files;
		if (!files) return;
		files.historyEntries().then(
			(entries: JournalEntrySummary[]) => {
				if (!stopped && request === historyRequest) bridge.patchFacts({ history: entries });
			},
			(error: unknown) => console.warn('could not read the undo history', error),
		);
	};

	function refresh() {
		if (stopped) return;
		const session = sources.activeSession();
		attach(session);
		const { files, ops } = sources;
		const journal = ops?.store.getState().snapshot?.journal;
		if (journal && journal.revision !== journalRevision) {
			journalRevision = journal.revision;
			loadHistory();
		}
		const queue = ops?.store.getState().snapshot;
		const view = sources.view.getState();
		const selection = session?.store.getState().selection;
		const chosen = session && selection ? selectedCount(selection, session.model.count) : 0;
		bridge.patchFacts({
			elevateSelection: elevateSelectionOf(session, chosen),
			file: files ? files.states(session) : emptyFacts().file,
			selected: session && selection ? selectedCount(selection, session.model.count) : 0,
			listing: session !== null,
			local: session?.model.location.uri.startsWith('file:') ?? false,
			trash: session?.model.layout === 'trash',
			batchRename: batchRenameSelection(session) !== null,
			sort: session?.model.sort ?? null,
			undoLabel: journal?.undo?.label ?? null,
			redoLabel: journal?.redo?.label ?? null,
			opsRunning: queue?.jobs.filter((job) => job.state.state === 'running').length ?? 0,
			opsPaused:
				(queue?.paused ?? false) ||
				(queue?.jobs.some((job) => job.state.state === 'paused') ?? false),
			undoHead: journal?.undo?.id ?? null,
			redoHead: journal?.redo?.id ?? null,
			viewMode: view.mode,
			showHidden: view.showHidden,
			sidebarOpen: sources.sidebar.getState().open,
		});
	}

	const stops = [
		sources.ops?.store.subscribe(refresh),
		sources.clipboard?.store.subscribe(refresh),
		sources.view.subscribe(refresh),
		sources.sidebar.subscribe(refresh),
		sources.subscribePanes(refresh),
	];
	refresh();

	return {
		refresh,
		stop() {
			stopped = true;
			for (const stop of stops) stop?.();
			for (const stop of detach) stop();
			detach = [];
		},
	};
}

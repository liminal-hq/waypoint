// The file commands (New Folder, New File, Rename, Duplicate, Trash, Delete, Undo, Redo): the requests they make and a thin dispatcher
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SelectionSpec } from '@liminal-hq/waypoint-protocol/generated/SelectionSpec';
import type { Sources } from '@liminal-hq/waypoint-protocol/generated/Sources';
import { formatSize } from '../browse/format';
import {
	firstSelectedNames,
	InsertTracker,
	revealByName,
	revealInserted,
	waitForInserts,
} from '../browse/reveal';
import { isSelected, selectedCount, type Selection } from '../browse/selection';
import { PAGE_SIZE } from '../browse/listingModel';
import type { ListingSession } from '../browse/useListingSession';
import { t, tf, tn } from '../i18n/messages';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import type { ClipboardMode } from '../services/opsClient';
import type {
	JobId,
	JobRequest,
	JournalEntrySummary,
	OpsCommandError,
} from '../services/opsClient';
import type { VfsClient } from '../services/vfsClient';
import { normaliseUri, pastedUris, pasteRefusal, pasteRequest } from './clipboardRules';
import type { ClipboardService } from './clipboardService';
import { pickDestination, type DestinationOptions } from './destinationStore';
import { errorText } from './jobText';
import { commandErrorText, runRedo, runUndo } from './opsNotices';
import type { OpsHandle } from './opsStore';

// --- What can be done where ------------------------------------------------------------------

export type FileCommandId =
	| 'newFolder'
	| 'newFile'
	| 'rename'
	| 'duplicate'
	| 'moveToTrash'
	| 'deletePermanently'
	| 'cut'
	| 'copy'
	| 'paste'
	| 'pasteInto'
	| 'copyTo'
	| 'moveTo'
	| 'copyToOtherPane'
	| 'moveToOtherPane'
	| 'undo'
	| 'redo';

/** A command is `visible` where it makes sense at all and `enabled` when it can run now. */
export interface CommandState {
	visible: boolean;
	enabled: boolean;
}

/** What decides which commands are offered. */
export interface CommandContext {
	/** The window has a queue to send jobs to. */
	queue: boolean;
	/** A listing is open in the pane the command acts on. */
	listing: boolean;
	/** The listing's provider writes nothing (the Trash, an archive). */
	readOnly: boolean;
	/** How many entries are selected. */
	selected: number;
	/** The keyboard is on an entry, so Rename has one to rename. */
	focused: boolean;
	undo: JournalEntrySummary | null;
	redo: JournalEntrySummary | null;
	/** The listing is the Trash, which nothing is copied out of by the clipboard commands. */
	trash?: boolean;
	/** How many entries the clipboard holds. */
	clipboardItems?: number;
	/** The pane is one half of a pair, so there is another pane to copy and move to. */
	paired?: boolean;
	/** The other pane's folder can be written to. */
	otherPaneWritable?: boolean;
}

/** Whether new items can be made in a listing: its provider says it writes. */
export function canCreateHere(context: Pick<CommandContext, 'queue' | 'listing' | 'readOnly'>) {
	return context.queue && context.listing && !context.readOnly;
}

/**
 * Which commands to offer. Commands that write are hidden in a read-only location rather than
 * shown disabled, because nothing there can ever enable them; Undo and Redo belong to the whole
 * history, so they are always shown and disabled when there is nothing to do.
 */
export function commandStates(context: CommandContext): Record<FileCommandId, CommandState> {
	const writes = canCreateHere(context);
	const state = (visible: boolean, enabled: boolean): CommandState => ({
		visible,
		enabled: visible && enabled,
	});
	// Copying only reads, so it is offered wherever a listing is open (an archive, a read-only
	// share), except the Trash, which has its own menu and its own way out (Restore).
	const reads = context.queue && context.listing && context.trash !== true;
	const selected = context.selected > 0;
	const paired = context.paired === true;
	const otherWrites = context.otherPaneWritable === true;
	return {
		newFolder: state(writes, true),
		newFile: state(writes, true),
		rename: state(writes, context.focused),
		duplicate: state(writes, context.selected > 0),
		moveToTrash: state(writes, context.selected > 0),
		deletePermanently: state(writes, context.selected > 0),
		cut: state(writes, selected),
		copy: state(reads, selected),
		paste: state(writes, (context.clipboardItems ?? 0) > 0),
		pasteInto: state(writes, (context.clipboardItems ?? 0) > 0),
		copyTo: state(reads, selected),
		moveTo: state(writes, selected),
		copyToOtherPane: state(reads && paired, selected && otherWrites),
		moveToOtherPane: state(writes && paired, selected && otherWrites),
		undo: state(context.queue, context.undo !== null),
		redo: state(context.queue, context.redo !== null),
	};
}

// --- The requests ----------------------------------------------------------------------------

const NO_OPTIONS = { conflict: null, verify: null } as const;

/** The selection as the wire form Rust resolves to paths, so a selection of a hundred thousand files stays a handle and a range. */
export function selectionSpec(selection: Selection): SelectionSpec {
	return selection.kind === 'some'
		? { kind: 'some', ids: [...selection.ids] }
		: { kind: 'allExcept', ids: [...selection.ids] };
}

export function selectionSources(handle: ListingHandle, selection: Selection): Sources {
	return { kind: 'selection', handle, spec: selectionSpec(selection) };
}

/**
 * A new folder or file in `folder`. `name` is the default, and `keepBoth` makes the planner pick
 * the first free name (`untitled folder`, then `untitled folder (2)`) instead of refusing a clash.
 */
export function createRequest(
	kind: 'createFolder' | 'createFile',
	folder: Location,
	name: string,
	windowLabel: string,
): JobRequest {
	return {
		kind: { kind },
		sources: { kind: 'locations', locations: [] },
		destination: folder,
		name,
		options: { conflict: 'keepBoth', verify: null },
		originWindow: windowLabel,
	};
}

/** Renames one entry of a listing. A clash is refused (`NameInUse`), never overwritten. */
export function renameRequest(
	handle: ListingHandle,
	id: EntryId,
	name: string,
	windowLabel: string,
): JobRequest {
	return {
		kind: { kind: 'rename' },
		sources: selectionSources(handle, { kind: 'some', ids: new Set([id]) }),
		destination: null,
		name,
		options: NO_OPTIONS,
		originWindow: windowLabel,
	};
}

/** Duplicate, trash or delete the selection in place. */
export function selectionRequest(
	kind: 'duplicate' | 'trash' | 'delete',
	handle: ListingHandle,
	selection: Selection,
	windowLabel: string,
): JobRequest {
	return {
		kind: { kind },
		sources: selectionSources(handle, selection),
		destination: null,
		name: null,
		options: NO_OPTIONS,
		originWindow: windowLabel,
	};
}

// --- Words -----------------------------------------------------------------------------------

/** What a rename that Rust refused says under the field. */
export function renameFailureText(error: unknown, name: string): string {
	const failure = (error as Partial<OpsCommandError> | null)?.error ?? (error as OpsError | null);
	if (failure?.kind === 'nameInUse') return tf('rename.error.exists', { name });
	if (failure?.kind === 'invalidName')
		return tf('rename.error.invalid', { reason: failure.reason });
	return tf('rename.error.failed', { reason: commandErrorText(error) });
}

// --- The dispatcher --------------------------------------------------------------------------

/** What a confirmation asks; `ConfirmHost` draws it with the chrome's `ConfirmDialog`. */
export interface ConfirmSpec {
	title: string;
	message: string;
	/** Names listed under the message, so the person sees what is affected. */
	items?: string[];
	/** A line after the list (how many more, how big). */
	note?: string;
	confirmLabel: string;
	/** What the cancelling button says; "Cancel" when omitted. */
	cancelLabel?: string;
	/** Styles the confirm button as destructive; focus then starts on Cancel. */
	danger: boolean;
}

export type RenameOutcome = { ok: true } | { ok: false; message: string };

export interface FileCommandDeps {
	ops: OpsHandle;
	vfs: VfsClient;
	/** The window's label, which a job started here carries as its `originWindow`. */
	windowLabel: string;
	/** The pane the keys act on. */
	activeSession: () => ListingSession | null;
	confirm: (spec: ConfirmSpec) => Promise<boolean>;
	/** A message that is shown and read out (the toast's region is a live region). */
	say: (text: string) => void;
	/** The window's clipboard; without one Cut, Copy and Paste say so instead of doing anything. */
	clipboard?: ClipboardService | null;
	/** The pane a pair keeps beside `session`, or `null` when it is not half of a pair. */
	otherPane?: (session: ListingSession) => ListingSession | null;
	/** Asks where to copy or move to, in the destination dialog; `null` when the person cancels. */
	pickDestination?: (options: DestinationOptions) => Promise<Location | null>;
}

/** How one step of the history went: done, or why the engine would not (in plain words). */
export type HistoryStepOutcome = { ok: true } | { ok: false; reason: string };

export interface FileCommands {
	/** Which commands to offer for `session` (the active pane's when omitted). */
	states(session?: ListingSession | null): Record<FileCommandId, CommandState>;
	/** The history's newest entries, for the menu labels. */
	history(): { undo: JournalEntrySummary | null; redo: JournalEntrySummary | null };
	newFolder(session?: ListingSession | null): Promise<void>;
	newFile(session?: ListingSession | null): Promise<void>;
	/** Puts `entry` (the focused one when omitted) into inline rename. */
	rename(session?: ListingSession | null, entry?: Entry): void;
	/** Renames `entry`, which the inline field calls; says what is wrong instead of throwing. */
	renameEntry(session: ListingSession, entry: Entry, name: string): Promise<RenameOutcome>;
	duplicate(session?: ListingSession | null): Promise<void>;
	moveToTrash(session?: ListingSession | null): Promise<void>;
	deletePermanently(session?: ListingSession | null): Promise<void>;
	/** Puts the selection on the clipboard to move on paste; the rows dim until it is pasted or replaced. */
	cut(session?: ListingSession | null): Promise<void>;
	/** Puts the selection on the clipboard to copy on paste. */
	copy(session?: ListingSession | null): Promise<void>;
	/**
	 * Pastes the clipboard into the pane's folder, or (`into`) into the folder entry that was
	 * chosen; a cut moves and then empties the clipboard.
	 */
	paste(session?: ListingSession | null, into?: Entry): Promise<void>;
	/** Copy To…: asks for a folder, then copies the selection there. */
	copyTo(session?: ListingSession | null): Promise<void>;
	/** Move To…: asks for a folder, then moves the selection there. */
	moveTo(session?: ListingSession | null): Promise<void>;
	/**
	 * F5: copies the selection to the other pane's folder. Without a pair, or when the other
	 * pane cannot be written to, it asks for a folder as Copy To… does; with nothing selected it
	 * says so.
	 */
	copyToOtherPane(session?: ListingSession | null): Promise<void>;
	/** Shift+F5: the same, moving. */
	moveToOtherPane(session?: ListingSession | null): Promise<void>;
	/**
	 * Link To…: asks for a folder, then makes links to the selection there (the drop's Link Here,
	 * without a pointer). Local items only: the engine refuses a link across kinds of location.
	 */
	linkTo(session?: ListingSession | null): Promise<void>;
	/**
	 * What a drop does: copies, moves or links the selection of `session` into `destination`. The
	 * refusals are the commands' (a move into the folder the items are in, a read-only source for a
	 * move); a copy into the same folder is a duplicate, and a link there is a link.
	 */
	transferTo(
		kind: 'copy' | 'move' | 'link',
		session: ListingSession,
		destination: Location,
	): Promise<void>;
	/**
	 * What a drop of files from outside the window does (another application, or another window):
	 * copies, moves or links `items`, named by location, into `destination`. A folder dropped into
	 * itself and a move onto the folder the items are in are refused as a paste refuses them
	 * (`pasteRefusal`); a copy beside the originals is a duplicate.
	 */
	transferLocations(
		kind: 'copy' | 'move' | 'link',
		items: Location[],
		destination: Location,
	): Promise<void>;
	undo(): Promise<void>;
	redo(): Promise<void>;
	/** The undo history, newest first, as the Undo History menu lists it. */
	historyEntries(): Promise<JournalEntrySummary[]>;
	/** Undoes one entry of the history (a refusal says why, as Undo does). */
	undoEntry(entry: number): Promise<void>;
	/** Redoes one entry of the history. */
	redoEntry(entry: number): Promise<void>;
	/**
	 * Undoes or redoes one entry and waits for the job, saying how it went instead of showing it:
	 * the palette chains these for an older entry and reports where a chain stopped.
	 */
	stepHistory(kind: 'undo' | 'redo', entry: number): Promise<HistoryStepOutcome>;
}

/** How long a command waits for its job before it stops following it; the job carries on. */
const JOB_WAIT_MS = 30_000;
const NAMES_SHOWN = 5;

/**
 * Waits for a job to finish, as the store mirrors it. Resolves to the finished snapshot, or to
 * `null` if the job leaves the queue (it was dismissed) or `timeoutMs` passes first.
 */
export function waitForJob(
	ops: OpsHandle,
	id: JobId,
	timeoutMs = JOB_WAIT_MS,
): Promise<JobSnapshot | null> {
	return new Promise((resolve) => {
		let seen = false;
		let stop = () => {};
		const timer = setTimeout(() => {
			stop();
			resolve(null);
		}, timeoutMs);
		const check = () => {
			const job = ops.store.getState().snapshot?.jobs.find((candidate) => candidate.id === id);
			if (!job) {
				// The job's event can arrive just after the command resolves; only a job that was seen and went has left.
				if (!seen) return;
			} else {
				seen = true;
				const { state } = job.state;
				if (state !== 'done' && state !== 'failed' && state !== 'cancelled') return;
			}
			clearTimeout(timer);
			stop();
			resolve(job ?? null);
		};
		stop = ops.store.subscribe(check);
		check();
	});
}

/**
 * The selection as explicit ids. A Select All is "everything except these", which Rust resolves
 * against the listing as it is when the job arrives; a dialog that lists what will go must send
 * exactly what it listed, so a file that appears while it is open is never part of the request.
 */
export async function freezeSelection(
	model: ListingSession['model'],
	selection: Selection,
): Promise<Selection> {
	if (selection.kind === 'some') return { kind: 'some', ids: new Set(selection.ids) };
	const ids = new Set<EntryId>();
	for (let position = 0; position < model.count; position += PAGE_SIZE) {
		for (const entry of await model.readRange(
			position,
			Math.min(model.count, position + PAGE_SIZE),
		)) {
			if (isSelected(selection, entry.id)) ids.add(entry.id);
		}
	}
	return { kind: 'some', ids };
}

function focusedEntry(session: ListingSession): Entry | undefined {
	const { focus } = session.store.getState();
	return focus === null ? undefined : session.model.entryAt(focus);
}

export function createFileCommands(deps: FileCommandDeps): FileCommands {
	const { ops, vfs, windowLabel, say } = deps;

	const history = () => {
		const journal = ops.store.getState().snapshot?.journal;
		return { undo: journal?.undo ?? null, redo: journal?.redo ?? null };
	};

	const contextFor = (session: ListingSession | null): CommandContext => {
		const { undo, redo } = history();
		const state = session?.store.getState();
		const other = session ? (deps.otherPane?.(session) ?? null) : null;
		return {
			trash: session?.model.layout === 'trash',
			clipboardItems: deps.clipboard?.store.getState().clipboard.items.length ?? 0,
			paired: other !== null,
			otherPaneWritable: other !== null && !other.model.readOnly,
			queue: true,
			listing: session !== null,
			readOnly: session?.model.readOnly ?? false,
			selected: session && state ? selectedCount(state.selection, session.model.count) : 0,
			focused: session !== null && focusedEntry(session) !== undefined,
			undo,
			redo,
		};
	};

	const target = (session?: ListingSession | null) => session ?? deps.activeSession();

	/** The session to write in, or `null` after saying why not. */
	const writable = (session?: ListingSession | null): ListingSession | null => {
		const found = target(session);
		if (!found) return null;
		if (found.model.readOnly) {
			say(t('files.readOnly'));
			return null;
		}
		return found;
	};

	/** The session with something selected to write to, or `null` after saying why not. */
	const withSelection = (session?: ListingSession | null): ListingSession | null => {
		const found = writable(session);
		if (!found) return null;
		if (selectedCount(found.store.getState().selection, found.model.count) === 0) {
			say(t('files.nothingSelected'));
			return null;
		}
		return found;
	};

	/** Puts the request on the queue and waits for the job to end. `null` when it was refused or did not end in time. */
	const run = async (request: JobRequest): Promise<JobSnapshot | null> => {
		let id: JobId;
		try {
			id = await ops.submitJob(request);
		} catch (error) {
			say(tf('files.failed', { reason: commandErrorText(error) }));
			return null;
		}
		return waitForJob(ops, id);
	};

	/** A job that ended badly says so; true when it did. */
	const reportFailure = (job: JobSnapshot | null): boolean => {
		if (job?.state.state !== 'failed') return false;
		say(tf('files.failed', { reason: errorText(job.state.error) }));
		return true;
	};

	const create = async (kind: 'createFolder' | 'createFile', session?: ListingSession | null) => {
		const found = writable(session);
		if (!found) return;
		const fallback = t(kind === 'createFolder' ? 'files.default.folder' : 'files.default.file');
		const job = await run(createRequest(kind, found.model.location, fallback, windowLabel));
		if (reportFailure(job) || job?.state.state !== 'done') return;
		// The planner picked the first free name; the listing shows it once its watcher has seen it.
		// Inline rename starts with it selected. Escape keeps the name as made, and Enter renames it
		// as one more undoable step.
		await revealByName(found.model, found.store, job.sources.first ?? fallback, {
			rename: true,
		}).catch(() => false);
	};

	const confirmDelete = async (
		found: ListingSession,
		selection: Selection,
		spec: { title: string; message?: string; reason?: string },
	): Promise<boolean> => {
		const { model } = found;
		const count = selectedCount(selection, model.count);
		const names = await firstSelectedNames(model, selection, NAMES_SHOWN).catch(() => []);
		const summary = await vfs
			.summariseSelection(model.handle, selectionSpec(selection))
			.catch(() => null);
		const more = count - names.length;
		const note = [
			more > 0 ? tf('files.delete.more', { count: new Intl.NumberFormat().format(more) }) : null,
			summary && summary.totalSize > 0
				? tf('files.delete.size', { size: formatSize(summary.totalSize) })
				: null,
		]
			.filter((part) => part !== null)
			.join(' · ');
		return deps.confirm({
			title: spec.title,
			message:
				spec.message ??
				(count === 1
					? t('files.delete.intro.one')
					: tf('files.delete.intro.other', { count: new Intl.NumberFormat().format(count) })),
			items: names,
			...(note ? { note } : {}),
			confirmLabel: t('files.delete.action'),
			danger: true,
		});
	};

	/** Deletes exactly `selection`, the one the person confirmed. */
	const runDelete = async (found: ListingSession, selection: Selection) => {
		const job = await run(selectionRequest('delete', found.model.handle, selection, windowLabel));
		reportFailure(job);
	};

	/** The session with something selected to read, or `null` after saying why not. Reading is allowed in a read-only listing. */
	const withReadableSelection = (session?: ListingSession | null): ListingSession | null => {
		const found = target(session);
		if (!found) return null;
		if (selectedCount(found.store.getState().selection, found.model.count) === 0) {
			say(t('files.nothingSelected'));
			return null;
		}
		return found;
	};

	const putOnClipboard = async (mode: ClipboardMode, session?: ListingSession | null) => {
		const found = mode === 'cut' ? withSelection(session) : withReadableSelection(session);
		if (!found) return;
		const { clipboard } = deps;
		if (!clipboard) {
			say(t('files.noQueue'));
			return;
		}
		const { model, store } = found;
		const selection = store.getState().selection;
		try {
			await clipboard.setFromSelection(model.handle, selectionSpec(selection), mode);
		} catch (error) {
			say(tf('files.failed', { reason: commandErrorText(error) }));
			return;
		}
		say(tn(mode === 'cut' ? 'files.cut' : 'files.copied', selectedCount(selection, model.count)));
	};

	const paste = async (session?: ListingSession | null, into?: Entry) => {
		const found = writable(session);
		if (!found) return;
		const { clipboard } = deps;
		if (!clipboard) {
			say(t('files.noQueue'));
			return;
		}
		// Files another application copied since are adopted first, so Ctrl+V pastes what was last copied anywhere.
		const board = await clipboard.forPaste().catch(() => clipboard.store.getState().clipboard);
		if (board.items.length === 0) {
			say(t('files.paste.nothing'));
			return;
		}
		let destination = found.model.location;
		if (into) {
			try {
				destination = await vfs.entryLocation(found.model.handle, into.id);
			} catch (error) {
				say(tf('files.failed', { reason: commandErrorText(error) }));
				return;
			}
		}
		const refusal = pasteRefusal(board.mode, board.items, destination);
		if (refusal) {
			say(errorText(refusal));
			return;
		}
		let id: JobId;
		try {
			id = await ops.submitJob(pasteRequest(board, destination, windowLabel));
		} catch (error) {
			say(tf('files.failed', { reason: commandErrorText(error) }));
			return;
		}
		// A cut is spent once it is on the queue: the rows stop dimming, and a second paste has nothing to move.
		// A Copy made elsewhere since the paste began is a newer clipboard and stays.
		if (board.mode === 'cut') {
			await clipboard
				.clear(board.revision, pastedUris(board.items, destination))
				.catch(() => false);
		}
		reportFailure(await waitForJob(ops, id));
	};

	/** Copies or moves the selection into `destination`; a copy into its own folder is a duplicate, and a move there is refused. */
	const transfer = async (
		kind: 'copy' | 'move' | 'link',
		found: ListingSession,
		destination: Location,
	): Promise<void> => {
		const { model, store } = found;
		const sources = selectionSources(model.handle, store.getState().selection);
		const here = normaliseUri(destination.uri) === normaliseUri(model.location.uri);
		if (here && kind === 'move') {
			say(errorText({ kind: 'sameFolder' }));
			return;
		}
		const duplicating = here && kind === 'copy';
		const request: JobRequest = {
			kind: { kind: duplicating ? 'duplicate' : kind },
			sources,
			destination: duplicating ? null : destination,
			name: null,
			options: NO_OPTIONS,
			originWindow: windowLabel,
		};
		reportFailure(await run(request));
	};

	/** Asks where to, then transfers. */
	const transferViaDialog = async (
		kind: 'copy' | 'move' | 'link',
		session?: ListingSession | null,
	) => {
		const found = kind === 'move' ? withSelection(session) : withReadableSelection(session);
		if (!found) return;
		const count = selectedCount(found.store.getState().selection, found.model.count);
		const ask = deps.pickDestination ?? pickDestination;
		const destination = await ask({
			title: tn(`destination.title.${kind}`, count),
			confirmLabel: t(`destination.${kind}`),
			base: found.model.location,
			// A copy into the folder the items are in is a duplicate; a move there has nothing to do.
			origin: found.model.location,
			forbidOrigin: kind === 'move',
		});
		if (destination) await transfer(kind, found, destination);
	};

	/** F5 and Shift+F5: the other pane's folder when there is one that can be written to, otherwise the dialog. */
	const transferToOtherPane = async (kind: 'copy' | 'move', session?: ListingSession | null) => {
		const found = kind === 'move' ? withSelection(session) : withReadableSelection(session);
		if (!found) return;
		const other = deps.otherPane?.(found) ?? null;
		if (!other || other.model.readOnly) return transferViaDialog(kind, found);
		await transfer(kind, found, other.model.location);
	};

	return {
		states: (session) =>
			commandStates(contextFor(session === undefined ? deps.activeSession() : session)),
		history,

		newFolder: (session) => create('createFolder', session),
		newFile: (session) => create('createFile', session),

		rename(session, entry) {
			const found = writable(session);
			if (!found) return;
			const subject = entry ?? focusedEntry(found);
			if (!subject) {
				say(t('files.nothingFocused'));
				return;
			}
			found.store.getState().beginRename(subject.id);
		},

		async renameEntry(session, entry, name) {
			const request = renameRequest(session.model.handle, entry.id, name, windowLabel);
			try {
				// Planning first finds a clash or a bad name without leaving a failed job in the queue.
				await ops.client.plan(request);
			} catch (error) {
				return { ok: false, message: renameFailureText(error, name) };
			}
			let id: JobId;
			try {
				id = await ops.submitJob(request);
			} catch (error) {
				return { ok: false, message: renameFailureText(error, name) };
			}
			const job = await waitForJob(ops, id);
			if (job?.state.state === 'failed') {
				const { error } = job.state;
				// The field shows this; the queue need not keep a failed job for it.
				if (error.kind === 'nameInUse' || error.kind === 'invalidName') {
					void ops.client.dismiss(id).catch(() => {});
				}
				return { ok: false, message: renameFailureText(error, name) };
			}
			if (job?.state.state !== 'done') {
				return { ok: false, message: tf('rename.error.failed', { reason: t('files.noQueue') }) };
			}
			// The renamed entry keeps its selection and focus: find it under its new name.
			await revealByName(session.model, session.store, name, { timeoutMs: 2000 }).catch(
				() => false,
			);
			say(tf('rename.done', { from: entry.name, to: name }));
			return { ok: true };
		},

		async duplicate(session) {
			const found = withSelection(session);
			if (!found) return;
			const tracker = new InsertTracker(found.model);
			try {
				const job = await run(
					selectionRequest(
						'duplicate',
						found.model.handle,
						found.store.getState().selection,
						windowLabel,
					),
				);
				if (reportFailure(job) || job?.state.state !== 'done') return;
				// The copies took whatever names were free, so they are the entries the listing gained.
				await waitForInserts(found.model, tracker, job.sources.count ?? 1);
				await revealInserted(found.store, tracker);
			} finally {
				tracker.dispose();
			}
		},

		async moveToTrash(session) {
			const found = withSelection(session);
			if (!found) return;
			const { model, store } = found;
			// What is confirmed is what is sent: a file that arrives while the dialog is open is not part of it.
			const selection = await freezeSelection(model, store.getState().selection);
			const settings = await ops.client.getSettings().catch(() => null);
			if (settings?.confirmTrash) {
				const count = selectedCount(selection, model.count);
				const [name] = await firstSelectedNames(model, selection, 1).catch(() => []);
				const ok = await deps.confirm({
					title: t('files.trash.confirm.title'),
					message:
						count === 1 && name
							? tf('files.trash.confirm.named', { name: `“${name}”` })
							: tn('files.trash.confirm', count),
					confirmLabel: t('files.trash.confirm.action'),
					danger: false,
				});
				if (!ok) return;
			}
			const job = await run(selectionRequest('trash', model.handle, selection, windowLabel));
			if (job?.state.state === 'failed' && job.state.error.kind === 'trashUnavailable') {
				// A volume with no Trash: offer the permanent delete, said plainly.
				void ops.client.dismiss(job.id).catch(() => {});
				const ok = await confirmDelete(found, selection, {
					title: t('files.trash.unavailable.title'),
					message: tf('files.trash.unavailable.message', { reason: job.state.error.reason }),
				});
				if (ok) await runDelete(found, selection);
				return;
			}
			reportFailure(job);
			// A finished job that can be undone gets its Undo toast from `startUndoNotices`.
		},

		async deletePermanently(session) {
			const found = withSelection(session);
			if (!found) return;
			// Always asked, whatever the settings say (D104); nothing here can be undone.
			const selection = await freezeSelection(found.model, found.store.getState().selection);
			if (await confirmDelete(found, selection, { title: t('files.delete.title') })) {
				await runDelete(found, selection);
			}
		},

		cut: (session) => putOnClipboard('cut', session),
		copy: (session) => putOnClipboard('copy', session),
		paste,
		copyTo: (session) => transferViaDialog('copy', session),
		moveTo: (session) => transferViaDialog('move', session),
		copyToOtherPane: (session) => transferToOtherPane('copy', session),
		moveToOtherPane: (session) => transferToOtherPane('move', session),

		linkTo: (session) => transferViaDialog('link', session),

		async transferTo(kind, session, destination) {
			const found = kind === 'move' ? withSelection(session) : withReadableSelection(session);
			if (found) await transfer(kind, found, destination);
		},

		async transferLocations(kind, items, destination) {
			if (items.length === 0) return;
			if (kind !== 'link') {
				const refusal = pasteRefusal(kind === 'move' ? 'cut' : 'copy', items, destination);
				if (refusal) {
					say(errorText(refusal));
					return;
				}
			}
			const request: JobRequest =
				kind === 'link'
					? {
							kind: { kind: 'link' },
							sources: { kind: 'locations', locations: [...items] },
							destination,
							name: null,
							options: NO_OPTIONS,
							originWindow: windowLabel,
						}
					: pasteRequest(
							{ mode: kind === 'move' ? 'cut' : 'copy', items },
							destination,
							windowLabel,
						);
			reportFailure(await run(request));
		},

		async undo() {
			if (!history().undo) {
				say(t('files.undo.nothing'));
				return;
			}
			await runUndo(ops, (text) => say(text));
		},

		async redo() {
			if (!history().redo) {
				say(t('files.redo.nothing'));
				return;
			}
			await runRedo(ops, (text) => say(text));
		},

		historyEntries: () => ops.client.journalSummaries(),

		async undoEntry(entry) {
			await runUndo(ops, (text) => say(text), entry);
		},

		async redoEntry(entry) {
			await runRedo(ops, (text) => say(text), entry);
		},

		async stepHistory(kind, entry) {
			let id: JobId;
			try {
				id = await (kind === 'undo' ? ops.undo(entry) : ops.redo(entry));
			} catch (error) {
				return { ok: false, reason: commandErrorText(error) };
			}
			const job = await waitForJob(ops, id);
			if (!job) return { ok: false, reason: t('history.step.unknown') };
			const { state } = job;
			if (state.state === 'failed') return { ok: false, reason: errorText(state.error) };
			if (state.state === 'cancelled') return { ok: false, reason: t('history.step.cancelled') };
			return { ok: true };
		},
	};
}

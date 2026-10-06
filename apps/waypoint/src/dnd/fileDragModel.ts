// What a file drag carries and what a target would do with it: the types, the verdict rules and the words, all pure
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import type { TransferEnds } from '@liminal-hq/waypoint-protocol/generated/TransferEnds';
import type { ListingSession } from '../browse/useListingSession';
import { t, tf, tn } from '../i18n/messages';
import { normaliseUri } from '../ops/clipboardRules';
import type { FileCommandId } from '../ops/fileCommands';
import { isArchiveLocation } from '../archives/archiveNames';
import { errorText } from '../ops/jobText';
import type { ListingHandle, Location, SelectionSpec } from '../services/opsClient';
import type { DragPill } from './dragSession';
import {
	chooseDropAction,
	type DropActionInput,
	type DropModifiers,
	type DropVerb,
	type VolumeRelation,
} from './dropAction';
import type { DropKind, DropSpot } from './dropTargets';

/** Files that came from outside any listing of this window: from another application or window, or this window's own drag coming back. */
export interface ExternalFiles {
	/** Each file's location, from the lossless `file:` URIs the system gave. */
	locations: Location[];
	/** The files are the ones this window handed to the system as an outbound drag. */
	own: boolean;
}

/** What every drag carries, whatever it was taken from. */
interface FileDragBase {
	/** The pane (tab) the drag began in, when the view is a pane's. */
	tab: number | null;
	count: number;
	/** The one name, when exactly one item is dragged. */
	name: string | null;
	/** The icons of the stack: up to three. */
	groups: IconGroup[];
	/** The thumbnails that had loaded when the drag began, aligned with `groups`: the stack shows them over the icons. */
	thumbnails?: Array<string | null>;
	/** The folder the files are in; `null` for external files that are not all in one folder. */
	folder: Location | null;
	/** The folder cannot be written to, so the files cannot be moved or trashed out of it. */
	readOnly: boolean;
	/** The right button started the drag, which opens the picker on release. */
	rightButton: boolean;
}

/** What is being dragged: the selection of one listing, as Rust will resolve it. */
export interface SelectionDragSource extends FileDragBase {
	kind?: 'selection';
	/** The listing the files are in; held open for the drag (`ListingManager.retain`). */
	session: ListingSession;
	handle: ListingHandle;
	spec: SelectionSpec;
	external?: undefined;
}

/**
 * What is being dragged: references, as `Sources::Locations`. They are the Shelf's items, or, when
 * `external` is set, files that came from outside any listing of this window. Either way the files
 * are named by location and no listing is held open.
 */
export interface LocationsDragSource extends FileDragBase {
	kind: 'locations';
	locations: Location[];
	/** Set for files that came from another application or window (or this window's own outbound drag coming back). */
	external?: ExternalFiles;
}

export type FileDragSource = SelectionDragSource | LocationsDragSource;

export function isLocationsSource(source: FileDragSource): source is LocationsDragSource {
	return source.kind === 'locations';
}

/** Items taken from the Shelf: references that did not come from outside the window. */
export function isShelfSource(source: FileDragSource): source is LocationsDragSource {
	return source.kind === 'locations' && !source.external;
}

/** Why a target refuses the drop. */
export type BlockReason =
	| { kind: 'sameFolder' }
	| { kind: 'intoItself' }
	/** The target is one of the dragged items. */
	| { kind: 'source' }
	| { kind: 'readOnly' }
	/** The Trash as a folder: items are trashed by dropping on the sidebar's Trash. */
	| { kind: 'trashView' }
	| { kind: 'trashSource' }
	/** Items taken from the Shelf can only be dropped into folders. */
	| { kind: 'shelfSource' }
	/** The Shelf is the target and the items are already on it. */
	| { kind: 'onShelf' }
	| { kind: 'unavailable' }
	| { kind: 'refused'; error: OpsError };

/** What a release does: a job (copy, move, link, trash), the picker, opening tabs, or putting references on the Shelf. */
export type DropOutcome = DropVerb | 'trash' | 'open' | 'shelf';

/** The target under the pointer and what a release over it would do. It is the drag's store `target`. */
export interface FileDropTarget {
	kind: DropKind;
	ref: string;
	label: string;
	/** Where the files would go; `null` while a folder row's location is being resolved, and for targets that are not folders. */
	location: Location | null;
	/** `null` when blocked. */
	outcome: DropOutcome | null;
	blocked: BlockReason | null;
	volume: VolumeRelation;
	/** The outcome is a guess until the planner says which volume the target is on. */
	pending: boolean;
	/**
	 * The drop sends the files to a server or brings them from one (D151), with the server's name,
	 * so the pill can say "Upload" or "Download"; absent between local folders, or until the
	 * planner has answered.
	 */
	transfer?: Transfer | undefined;
}

/** Which way a drop sends files between this computer and a server, and the server it names. */
export interface Transfer {
	way: 'upload' | 'download' | 'across';
	server: string;
}

/**
 * Which way the files go, from the logins the planner found (`TransferEnds`): onto a server from
 * elsewhere is an upload, from a server to a local folder a download, and from one server to
 * another (or to another place on the same one) a copy across. `name` turns a login into what the
 * person calls it.
 */
export function transferOf(
	ends: TransferEnds | null | undefined,
	name: (login: string) => string,
): Transfer | undefined {
	if (!ends) return undefined;
	if (ends.to !== null) {
		const local = ends.from.length === 0;
		return { way: local ? 'upload' : 'across', server: name(ends.to) };
	}
	const from = ends.from[0];
	return from === undefined ? undefined : { way: 'download', server: name(from) };
}

export function sameFileTarget(a: FileDropTarget, b: FileDropTarget): boolean {
	return (
		a.kind === b.kind &&
		a.ref === b.ref &&
		a.label === b.label &&
		a.outcome === b.outcome &&
		a.pending === b.pending &&
		a.location?.uri === b.location?.uri &&
		a.transfer?.way === b.transfer?.way &&
		a.transfer?.server === b.transfer?.server &&
		JSON.stringify(a.blocked) === JSON.stringify(b.blocked)
	);
}

/** What the planner said about a source and a target, kept for the length of the drag. */
export interface PlanFact {
	volume: VolumeRelation;
	/** A refusal the planner gave (into itself, permission denied, …). */
	error: OpsError | null;
	/** Which way the files would go between this computer and a server, when they would. */
	transfer?: Transfer | undefined;
}

export interface EvaluateInput {
	source: FileDragSource;
	spot: DropSpot;
	/** The target's folder, when it is known. */
	location: Location | null;
	/** Whether a pane's folder, or the row's listing, cannot be written to. */
	readOnly: boolean;
	/** The dragged selection contains the row (a folder row of the source listing). */
	selfRow: boolean;
	plan: PlanFact | null;
	modifiers: DropModifiers;
	rule: DropActionInput['rule'];
	canLink: boolean;
	/** The system has a Trash to move to. */
	trashAvailable: boolean;
}

const TRASH_SCHEME = 'trash:';

export function isTrashUri(uri: string): boolean {
	return uri.startsWith(TRASH_SCHEME);
}

/** Whether `location` is the folder the files are already in. */
export function isSourceFolder(
	source: Pick<FileDragSource, 'folder'>,
	location: Location | null,
): boolean {
	return (
		location !== null &&
		source.folder !== null &&
		normaliseUri(location.uri) === normaliseUri(source.folder.uri)
	);
}

/** Whether links to the files can be made: they, and the folder they are in, are local. */
export function canLinkSource(source: FileDragSource): boolean {
	return isLocationsSource(source)
		? source.locations.every((location) => location.uri.startsWith('file:'))
		: (source.folder?.uri.startsWith('file:') ?? false);
}

/**
 * The verdict for one target (SPEC §6):
 * - the Trash moves to the Trash, unless the files cannot be moved out of where they are, and
 *   nothing else may be dropped into the Trash's own listing;
 * - the + button and a group chip open tabs and write nothing, so they take any drag;
 * - a folder is refused when it is read-only, is one of the dragged items, is the folder the files
 *   are in (for a move: a copy there is a duplicate), or the planner refused it (a folder into
 *   itself or its own descendant, permission denied, …);
 * - otherwise the default rule and the modifiers choose (`chooseDropAction`).
 */
export function evaluateTarget(input: EvaluateInput): FileDropTarget {
	const { source, spot, location, plan, modifiers } = input;
	const base: FileDropTarget = {
		kind: spot.kind,
		ref: spot.ref,
		label: spot.label,
		location,
		outcome: null,
		blocked: null,
		volume: plan?.volume ?? 'unknown',
		pending: false,
	};
	const block = (blocked: BlockReason): FileDropTarget => ({ ...base, blocked });

	// References taken from the Shelf go into folders; the Trash, the + button and a chip are not that.
	if (isShelfSource(source) && ['trash', 'plus', 'chip'].includes(spot.kind)) {
		return block({ kind: 'shelfSource' });
	}

	switch (spot.kind) {
		case 'trash':
			if (spot.unavailable || !input.trashAvailable) return block({ kind: 'unavailable' });
			// Files from another application are not trashed by a drop: Delete in their own file manager does that.
			if (source.external || source.readOnly || (source.folder && isTrashUri(source.folder.uri))) {
				return block({ kind: 'trashSource' });
			}
			return { ...base, outcome: 'trash' };
		case 'plus':
		case 'chip':
			return { ...base, outcome: 'open' };
		// Dropping on the Shelf writes no file: it adds references, so nothing can refuse it here.
		case 'shelf':
			return isShelfSource(source) ? block({ kind: 'onShelf' }) : { ...base, outcome: 'shelf' };
		default:
			break;
	}

	if (input.selfRow) return block({ kind: 'source' });
	if (input.readOnly || spot.readOnly) return block({ kind: 'readOnly' });
	if (location && isTrashUri(location.uri)) return block({ kind: 'trashView' });
	if (plan?.error) return block({ kind: 'refused', error: plan.error });

	const here = isSourceFolder(source, location);
	const volume: VolumeRelation = here ? 'same' : (plan?.volume ?? 'unknown');
	const choice = chooseDropAction({
		rule: input.rule,
		volume,
		modifiers,
		rightButton: source.rightButton,
		// Nothing is moved into an archive: it is added to, and the originals stay (D170).
		canMove: !source.readOnly && !(location && isArchiveLocation(location)),
		canLink: input.canLink,
		// This window's own drag coming back is known to be movable; anything else from outside is not.
		foreign: source.external !== undefined && !source.external.own,
	});
	if (here && choice.verb === 'move') return block({ kind: 'sameFolder' });
	return {
		...base,
		volume,
		outcome: choice.verb,
		pending: choice.pending,
		...(plan?.transfer && !here ? { transfer: plan.transfer } : {}),
	};
}

// --- Words -----------------------------------------------------------------------------------

/** "report.pdf" for one item, "3 items" for several. */
export function subjectText(source: Pick<FileDragSource, 'count' | 'name'>): string {
	return source.count === 1 && source.name ? source.name : tn('dnd.items', source.count);
}

/** "Dragging report.pdf" or "Dragging 3 items". */
export function dragText(source: Pick<FileDragSource, 'count' | 'name'>): string {
	return source.count === 1 && source.name
		? tf('dnd.drag.one', { name: source.name })
		: tn('dnd.drag.items', source.count);
}

export function blockedText(blocked: BlockReason, target: Pick<FileDropTarget, 'label'>): string {
	switch (blocked.kind) {
		case 'sameFolder':
			return tf('dnd.blocked.sameFolder', { target: target.label });
		case 'intoItself':
			return t('dnd.blocked.intoItself');
		case 'source':
			return tf('dnd.blocked.source', { target: target.label });
		case 'readOnly':
			return tf('dnd.blocked.readOnly', { target: target.label });
		case 'trashView':
			return t('dnd.blocked.trashView');
		case 'trashSource':
			return t('dnd.blocked.trashSource');
		case 'shelfSource':
			return t('dnd.blocked.shelfSource');
		case 'onShelf':
			return t('dnd.blocked.onShelf');
		case 'unavailable':
			return t('dnd.blocked.unavailable');
		case 'refused':
			return blocked.error.kind === 'intoItself'
				? t('dnd.blocked.intoItself')
				: tf('dnd.blocked.refused', { reason: errorText(blocked.error) });
	}
}

/** The pill kinds, which the stylesheet and the badge key on. */
export type PillKind =
	'idle' | 'copy' | 'move' | 'link' | 'ask' | 'trash' | 'open' | 'shelf' | 'blocked' | 'pending';

function verbWord(target: FileDropTarget): string {
	if (target.pending && target.outcome === 'copy') return t('dnd.verb.moveOrCopy');
	switch (target.outcome) {
		case 'copy':
		case 'move':
		case 'link':
		case 'ask':
		case 'trash':
		case 'open':
		case 'shelf':
			return t(`dnd.verb.${target.outcome}`);
		default:
			return '';
	}
}

/**
 * The label beside the pointer: what a release would do ("Copy 3 items to Documents"), and the
 * sentence the live region reads for the same state ("Over Documents: will copy"). A pill with no
 * target says what is being dragged.
 */
export function pillFor(
	source: Pick<FileDragSource, 'count' | 'name'>,
	target: FileDropTarget | null,
	alt = false,
): DragPill {
	const what = subjectText(source);
	if (!target) {
		const text = dragText(source);
		return { text, kind: 'idle' };
	}
	const name = target.label;
	if (target.blocked) {
		const reason = blockedText(target.blocked, target);
		return {
			text: tf('dnd.pill.blocked', { reason }),
			kind: 'blocked',
			announce: tf('dnd.announce.blocked', { target: name, reason }),
		};
	}
	const announce = tf('dnd.announce.over', { target: name, action: verbWord(target) });
	const transfer = target.transfer;
	switch (target.outcome) {
		case 'copy':
			if (target.pending) {
				return {
					text: tf('dnd.pill.moveOrCopy', { what, target: name }),
					kind: 'pending',
					announce,
				};
			}
			return {
				text: transfer
					? tf(`dnd.pill.${transfer.way}`, { what, target: name, server: transfer.server })
					: tf('dnd.pill.copy', { what, target: name }),
				kind: 'copy',
				announce: transfer
					? tf(`dnd.announce.${transfer.way}`, { target: name, server: transfer.server })
					: announce,
			};
		case 'move':
			return {
				text: transfer
					? tf(transfer.way === 'download' ? 'dnd.pill.moveFrom' : 'dnd.pill.moveTo', {
							what,
							target: name,
							server: transfer.server,
						})
					: tf('dnd.pill.move', { what, target: name }),
				kind: 'move',
				announce,
			};
		case 'link':
			return { text: tf('dnd.pill.link', { what, target: name }), kind: 'link', announce };
		case 'ask':
			return { text: tf('dnd.pill.ask', { what, target: name }), kind: 'ask', announce };
		case 'trash':
			return { text: tf('dnd.pill.trash', { what }), kind: 'trash', announce };
		case 'shelf':
			return { text: tf('dnd.pill.shelf', { what }), kind: 'shelf', announce };
		case 'open':
			return {
				text:
					target.kind === 'chip'
						? tf('dnd.pill.openInGroup', { target: name })
						: alt
							? t('dnd.pill.openSplit')
							: t(source.count === 1 ? 'dnd.pill.openTab' : 'dnd.pill.openTabs'),
				kind: 'open',
				announce,
			};
		default:
			return { text: dragText(source), kind: 'idle' };
	}
}

// --- Without a pointer -----------------------------------------------------------------------

/** How a drop's outcome is reached without a pointer (`docs/accessibility.md`: every drop has a non-pointer path). */
export type NonPointerPath =
	| { kind: 'command'; command: FileCommandId | 'linkTo'; keys?: string }
	| { kind: 'menu'; item: string }
	| { kind: 'none'; why: string };

/**
 * Every outcome of a file drop and the command or menu item that does the same without a drag. A
 * new `DropOutcome` fails to compile until it is listed, and `fileDragParity.test.tsx` checks that
 * each path names something that exists.
 */
export const NON_POINTER_PATHS: Record<DropOutcome, NonPointerPath> = {
	copy: { kind: 'command', command: 'copyTo' },
	move: { kind: 'command', command: 'moveTo' },
	link: { kind: 'command', command: 'linkTo' },
	trash: { kind: 'command', command: 'moveToTrash', keys: 'Delete' },
	open: { kind: 'menu', item: 'openInNewTab' },
	shelf: { kind: 'menu', item: 'addToShelf' },
	ask: {
		kind: 'none',
		why: 'the picker only chooses between copy, move and link, each of which has its own path',
	},
};

/**
 * The same outcomes for files dragged in from another application. Copy and Cut there put the
 * files on the system clipboard, which a paste in Waypoint adopts (D107), so Paste is the way to a
 * copy or a move; a link, the Trash and tabs are made from Waypoint's own selection or commands.
 */
export const NATIVE_DROP_PATHS: Record<DropOutcome, NonPointerPath> = {
	copy: { kind: 'command', command: 'paste', keys: 'Ctrl+V' },
	move: { kind: 'command', command: 'paste', keys: 'Ctrl+V' },
	link: {
		kind: 'none',
		why: 'a link to files in another application is made in that application, or with Link To… once they are in a Waypoint folder',
	},
	trash: {
		kind: 'none',
		why: 'a drop never trashes files that came from another application: they are deleted where they are',
	},
	open: { kind: 'menu', item: 'openInNewTab' },
	shelf: {
		kind: 'none',
		why: 'files in another application are put on the Shelf by dropping them there, or with Add to Shelf once they are in a Waypoint folder',
	},
	ask: {
		kind: 'none',
		why: 'the picker only chooses between copy, move and link, each of which has its own path',
	},
};

/** How a drag out of the window to another application is done without a pointer: put the files on the system clipboard and paste there. */
export const OUTBOUND_PATHS: Record<'copy' | 'move', NonPointerPath> = {
	copy: { kind: 'command', command: 'copy', keys: 'Ctrl+C' },
	move: { kind: 'command', command: 'cut', keys: 'Ctrl+X' },
};

/**
 * How each kind of target is reached without a pointer: the destination dialog (`pickDestination`)
 * offers the places, the open tabs and the recent folders, Paste Into Folder is a folder's menu
 * item, Paste and F5 act on the pane, and Delete moves to the Trash.
 */
export const TARGET_PATHS: Record<DropKind, string> = {
	folder: 'pasteInto',
	pane: 'paste',
	place: 'copyTo',
	crumb: 'copyTo',
	tab: 'copyTo',
	trash: 'moveToTrash',
	plus: 'openInNewTab',
	chip: 'openInNewTab',
	shelf: 'addToShelf',
};

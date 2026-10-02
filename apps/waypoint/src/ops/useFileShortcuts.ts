// The window keys for the file commands: F7, Shift+F7, F2, Delete, Shift+Delete, Ctrl+Shift+D, Ctrl+X, Ctrl+C, Ctrl+V, F5, Shift+F5, Ctrl+Z and Ctrl+Shift+Z
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef } from 'react';
import type { ListingSession } from '../browse/useListingSession';
import type { FileCommands } from './fileCommands';

/** What each key does; `useFileShortcuts` supplies the real ones and tests supply spies. */
export type FileKeyHandlers = Pick<
	FileCommands,
	| 'newFolder'
	| 'newFile'
	| 'rename'
	| 'duplicate'
	| 'moveToTrash'
	| 'deletePermanently'
	| 'cut'
	| 'copy'
	| 'paste'
	| 'copyToOtherPane'
	| 'moveToOtherPane'
	| 'undo'
	| 'redo'
>;

type KeyEventLike = Pick<
	KeyboardEvent,
	'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey' | 'isComposing'
>;

/**
 * F7 and Shift+F7 make a folder and a file, F2 renames the focused entry (Ctrl+F2 is batch
 * rename, which is not this hook's), Delete moves the selection to the Trash and Shift+Delete
 * deletes it permanently (after a confirmation), Ctrl+Shift+D duplicates, Ctrl+X, Ctrl+C and
 * Ctrl+V cut, copy and paste, F5 and Shift+F5 copy and move to the other pane (or ask where to,
 * without a pair), and Ctrl+Z and Ctrl+Shift+Z undo and redo. Returns whether the key was one of
 * these.
 */
export function handleFileKey(event: KeyEventLike, handlers: FileKeyHandlers): boolean {
	if (event.isComposing) return false;
	const modifier = event.ctrlKey || event.metaKey;
	const key = event.key.toLowerCase();
	if (!modifier && !event.altKey) {
		if (event.key === 'F7') {
			void (event.shiftKey ? handlers.newFile() : handlers.newFolder());
			return true;
		}
		if (event.key === 'F2' && !event.shiftKey) {
			handlers.rename();
			return true;
		}
		if (event.key === 'Delete') {
			void (event.shiftKey ? handlers.deletePermanently() : handlers.moveToTrash());
			return true;
		}
		if (event.key === 'F5') {
			void (event.shiftKey ? handlers.moveToOtherPane() : handlers.copyToOtherPane());
			return true;
		}
		return false;
	}
	if (modifier && !event.altKey) {
		if (key === 'd' && event.shiftKey) {
			void handlers.duplicate();
			return true;
		}
		if (key === 'z') {
			void (event.shiftKey ? handlers.redo() : handlers.undo());
			return true;
		}
		if (!event.shiftKey && (key === 'x' || key === 'c' || key === 'v')) {
			void (key === 'x' ? handlers.cut() : key === 'c' ? handlers.copy() : handlers.paste());
			return true;
		}
	}
	return false;
}

/** Whether keys typed at `target` belong to it: a text field, a menu or a dialog takes its own keys. */
export function ownsKeys(target: EventTarget | null): boolean {
	if (!(target instanceof Element)) return false;
	if (target.closest('input, textarea, select, [contenteditable]:not([contenteditable="false"])')) {
		return true;
	}
	return target.closest('dialog, [role="menu"], [role="dialog"]') !== null;
}

/**
 * Whether a key at `target` is in the file area: nothing in particular has focus, or focus is in a
 * pane. The sidebar, the tab strip and the path bar keep their own Delete and F2.
 */
export function inFileArea(target: EventTarget | null): boolean {
	if (!(target instanceof Element)) return true;
	if (target === document.body || target === document.documentElement) return true;
	return target.closest('[data-pane]') !== null;
}

/** Ctrl+Shift+R: Restore in the Trash. */
export function isRestoreKey(
	event: Pick<KeyEventLike, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>,
) {
	return (
		event.key.toLowerCase() === 'r' &&
		event.shiftKey &&
		(event.ctrlKey || event.metaKey) &&
		!event.altKey
	);
}

/** What the keys need to know about the pane they act on. */
export interface FileShortcutOptions {
	/** The pane the keys act on, read when a key is pressed. */
	activeSession?: () => ListingSession | null;
	/** Delete in the Trash: delete the selection for good (after asking). */
	deleteInTrash?: (session: ListingSession) => void;
	/** Ctrl+Shift+R in the Trash: restore the selection. */
	restoreInTrash?: (session: ListingSession) => void;
}

/**
 * Registers the file keys on the window. They are left alone where a field, a menu or a dialog
 * has focus (so a field's own Undo and Delete win), and the keys that act on the selection only
 * act in the file area. F7 and the history work wherever the browser has focus.
 */
export function useFileShortcuts(
	commands: FileCommands | null,
	options: FileShortcutOptions = {},
): void {
	const latest = useRef(commands);
	latest.current = commands;
	const opts = useRef(options);
	opts.current = options;

	useEffect(() => {
		const handlers: FileKeyHandlers = {
			newFolder: (session) => latest.current?.newFolder(session) ?? Promise.resolve(),
			newFile: (session) => latest.current?.newFile(session) ?? Promise.resolve(),
			rename: (session, entry) => latest.current?.rename(session, entry),
			duplicate: (session) => latest.current?.duplicate(session) ?? Promise.resolve(),
			moveToTrash: (session) => latest.current?.moveToTrash(session) ?? Promise.resolve(),
			deletePermanently: (session) =>
				latest.current?.deletePermanently(session) ?? Promise.resolve(),
			cut: (session) => latest.current?.cut(session) ?? Promise.resolve(),
			copy: (session) => latest.current?.copy(session) ?? Promise.resolve(),
			paste: (session, into) => latest.current?.paste(session, into) ?? Promise.resolve(),
			copyToOtherPane: (session) => latest.current?.copyToOtherPane(session) ?? Promise.resolve(),
			moveToOtherPane: (session) => latest.current?.moveToOtherPane(session) ?? Promise.resolve(),
			undo: () => latest.current?.undo() ?? Promise.resolve(),
			redo: () => latest.current?.redo() ?? Promise.resolve(),
		};
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || !latest.current || ownsKeys(event.target)) return;
			const clipboardKey =
				(event.ctrlKey || event.metaKey) &&
				!event.shiftKey &&
				['x', 'c', 'v'].includes(event.key.toLowerCase());
			const onSelection =
				event.key === 'F2' ||
				event.key === 'Delete' ||
				event.key === 'F5' ||
				clipboardKey ||
				(event.key.toLowerCase() === 'd' && event.shiftKey);
			if (onSelection && !inFileArea(event.target)) return;
			const session = opts.current.activeSession?.() ?? null;
			if (session?.model.layout === 'trash') {
				// The Trash is read only: Delete means Delete Permanently (with its question), and
				// New, Rename and Duplicate do nothing there. Undo and Redo are the history's.
				const plain = !event.ctrlKey && !event.metaKey && !event.altKey;
				if (event.key === 'Delete' && plain) {
					if (opts.current.deleteInTrash) {
						event.preventDefault();
						opts.current.deleteInTrash(session);
					}
					return;
				}
				if (isRestoreKey(event)) {
					if (opts.current.restoreInTrash && inFileArea(event.target)) {
						event.preventDefault();
						opts.current.restoreInTrash(session);
					}
					return;
				}
				const writes =
					event.key === 'F7' ||
					event.key === 'F5' ||
					clipboardKey ||
					(event.key === 'F2' && plain) ||
					(event.key.toLowerCase() === 'd' && event.shiftKey);
				if (writes) return;
			}
			if (handleFileKey(event, handlers)) event.preventDefault();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, []);
}

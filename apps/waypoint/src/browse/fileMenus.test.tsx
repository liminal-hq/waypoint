// Verifies the file commands in the entry and empty-space menus: their order, where they are hidden, and a key for every item
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expectEveryItemHasIcon } from '@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { commandStates, type CommandContext } from '../ops/fileCommands';
import { handleFileKey, type FileKeyHandlers } from '../ops/useFileShortcuts';
import {
	BackgroundContextMenu,
	backgroundMenuItems,
	type BackgroundCommands,
} from './BackgroundContextMenu';
import { EntryContextMenu, entryMenuItems } from './EntryContextMenu';

afterEach(cleanup);

const file = { id: 1, name: 'a.txt', kind: 'file', linkTarget: null } as unknown as Entry;
const folder = { id: 2, name: 'd', kind: 'directory', linkTarget: null } as unknown as Entry;

const base: CommandContext = {
	queue: true,
	listing: true,
	readOnly: false,
	selected: 1,
	focused: true,
	undo: null,
	redo: null,
};
const writable = commandStates(base);
const readOnly = commandStates({ ...base, readOnly: true });

/** The ids of the selectable rows, with `|` for each separator, so an order reads as one line. */
function shape(items: readonly MenuItem[]): string[] {
	return items.flatMap((item) =>
		item.type === 'separator'
			? ['|']
			: item.type === 'section'
				? []
				: item.type === 'submenu'
					? [`${item.id}[${shape(item.items).join(',')}]`]
					: [item.id],
	);
}

describe('the entry menu on an archive', () => {
	const archive = { id: 5, name: 'a.zip', kind: 'file', linkTarget: null } as unknown as Entry;
	const onArchive = commandStates({ ...base, archive: true });

	it('opens like a folder: Open, then Open in New Tab, Open in Split Pane and Open in New Window', () => {
		const ids = shape(entryMenuItems(archive, onArchive));
		expect(ids.slice(0, 5)).toEqual([
			'open',
			'openInNewTab',
			'openInSplit',
			'openInNewWindow',
			'|',
		]);
		expect(ids).not.toContain('openAsFolder');
	});

	it('offers Extract Here and Extract To… as a section of their own, and Compress…', () => {
		const ids = shape(entryMenuItems(archive, onArchive));
		const at = ids.indexOf('extractHere');
		expect(ids.slice(at - 1, at + 3)).toEqual(['|', 'extractHere', 'extractTo', '|']);
		expect(ids).toContain('compress');
		expectEveryItemHasIcon(entryMenuItems(archive, onArchive));
	});

	it('keeps Extract To… where it only reads, and drops Extract Here and Compress', () => {
		const reading = commandStates({ ...base, archive: true, readOnly: true });
		const ids = shape(entryMenuItems(archive, reading));
		expect(ids).toContain('extractTo');
		expect(ids).not.toContain('extractHere');
		expect(ids).not.toContain('compress');
	});

	it('offers none of it on a file that is not an archive, or on a folder named like one', () => {
		const folderZip = {
			id: 6,
			name: 'a.zip',
			kind: 'directory',
			linkTarget: null,
		} as unknown as Entry;
		for (const entry of [file, folderZip]) {
			const ids = shape(entryMenuItems(entry, writable));
			expect(ids).not.toContain('extractHere');
			expect(ids).not.toContain('extractTo');
		}
		expect(shape(entryMenuItems(file, writable))).not.toContain('openInNewTab');
	});

	it('runs Open in New Tab, Extract Here and Compress… on the entry', () => {
		const onOpenInNewTab = vi.fn();
		const onCommand = vi.fn();
		for (const name of [/Open in New Tab/, /Extract Here/, /Compress…/]) {
			render(
				<EntryContextMenu
					entry={archive}
					handle={7}
					position={{ x: 0, y: 0 }}
					keyboard={false}
					onClose={() => {}}
					onOpen={() => {}}
					onOpenInNewTab={onOpenInNewTab}
					onCopyPath={() => {}}
					onAddToFavourites={() => {}}
					commands={onArchive}
					onCommand={onCommand}
				/>,
			);
			fireEvent.click(screen.getByRole('menuitem', { name }));
			cleanup();
		}
		expect(onOpenInNewTab).toHaveBeenCalledWith(archive, 7);
		expect(onCommand.mock.calls.map((call) => call[0])).toEqual(['extractHere', 'compress']);
	});
});

describe('the entry menu', () => {
	it('keeps its current order and gains no write items without commands', () => {
		expect(shape(entryMenuItems(file))).toEqual([
			'open',
			'|',
			'addToShelf',
			'copyPath',
			'|',
			'properties',
		]);
		expect(shape(entryMenuItems(folder))).toEqual([
			'open',
			'openInNewTab',
			'openInSplit',
			'openInNewWindow',
			'|',
			'addToFavourites',
			'addToShelf',
			'copyPath',
			'|',
			'properties',
		]);
	});

	it('puts Cut, Copy and Paste ahead of Copy Path, Rename and Duplicate next, the transfers after them and the destructive items last, in docs/interactions.md order', () => {
		expect(shape(entryMenuItems(file, writable))).toEqual([
			'open',
			'|',
			'cut',
			'copy',
			'paste',
			'addToShelf',
			'copyPath',
			'|',
			'rename',
			'duplicate',
			'compress',
			'|',
			'copyTo',
			'moveTo',
			'|',
			'moveToTrash',
			'deletePermanently',
			'|',
			'properties',
		]);
	});

	it('says Paste Into Folder on a folder, so the files go where the menu says', () => {
		const ids = shape(entryMenuItems(folder, writable));
		expect(ids).toContain('pasteInto');
		expect(ids).not.toContain('paste');
		expect(ids.slice(0, 9)).toEqual([
			'open',
			'openInNewTab',
			'openInSplit',
			'openInNewWindow',
			'|',
			'cut',
			'copy',
			'pasteInto',
			'addToFavourites',
		]);
		const item = entryMenuItems(folder, writable).find(
			(candidate) => candidate.type === 'action' && candidate.id === 'pasteInto',
		);
		expect(item).toMatchObject({ label: 'Paste Into Folder' });
		expect(item).not.toHaveProperty('shortcut');
	});

	it('shows the clipboard keys and disables Paste while the clipboard is empty', () => {
		const items = entryMenuItems(file, writable).filter((item) => item.type === 'action');
		const byId = Object.fromEntries(items.map((item) => [item.id, item]));
		expect(byId.cut).toMatchObject({ label: 'Cut', shortcut: 'Ctrl+X' });
		expect(byId.copy).toMatchObject({ label: 'Copy', shortcut: 'Ctrl+C' });
		expect(byId.paste).toMatchObject({ label: 'Paste', shortcut: 'Ctrl+V', disabled: true });
		expect(byId.copyTo).toMatchObject({ label: 'Copy To…' });
		expect(byId.moveTo).toMatchObject({ label: 'Move To…' });
		const filled = commandStates({ ...base, clipboardItems: 2 });
		const paste = entryMenuItems(file, filled).find(
			(candidate) => candidate.type === 'action' && candidate.id === 'paste',
		);
		expect(paste).not.toMatchObject({ disabled: true });
	});

	it('offers Copy to Other Pane and Move to Other Pane only in a pair, with their keys, disabled when the other pane cannot be written to', () => {
		const pair = commandStates({ ...base, paired: true, otherPaneWritable: true });
		const items = entryMenuItems(file, pair).filter((item) => item.type === 'action');
		const byId = Object.fromEntries(items.map((item) => [item.id, item]));
		expect(byId.copyToOtherPane).toMatchObject({ label: 'Copy to Other Pane', shortcut: 'F5' });
		expect(byId.moveToOtherPane).toMatchObject({
			label: 'Move to Other Pane',
			shortcut: 'Shift+F5',
		});
		expect(shape(entryMenuItems(file, pair))).toContain('copyToOtherPane');
		expect(shape(entryMenuItems(file, writable))).not.toContain('copyToOtherPane');
		const locked = commandStates({ ...base, paired: true, otherPaneWritable: false });
		const lockedItems = entryMenuItems(file, locked).filter((item) => item.type === 'action');
		for (const id of ['copyToOtherPane', 'moveToOtherPane']) {
			expect(lockedItems.find((item) => item.id === id)).toMatchObject({ disabled: true });
		}
	});

	it('adds Rename Selected… after Rename only when more than one entry is selected', () => {
		expect(shape(entryMenuItems(file, writable))).not.toContain('batchRename');
		expect(shape(entryMenuItems(file, writable, true)).slice(5, 12)).toEqual([
			'addToShelf',
			'copyPath',
			'|',
			'rename',
			'batchRename',
			'duplicate',
			'compress',
		]);
		const item = entryMenuItems(file, writable, true).find(
			(candidate) => candidate.type === 'action' && candidate.id === 'batchRename',
		);
		expect(item).toMatchObject({ label: 'Rename Selected…', shortcut: 'Ctrl+F2' });
		expect(shape(entryMenuItems(file, readOnly, true))).not.toContain('batchRename');
		expectEveryItemHasIcon(entryMenuItems(file, writable, true));
	});

	it('shows the keys, and styles Move to Trash and Delete Permanently as dangerous', () => {
		const items = entryMenuItems(file, writable).filter((item) => item.type === 'action');
		const byId = Object.fromEntries(items.map((item) => [item.id, item]));
		expect(byId.rename).toMatchObject({ label: 'Rename', shortcut: 'F2' });
		expect(byId.duplicate).toMatchObject({ label: 'Duplicate', shortcut: 'Ctrl+Shift+D' });
		expect(byId.moveToTrash).toMatchObject({
			label: 'Move to Trash',
			shortcut: 'Delete',
			danger: true,
		});
		expect(byId.deletePermanently).toMatchObject({
			label: 'Delete Permanently',
			shortcut: 'Shift+Delete',
			danger: true,
		});
		expect(byId.rename).not.toHaveProperty('danger');
	});

	it('hides every write item in a read-only location but still copies from it', () => {
		expect(shape(entryMenuItems(file, readOnly))).toEqual([
			'open',
			'|',
			'copy',
			'addToShelf',
			'copyPath',
			'|',
			'copyTo',
			'|',
			'properties',
		]);
		const ids = shape(entryMenuItems(folder, readOnly));
		for (const hidden of ['cut', 'paste', 'pasteInto', 'moveTo', 'rename', 'moveToTrash']) {
			expect(ids).not.toContain(hidden);
		}
	});

	it('copies nowhere from the Trash, which has its own menu', () => {
		const trash = commandStates({ ...base, readOnly: true, trash: true });
		expect(shape(entryMenuItems(file, trash))).toEqual([
			'open',
			'|',
			'addToShelf',
			'copyPath',
			'|',
			'properties',
		]);
	});

	it('gives every item an icon', () => {
		expectEveryItemHasIcon(entryMenuItems(file, writable));
		expectEveryItemHasIcon(entryMenuItems(folder, writable));
	});

	it('runs the chosen command on the entry it was opened for', () => {
		const onCommand = vi.fn();
		render(
			<EntryContextMenu
				entry={file}
				handle={1}
				position={{ x: 0, y: 0 }}
				keyboard={false}
				onClose={() => {}}
				onOpen={() => {}}
				onOpenInNewTab={() => {}}
				onCopyPath={() => {}}
				onAddToFavourites={() => {}}
				commands={writable}
				onCommand={onCommand}
			/>,
		);
		fireEvent.click(screen.getByRole('menuitem', { name: /Move to Trash/ }));
		expect(onCommand).toHaveBeenCalledWith('moveToTrash', file);
	});

	it('disables the selection commands when nothing is selected', () => {
		const none = commandStates({ ...base, selected: 0, focused: false });
		const items = entryMenuItems(file, none).filter((item) => item.type === 'action');
		for (const id of ['rename', 'duplicate', 'moveToTrash', 'deletePermanently']) {
			expect(items.find((item) => item.id === id)).toMatchObject({ disabled: true });
		}
	});
});

describe('the empty-space menu', () => {
	const sort = { key: 'name', descending: false, directoriesFirst: true, groupBy: 'none' } as const;
	const commands = (overrides: Partial<BackgroundCommands> = {}): BackgroundCommands => ({
		states: writable,
		undoLabel: null,
		redoLabel: null,
		...overrides,
	});

	it('keeps its current order without commands', () => {
		expect(shape(backgroundMenuItems(sort, false))).toEqual([
			'sort:name',
			'sort:size',
			'sort:modified',
			'sort:kind',
			'|',
			'descending',
			'foldersFirst',
			'|',
			'group:none',
			'group:kind',
			'group:modified',
			'group:size',
			'group:name',
			'group:type',
			'|',
			'showHidden',
			'|',
			'properties',
		]);
	});

	it('puts New (a submenu of Folder and File) and Paste, then Undo and Redo, ahead of the view items', () => {
		const items = backgroundMenuItems(sort, false, { commands: commands() });
		expect(shape(items).slice(0, 8)).toEqual([
			'new[newFolder,newFile]',
			'paste',
			'|',
			'undo',
			'redo',
			'|',
			'sort:name',
			'sort:size',
		]);
		const submenu = items[0]!;
		expect(submenu).toMatchObject({ type: 'submenu', label: 'New' });
		const [folder, file] = (submenu as Extract<MenuItem, { type: 'submenu' }>).items;
		expect(folder).toMatchObject({ label: 'Folder', shortcut: 'F7' });
		expect(file).toMatchObject({ label: 'File', shortcut: 'Shift+F7' });
	});

	it('shows Paste with its key after New, disabled while the clipboard is empty', () => {
		const items = backgroundMenuItems(sort, false, { commands: commands() });
		const paste = items.find((item) => item.type === 'action' && item.id === 'paste');
		expect(paste).toMatchObject({ label: 'Paste', shortcut: 'Ctrl+V', disabled: true });
		const filled = backgroundMenuItems(sort, false, {
			commands: commands({ states: commandStates({ ...base, clipboardItems: 1 }) }),
		});
		expect(filled.find((item) => item.type === 'action' && item.id === 'paste')).not.toMatchObject({
			disabled: true,
		});
	});

	it('hides Paste in a read-only location', () => {
		const items = backgroundMenuItems(sort, false, { commands: commands({ states: readOnly }) });
		expect(shape(items)).not.toContain('paste');
	});

	it('labels Undo and Redo with what they would do, and disables them when there is nothing', () => {
		const labelled = backgroundMenuItems(sort, false, {
			commands: commands({
				states: commandStates({
					...base,
					undo: {
						id: 1,
						label: 'Move 3 items to Trash',
						atMs: 0,
						undoable: true,
						redoable: false,
						partlyUndone: false,
					},
				}),
				undoLabel: 'Move 3 items to Trash',
			}),
		});
		const undo = labelled.find((item) => item.type === 'action' && item.id === 'undo');
		expect(undo).toMatchObject({ label: 'Undo Move 3 items to Trash', shortcut: 'Ctrl+Z' });
		expect(undo).not.toMatchObject({ disabled: true });
		const redo = labelled.find((item) => item.type === 'action' && item.id === 'redo');
		expect(redo).toMatchObject({ label: 'Redo', shortcut: 'Ctrl+Shift+Z', disabled: true });
	});

	it('hides New in a read-only location but keeps the history', () => {
		const items = backgroundMenuItems(sort, false, { commands: commands({ states: readOnly }) });
		expect(shape(items).slice(0, 3)).toEqual(['undo', 'redo', '|']);
		expect(shape(items)).not.toContain('new[newFolder,newFile]');
	});

	it('offers only the Trash items in the Trash, even when the commands are passed too', () => {
		const items = backgroundMenuItems(sort, false, {
			trash: { count: 2 },
			commands: commands(),
		});
		const ids = shape(items).join(' ');
		expect(ids).toContain('emptyTrash');
		expect(ids).not.toMatch(/newFolder|undo|redo/);
		expect(shape(items).at(-1)).toBe('emptyTrash');
	});

	it('gives every item an icon', () => {
		expectEveryItemHasIcon(backgroundMenuItems(sort, true, { commands: commands() }));
		expectEveryItemHasIcon(
			backgroundMenuItems(undefined, false, { commands: commands({ states: readOnly }) }),
		);
	});

	it('runs New Folder from the submenu', () => {
		const onCommand = vi.fn();
		render(
			<BackgroundContextMenu
				session={null}
				showHidden={false}
				position={{ x: 0, y: 0 }}
				keyboard={false}
				onToggleHidden={() => {}}
				onClose={() => {}}
				commands={commands()}
				onCommand={onCommand}
			/>,
		);
		fireEvent.click(screen.getByRole('menuitem', { name: /^New/ }));
		fireEvent.click(screen.getByRole('menuitem', { name: /Folder/ }));
		expect(onCommand).toHaveBeenCalledWith('newFolder');
	});
});

describe('a key for every menu item', () => {
	// What each menu shortcut is typed as; a new write item without a row here fails the test.
	const KEYS: Record<string, KeyboardEventInit> = {
		F2: { key: 'F2' },
		'Ctrl+Shift+D': { key: 'D', ctrlKey: true, shiftKey: true },
		Delete: { key: 'Delete' },
		'Shift+Delete': { key: 'Delete', shiftKey: true },
		F7: { key: 'F7' },
		'Shift+F7': { key: 'F7', shiftKey: true },
		'Ctrl+Z': { key: 'z', ctrlKey: true },
		'Ctrl+Shift+Z': { key: 'Z', ctrlKey: true, shiftKey: true },
		'Ctrl+X': { key: 'x', ctrlKey: true },
		'Ctrl+C': { key: 'c', ctrlKey: true },
		'Ctrl+V': { key: 'v', ctrlKey: true },
		F5: { key: 'F5' },
		'Shift+F5': { key: 'F5', shiftKey: true },
	};
	const WRITE_IDS = new Set([
		'rename',
		'duplicate',
		'moveToTrash',
		'deletePermanently',
		'newFolder',
		'newFile',
		'undo',
		'redo',
		'cut',
		'copy',
		'paste',
		'copyToOtherPane',
		'moveToOtherPane',
	]);
	// Items whose command has no key of its own: they run from the menu (the dialog), and F5 asks
	// the same question when there is no pair.
	const MENU_ONLY = new Set(['copyTo', 'moveTo', 'pasteInto']);

	function actions(items: readonly MenuItem[]): MenuItem[] {
		return items.flatMap((item) => (item.type === 'submenu' ? actions(item.items) : [item]));
	}

	it('runs the same command from the key shown on the item', () => {
		const everything = commandStates({
			...base,
			paired: true,
			otherPaneWritable: true,
			clipboardItems: 1,
			undo: { id: 1, label: 'x', atMs: 0, undoable: true, redoable: false, partlyUndone: false },
			redo: { id: 1, label: 'x', atMs: 0, undoable: false, redoable: true, partlyUndone: false },
		});
		const items = actions([
			...entryMenuItems(file, everything),
			...backgroundMenuItems(undefined, false, {
				commands: { states: everything, undoLabel: null, redoLabel: null },
			}),
		]).filter((item) => item.type === 'action' && WRITE_IDS.has(item.id));
		expect([...new Set(items.map((item) => item.id))].sort()).toEqual([...WRITE_IDS].sort());
		for (const item of items) {
			if (item.type !== 'action') continue;
			const event = KEYS[item.shortcut ?? ''];
			expect(event, `${item.id} shows a key that is typed as something`).toBeDefined();
			const calls: string[] = [];
			const handlers = Object.fromEntries(
				[...WRITE_IDS].map((id) => [id, () => void calls.push(id)]),
			) as unknown as FileKeyHandlers;
			handleFileKey(
				{
					key: '',
					ctrlKey: false,
					metaKey: false,
					altKey: false,
					shiftKey: false,
					isComposing: false,
					...event,
				},
				handlers,
			);
			expect(calls, `${item.shortcut} runs ${item.id}`).toEqual([item.id]);
		}
	});

	it('keeps the items with no key of their own to the dialog and the folder menu, which have a menu path only', () => {
		const everything = commandStates({
			...base,
			paired: true,
			otherPaneWritable: true,
			clipboardItems: 1,
		});
		const items = actions([
			...entryMenuItems(file, everything),
			...entryMenuItems(folder, everything),
		]).filter((item) => item.type === 'action' && MENU_ONLY.has(item.id));
		expect([...new Set(items.map((item) => item.id))].sort()).toEqual([...MENU_ONLY].sort());
		for (const item of items) expect(item).not.toHaveProperty('shortcut');
	});
});

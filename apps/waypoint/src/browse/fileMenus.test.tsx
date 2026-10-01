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

describe('the entry menu', () => {
	it('keeps its current order and gains no write items without commands', () => {
		expect(shape(entryMenuItems(file))).toEqual(['open', '|', 'copyPath']);
		expect(shape(entryMenuItems(folder))).toEqual([
			'open',
			'openInNewTab',
			'openInNewWindow',
			'|',
			'addToFavourites',
			'copyPath',
		]);
	});

	it('puts Rename and Duplicate after Copy Path and the destructive items last, in docs/interactions.md order', () => {
		expect(shape(entryMenuItems(file, writable))).toEqual([
			'open',
			'|',
			'copyPath',
			'|',
			'rename',
			'duplicate',
			'|',
			'moveToTrash',
			'deletePermanently',
		]);
	});

	it('adds Rename Selected… after Rename only when more than one entry is selected', () => {
		expect(shape(entryMenuItems(file, writable))).not.toContain('batchRename');
		expect(shape(entryMenuItems(file, writable, true))).toEqual([
			'open',
			'|',
			'copyPath',
			'|',
			'rename',
			'batchRename',
			'duplicate',
			'|',
			'moveToTrash',
			'deletePermanently',
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

	it('hides every write item in a read-only location', () => {
		expect(shape(entryMenuItems(file, readOnly))).toEqual(['open', '|', 'copyPath']);
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
	const sort = { key: 'name', descending: false, directoriesFirst: true } as const;
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
			'showHidden',
		]);
	});

	it('puts New (a submenu of Folder and File), then Undo and Redo, ahead of the view items', () => {
		const items = backgroundMenuItems(sort, false, { commands: commands() });
		expect(shape(items).slice(0, 7)).toEqual([
			'new[newFolder,newFile]',
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

	it('labels Undo and Redo with what they would do, and disables them when there is nothing', () => {
		const labelled = backgroundMenuItems(sort, false, {
			commands: commands({
				states: commandStates({
					...base,
					undo: { id: 1, label: 'Move 3 items to Trash', atMs: 0, undoable: true, redoable: false },
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
	]);

	function actions(items: readonly MenuItem[]): MenuItem[] {
		return items.flatMap((item) => (item.type === 'submenu' ? actions(item.items) : [item]));
	}

	it('runs the same command from the key shown on the item', () => {
		const everything = commandStates({
			...base,
			undo: { id: 1, label: 'x', atMs: 0, undoable: true, redoable: false },
			redo: { id: 1, label: 'x', atMs: 0, undoable: false, redoable: true },
		});
		const items = actions([
			...entryMenuItems(file, everything),
			...backgroundMenuItems(undefined, false, {
				commands: { states: everything, undoLabel: null, redoLabel: null },
			}),
		]).filter((item) => item.type === 'action' && WRITE_IDS.has(item.id));
		expect(items.map((item) => item.id).sort()).toEqual([...WRITE_IDS].sort());
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
});

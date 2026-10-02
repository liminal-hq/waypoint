// Tests for the application menu's model: its menus, the shortcuts it shows, hiding and disabling, and the Undo History rules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expectEveryItemHasIcon } from '@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { describe, expect, it } from 'vitest';
import { t } from '../i18n/messages';
import { entry, factsFor } from '../test/commandFacts';
import type { CommandFacts } from './commandEnv';
import {
	APP_MENU_MNEMONICS,
	HISTORY_MORE_ID,
	appMenuItems,
	historyItems,
	historyRowId,
	historyTime,
	MENU_IDS,
	parseHistoryRow,
} from './appMenuModel';
import { commandDef, evaluateCommands } from './registry';

const NOW = new Date(2026, 9, 1, 18, 0).getTime();

const build = (facts: CommandFacts) => appMenuItems(evaluateCommands(facts), facts, NOW, 'en-CA');

const submenu = (items: MenuItem[], id: string) => {
	const found = items.find((item) => item.type === 'submenu' && item.id === id);
	if (found?.type !== 'submenu') throw new Error(`no ${id} menu`);
	return found;
};

const idsOf = (items: readonly MenuItem[]) =>
	items.flatMap((item) => (item.type === 'separator' || item.type === 'section' ? [] : [item.id]));

const rowOf = (items: readonly MenuItem[], id: string) => {
	const found = items.find(
		(item) => item.type !== 'separator' && item.type !== 'section' && item.id === id,
	);
	if (!found || found.type === 'separator' || found.type === 'section') throw new Error(`no ${id}`);
	return found;
};

const writable = factsFor({
	selected: 2,
	clipboardItems: 1,
	undo: entry(2, 'Move 3 items to Trash'),
});

describe('the menus', () => {
	it('are File, Edit, View and Window, and there is no Help (nothing for it to open)', () => {
		const items = build(writable);
		expect(items.map((item) => item.type === 'submenu' && item.label)).toEqual([
			t('appMenu.file'),
			t('appMenu.edit'),
			t('appMenu.view'),
			t('appMenu.window'),
		]);
		expect(items.some((item) => item.type === 'submenu' && /help/i.test(item.label))).toBe(false);
	});

	it('File: windows and tabs, new items, rename, trash and close', () => {
		const file = submenu(build(writable), MENU_IDS.file);
		expect(idsOf(file.items)).toEqual([
			'newWindow',
			'newTab',
			'newFolder',
			'newFile',
			'rename',
			'batchRename',
			'duplicate',
			'moveToTrash',
			'deletePermanently',
			'closeTab',
			'reopenClosedTab',
			'closeWindow',
		]);
	});

	it('Edit: Undo and Redo with the history, then the clipboard, copy and move, and the selection', () => {
		const edit = submenu(build(writable), MENU_IDS.edit);
		expect(idsOf(edit.items)).toEqual([
			'undo',
			'redo',
			MENU_IDS.history,
			'cut',
			'copy',
			'paste',
			'copyTo',
			'moveTo',
			'linkTo',
			'addToShelf',
			'selectAll',
			'invertSelection',
		]);
		expect(rowOf(edit.items, 'undo').label).toBe('Undo Move 3 items to Trash');
	});

	it('View: the two views, the toggles and Split View', () => {
		const view = submenu(build(writable), MENU_IDS.view);
		expect(idsOf(view.items)).toEqual([
			'viewList',
			'viewGrid',
			'showHidden',
			'sidebar',
			'toggleShelf',
			'actionBar',
			'splitView',
			'commandPalette',
		]);
	});

	it('Window: Duplicate Tab, Move Tab to New Window, then Settings; Always on Top only where supported', () => {
		const window = submenu(build(writable), MENU_IDS.window);
		expect(idsOf(window.items)).toEqual(['duplicateTab', 'moveTabToNewWindow', 'settings']);
		const supported = submenu(
			build(factsFor({}, { alwaysOnTop: { supported: true, on: false } })),
			MENU_IDS.window,
		);
		expect(idsOf(supported.items)).toEqual([
			'duplicateTab',
			'moveTabToNewWindow',
			'alwaysOnTop',
			'settings',
		]);
		expect(rowOf(supported.items, 'alwaysOnTop')).toMatchObject({
			type: 'checkbox',
			checked: false,
		});
	});

	it('shows checkboxes for the settings that are on', () => {
		const view = submenu(
			build(
				factsFor({}, { viewMode: 'grid', sidebarOpen: true, showHidden: true, actionBar: false }),
			),
			MENU_IDS.view,
		);
		expect(rowOf(view.items, 'viewGrid')).toMatchObject({ type: 'checkbox', checked: true });
		expect(rowOf(view.items, 'viewList')).toMatchObject({ type: 'checkbox', checked: false });
		expect(rowOf(view.items, 'sidebar')).toMatchObject({ checked: true });
		expect(rowOf(view.items, 'actionBar')).toMatchObject({ checked: false });
	});

	it('gives every row an icon, and the submenus too', () => {
		expectEveryItemHasIcon(build(writable));
		expectEveryItemHasIcon(build(factsFor({ readOnly: true, trash: true })));
	});

	it('shows exactly the shortcut the registry has, which the hooks bind (see the bindings test)', () => {
		const walk = (items: readonly MenuItem[]) => {
			for (const item of items) {
				if (item.type === 'submenu') walk(item.items);
				else if (
					(item.type === 'action' || item.type === 'checkbox') &&
					!item.id.startsWith('history:')
				) {
					expect(item.shortcut, item.id).toBe(commandDef(item.id as never).shortcut);
				}
			}
		};
		walk(build(writable));
	});

	it('shows every shortcut that is bound, so a reader can learn the keys from the menu', () => {
		const shown = new Set<string>();
		const walk = (items: readonly MenuItem[]) => {
			for (const item of items) {
				if (item.type === 'submenu') walk(item.items);
				else if ((item.type === 'action' || item.type === 'checkbox') && item.shortcut)
					shown.add(item.shortcut);
			}
		};
		walk(build(factsFor({ selected: 1, clipboardItems: 1, paired: true, undo: entry(1, 'x') })));
		for (const key of [
			'F7',
			'Shift+F7',
			'F2',
			'Ctrl+F2',
			'Ctrl+Shift+D',
			'Delete',
			'Shift+Delete',
		]) {
			expect(shown.has(key), key).toBe(true);
		}
		for (const key of ['Ctrl+Z', 'Ctrl+X', 'Ctrl+C', 'Ctrl+V', 'Ctrl+A', 'Ctrl+I']) {
			expect(shown.has(key), key).toBe(true);
		}
		for (const key of [
			'Ctrl+1',
			'Ctrl+2',
			'Ctrl+H',
			'F9',
			'F3',
			'Ctrl+,',
			'Ctrl+T',
			'Ctrl+W',
			'Ctrl+Shift+N',
		]) {
			expect(shown.has(key), key).toBe(true);
		}
	});

	it('has no stray, doubled or trailing separators, whatever is hidden', () => {
		const states = [
			writable,
			factsFor({ readOnly: true }),
			factsFor({ readOnly: true, trash: true }),
			factsFor({ queue: false }),
			factsFor({ listing: false }),
			factsFor({}, { tab: false }),
		];
		const check = (items: readonly MenuItem[]) => {
			expect(items.at(0)?.type).not.toBe('separator');
			expect(items.at(-1)?.type).not.toBe('separator');
			items.forEach((item, index) => {
				if (item.type === 'separator') expect(items[index + 1]?.type).not.toBe('separator');
				if (item.type === 'submenu') check(item.items);
			});
		};
		for (const facts of states) check(build(facts));
	});
});

describe('hiding and disabling', () => {
	it('hides what cannot work here and disables what needs something first, with the reason as the tooltip', () => {
		const items = build(factsFor({}));
		const file = submenu(items, MENU_IDS.file);
		expect(rowOf(file.items, 'newFolder').disabled).toBeUndefined();
		const rename = rowOf(file.items, 'rename');
		expect(rename.disabled).toBe(true);
		expect(rename.title).toBe(t('cmd.reason.nothingFocused'));
		expect(rowOf(file.items, 'moveToTrash')).toMatchObject({
			disabled: true,
			title: t('cmd.reason.nothingSelected'),
		});
		const edit = submenu(items, MENU_IDS.edit);
		expect(rowOf(edit.items, 'paste')).toMatchObject({
			disabled: true,
			title: t('cmd.reason.clipboardEmpty'),
		});
		expect(rowOf(edit.items, 'undo')).toMatchObject({ disabled: true, label: 'Undo' });
	});

	it('hides the write commands in a read-only folder and the Trash, keeping what only reads', () => {
		const readOnly = build(factsFor({ readOnly: true, selected: 1 }));
		const file = idsOf(submenu(readOnly, MENU_IDS.file).items);
		for (const id of [
			'newFolder',
			'newFile',
			'rename',
			'duplicate',
			'moveToTrash',
			'deletePermanently',
		]) {
			expect(file, id).not.toContain(id);
		}
		const edit = idsOf(submenu(readOnly, MENU_IDS.edit).items);
		expect(edit).toContain('copy');
		expect(edit).not.toContain('cut');
		const trash = build(factsFor({ readOnly: true, trash: true, selected: 1 }));
		expect(idsOf(submenu(trash, MENU_IDS.view).items)).not.toContain('showHidden');
	});

	it('leaves out a menu with nothing in it', () => {
		// A window with no queue and no listing: the window and tab menus remain.
		const items = build(factsFor({ queue: false, listing: false }));
		expect(items.length).toBeGreaterThan(0);
		for (const item of items)
			expect(item.type === 'submenu' && item.items.length).toBeGreaterThan(0);
	});
});

describe('the mnemonics', () => {
	it('name each top-level menu, once, with no letter for the Help menu that is not there', () => {
		const ids = build(writable).flatMap((item) => (item.type === 'submenu' ? [item.id] : []));
		expect(Object.values(APP_MENU_MNEMONICS).sort()).toEqual([...ids].sort());
		expect(Object.keys(APP_MENU_MNEMONICS)).toEqual(['f', 'e', 'v', 'w']);
		expect(APP_MENU_MNEMONICS.h).toBeUndefined();
	});
});

describe('Undo History', () => {
	const undoHead = entry(3, 'Move 3 items to Trash');
	const older = entry(2, 'New folder');
	const undone = entry(1, 'Rename report', { undoable: false, redoable: true });
	const facts = { history: [undoHead, older, undone], undoHead: 3, redoHead: 1 };

	it('says so when there is nothing in it yet', () => {
		const [only] = historyItems({ history: [], undoHead: null, redoHead: null }, NOW, 'en-CA');
		expect(only).toMatchObject({
			id: 'history:empty',
			disabled: true,
			label: t('appMenu.history.empty'),
		});
	});

	it('lists the entries newest first, each with when it happened, and marks an undone one', () => {
		const items = historyItems(facts, NOW, 'en-CA');
		expect(
			items.flatMap((item) =>
				item.type === 'action' && item.id !== HISTORY_MORE_ID ? [item.label] : [],
			),
		).toEqual([
			'Move 3 items to Trash — 2:30 p.m.',
			'New folder — 2:30 p.m.',
			'Rename report — 2:30 p.m. (undone)',
		]);
	});

	it('ends with a footer that opens the palette, where every entry can be chosen', () => {
		const items = historyItems(facts, NOW, 'en-CA');
		expect(items.at(-2)).toMatchObject({ type: 'separator' });
		expect(items.at(-1)).toMatchObject({
			type: 'action',
			id: HISTORY_MORE_ID,
			label: t('appMenu.history.more'),
		});
		expect(items.at(-1)).not.toHaveProperty('disabled');
		expect(historyItems({ history: [], undoHead: null, redoHead: null }, NOW)).toHaveLength(1);
	});

	it('enables only the entry Undo would undo and the entry Redo would redo, and explains the rest', () => {
		const items = historyItems(facts, NOW, 'en-CA');
		const rows = items.flatMap((item) => (item.type === 'action' ? [item] : []));
		expect(rows[0]).toMatchObject({ id: historyRowId('undo', 3) });
		expect(rows[0]?.disabled).toBeUndefined();
		expect(rows[1]).toMatchObject({
			id: 'history:later:2',
			disabled: true,
			title: t('appMenu.history.later'),
		});
		expect(rows[2]).toMatchObject({ id: historyRowId('redo', 1) });
		expect(rows[2]?.disabled).toBeUndefined();
	});

	it('reads a row id back into what to do', () => {
		expect(parseHistoryRow('history:undo:3')).toEqual({ kind: 'undo', entry: 3 });
		expect(parseHistoryRow('history:redo:12')).toEqual({ kind: 'redo', entry: 12 });
		expect(parseHistoryRow('history:later:2')).toBeNull();
		expect(parseHistoryRow('newFolder')).toBeNull();
	});

	it('hangs from Edit as a submenu with an icon, and only where the history is offered', () => {
		const edit = submenu(build(factsFor({ undo: undoHead })), MENU_IDS.edit);
		const history = submenu(edit.items, MENU_IDS.history);
		expect(history.label).toBe(t('appMenu.history'));
		expect(history.items.length).toBeGreaterThan(0);
		const noQueue = submenu(build(factsFor({ queue: false })), MENU_IDS.edit);
		expect(idsOf(noQueue.items)).not.toContain(MENU_IDS.history);
	});

	it('shows the time for today and the date with it for an older entry', () => {
		expect(historyTime(new Date(2026, 9, 1, 14, 30).getTime(), NOW, 'en-CA')).toBe('2:30 p.m.');
		expect(historyTime(new Date(2026, 8, 28, 14, 30).getTime(), NOW, 'en-CA')).toMatch(/Sep\.? 28/);
	});
});

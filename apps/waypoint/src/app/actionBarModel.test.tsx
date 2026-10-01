// Tests for the Action bar's buttons as data: which appear, what they run, and their words in each state
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expectEveryItemHasIcon } from '@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon';
import { describe, expect, it } from 'vitest';
import { t } from '../i18n/messages';
import { entry, factsFor } from '../test/commandFacts';
import { evaluateCommands, type CommandId, type CommandView } from '../commands/registry';
import type { CommandFacts } from '../commands/commandEnv';
import {
	actionBarItems,
	dividerBefore,
	overflowRows,
	tooltipFor,
	type ActionBarItem,
} from './actionBarModel';

const api = (facts: CommandFacts) => {
	const views = new Map(evaluateCommands(facts).map((view) => [view.id, view] as const));
	return { get: (id: CommandId): CommandView => views.get(id)! };
};
const items = (facts: CommandFacts) => actionBarItems(api(facts));
const ids = (list: ActionBarItem[]) => list.map((item) => item.id);
const byId = (list: ActionBarItem[], id: string) => list.find((item) => item.id === id)!;

describe('the buttons', () => {
	it('are New, Cut, Copy, Paste, Rename, Delete, Sort, View, Undo and Redo in a writable folder', () => {
		expect(ids(items(factsFor({ selected: 1 })))).toEqual([
			'new',
			'cut',
			'copy',
			'paste',
			'rename',
			'moveToTrash',
			'sort',
			'view',
			'undo',
			'redo',
		]);
	});

	it('draws a rule between the groups and nowhere else', () => {
		const list = items(factsFor({ selected: 1 }));
		const rules = list.flatMap((item, index) => (dividerBefore(list, index) ? [item.id] : []));
		expect(rules).toEqual(['cut', 'sort', 'undo']);
	});

	it('leaves out what is hidden: the Trash has no New, Cut, Copy, Paste, Rename or Delete', () => {
		const trash = items(factsFor({ readOnly: true, trash: true, selected: 1 }));
		expect(ids(trash)).toEqual(['sort', 'view', 'undo', 'redo']);
	});

	it('keeps Copy in a read-only folder, which only reads', () => {
		expect(ids(items(factsFor({ readOnly: true, selected: 1 })))).toEqual([
			'copy',
			'sort',
			'view',
			'undo',
			'redo',
		]);
	});

	it('has no file buttons, Sort or Undo without a queue and a listing', () => {
		expect(ids(items(factsFor({ queue: false, listing: false })))).toEqual(['view']);
	});

	it('disables each button by its command, with the reason', () => {
		const list = items(factsFor({}));
		for (const id of ['cut', 'copy', 'rename', 'moveToTrash']) {
			expect(byId(list, id).enabled, id).toBe(false);
			expect(byId(list, id).reason, id).toBeTruthy();
		}
		expect(byId(list, 'paste').reason).toBe(t('cmd.reason.clipboardEmpty'));
		expect(byId(list, 'undo').reason).toBe(t('cmd.reason.nothingToUndo'));
		expect(byId(list, 'new').enabled).toBe(true);
		expect(byId(list, 'sort').enabled).toBe(true);
		expect(byId(list, 'view').enabled).toBe(true);
	});

	it('enables them with a selection, a full clipboard and a history', () => {
		const list = items(
			factsFor({
				selected: 2,
				clipboardItems: 1,
				undo: entry(2, 'x'),
				redo: entry(1, 'y', { undoable: false, redoable: true }),
			}),
		);
		for (const item of list) expect(item.enabled, item.id).toBe(true);
	});

	it('runs the registry command of each, and Delete moves to the Trash', () => {
		const list = items(factsFor({ selected: 1 }));
		expect(list.filter((item) => item.command).map((item) => item.command)).toEqual([
			'cut',
			'copy',
			'paste',
			'rename',
			'moveToTrash',
			'viewGrid',
			'undo',
			'redo',
		]);
		expect(byId(list, 'moveToTrash').label).toBe(t('actionBar.delete'));
		expect(byId(list, 'moveToTrash').tooltip).toBe('Move to Trash (Delete)');
	});

	it('gives every button an icon', () => {
		for (const item of items(factsFor({ selected: 1 }))) expect(item.icon, item.id).toBeDefined();
	});
});

describe('the menu buttons', () => {
	it('New opens Folder and File', () => {
		const list = items(factsFor({}));
		const menu = byId(list, 'new').menu!;
		expect(menu.map((row) => row.type !== 'separator' && row.type !== 'section' && row.id)).toEqual(
			['newFolder', 'newFile'],
		);
		expectEveryItemHasIcon(menu);
	});

	it('Sort opens the sort keys, then Descending and Folders first, with the current ones checked', () => {
		const menu = byId(items(factsFor({})), 'sort').menu!;
		const rows = menu.flatMap((row) => (row.type === 'checkbox' ? [row] : []));
		expect(rows.map((row) => row.id)).toEqual([
			'sortName',
			'sortSize',
			'sortModified',
			'sortKind',
			'sortDescending',
			'sortFoldersFirst',
		]);
		expect(rows.filter((row) => row.checked).map((row) => row.id)).toEqual([
			'sortName',
			'sortFoldersFirst',
		]);
		expect(menu.filter((row) => row.type === 'separator')).toHaveLength(1);
	});

	it('Sort in the Trash offers Date deleted and not Modified or Kind', () => {
		const menu = byId(items(factsFor({ readOnly: true, trash: true })), 'sort').menu!;
		const keys = menu.flatMap((row) => (row.type === 'checkbox' ? [row.id] : []));
		expect(keys).toContain('sortDeleted');
		expect(keys).not.toContain('sortModified');
		expect(keys).not.toContain('sortKind');
	});
});

describe('the View button', () => {
	it('switches to Grid from the list and says so, with the key', () => {
		const view = byId(items(factsFor({}, { viewMode: 'list' })), 'view');
		expect(view.command).toBe('viewGrid');
		expect(view.tooltip).toBe('View: switch to Grid (Ctrl+1)');
	});

	it('switches back to List from the grid', () => {
		const view = byId(items(factsFor({}, { viewMode: 'grid' })), 'view');
		expect(view.command).toBe('viewList');
		expect(view.tooltip).toBe('View: switch to List (Ctrl+2)');
	});
});

describe('tooltips', () => {
	it('say the name and the key, or why a button is disabled', () => {
		expect(tooltipFor('Cut', 'Ctrl+X', undefined)).toBe('Cut (Ctrl+X)');
		expect(tooltipFor('New', undefined, undefined)).toBe('New');
		expect(tooltipFor('Cut', 'Ctrl+X', 'Select something first')).toBe(
			'Cut — Select something first',
		);
		const cut = byId(items(factsFor({})), 'cut');
		expect(cut.tooltip).toBe(`Cut — ${t('cmd.reason.nothingSelected')}`);
		expect(byId(items(factsFor({ selected: 1 })), 'cut').tooltip).toBe('Cut (Ctrl+X)');
	});

	it('name the next change on Undo and Redo through the registry label', () => {
		const list = items(factsFor({ undo: entry(2, 'Move 3 items to Trash') }));
		expect(byId(list, 'undo').tooltip).toBe('Undo (Ctrl+Z)');
	});
});

describe('the More menu', () => {
	it('lists the buttons that did not fit: menu buttons as submenus, the rest as rows', () => {
		const list = items(factsFor({ selected: 1 }));
		const rows = overflowRows(list.filter((item) => ['sort', 'view', 'undo'].includes(item.id)));
		expect(rows.map((row) => row.type)).toEqual(['submenu', 'action', 'action']);
		expect(rows.map((row) => row.type !== 'separator' && row.type !== 'section' && row.id)).toEqual(
			['bar:sort', 'viewGrid', 'undo'],
		);
		expectEveryItemHasIcon(rows);
	});

	it('keeps a disabled button disabled, with its reason', () => {
		const list = items(factsFor({}));
		const [row] = overflowRows([byId(list, 'undo')]);
		expect(row).toMatchObject({ disabled: true, title: t('cmd.reason.nothingToUndo') });
	});
});

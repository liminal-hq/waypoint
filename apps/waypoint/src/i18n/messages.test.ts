// Verifies the catalogue is complete, feeds the chrome's labels, and that screens hold no literal copy
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defaultChromeLabels } from '@liminal-hq/waypoint-chrome/labels';
import { describe, expect, it } from 'vitest';
import { chromeLabels } from './chromeLabels';
import { pluralText, policyLabel } from '../ops/resolveText';
import { enMessages, t, tf, tn } from './messages';

const screens = import.meta.glob<string>(
	[
		'../app/*.tsx',
		'../browse/*.tsx',
		'../dnd/*.tsx',
		'../nav/*.tsx',
		'../ops/*.tsx',
		'../settings/*.tsx',
		'../tabs/*.tsx',
		'../status/*.tsx',
		'../sidebar/*.tsx',
		'../trash/*.tsx',
	],
	{
		query: '?raw',
		import: 'default',
		eager: true,
	},
);

describe('the message catalogue', () => {
	it('has no empty messages', () => {
		for (const [id, message] of Object.entries(enMessages)) {
			expect(message.trim(), id).not.toBe('');
		}
	});

	it('looks a message up by identifier', () => {
		expect(t('window.main.title')).toBe(enMessages['window.main.title']);
	});
});

describe('tf', () => {
	it('fills each token and leaves an unknown one as written', () => {
		expect(tf('browse.capped', { shown: '10', total: '20' })).toBe(
			'Showing the first 10 of 20 items',
		);
		expect(tf('browse.capped', { shown: '10' })).toBe('Showing the first 10 of {total} items');
	});
});

describe('tn', () => {
	it('chooses the plural form and formats the number for the locale', () => {
		expect(tn('browse.selection', 1, 'en-CA')).toBe('1 item selected');
		expect(tn('browse.selection', 3, 'en-CA')).toBe('3 items selected');
		expect(tn('browse.selection', 1200, 'en-CA')).toBe('1,200 items selected');
		expect(tn('browse.selection', 0, 'en-CA')).toBe('0 items selected');
	});
});

describe('the Trash messages', () => {
	it('have a one and an other form for everything counted', () => {
		expect(tn('trash.confirm.delete.message', 1, 'en-CA')).toBe(
			'1 item will be deleted permanently. This cannot be undone.',
		);
		expect(tn('trash.confirm.delete.message', 1200, 'en-CA')).toBe(
			'1,200 items will be deleted permanently. This cannot be undone.',
		);
		expect(tn('trash.confirm.empty.message', 1, 'en-CA')).toBe(
			'The 1 item in the Trash will be deleted permanently. This cannot be undone.',
		);
		expect(tn('trash.confirm.empty.message', 3, 'en-CA')).toBe(
			'All 3 items in the Trash will be deleted permanently. This cannot be undone.',
		);
		expect(tn('trash.done.restored', 1, 'en-CA')).toBe('Restored 1 item');
		expect(tn('trash.done.deleted', 2, 'en-CA')).toBe('Deleted 2 items permanently');
		expect(tn('trash.done.skipped', 1, 'en-CA')).toBe('1 item was left in the Trash');
		expect(tn('sidebar.trash.count', 4, 'en-CA')).toBe('4 items in the Trash');
	});

	it('fill in the names they mention', () => {
		expect(tf('trash.failed.restore', { reason: 'no room' })).toBe('Could not restore: no room');
	});

	it('keep the Trash’s words consistent where they appear in more than one place', () => {
		expect(t('menu.emptyTrash')).toBe(t('trash.empty'));
		expect(t('menu.deletePermanently')).toBe(t('trash.delete'));
		expect(t('menu.restore')).toBe(t('trash.restore'));
		expect(t('browse.column.deleted')).toBe(t('menu.sort.deleted'));
	});
});

describe('the conflict resolver messages', () => {
	it('have a one and an other form for what is counted', () => {
		expect(pluralText('ops.conflict.title', 1, { destination: '/dest' }, 'en-CA')).toBe(
			'1 item already exists in /dest',
		);
		expect(pluralText('ops.conflict.title', 1200, { destination: '/dest' }, 'en-CA')).toBe(
			'1,200 items already exist in /dest',
		);
		expect(pluralText('ops.conflict.title.restore', 3, {}, 'en-CA')).toBe(
			'3 items already exist in their original folders',
		);
		expect(pluralText('ops.conflict.more', 30, {}, 'en-CA')).toBe('and 30 more');
	});

	it('name every choice the engine knows', () => {
		expect(
			(['replace', 'skip', 'keepBoth', 'mergeFolders', 'replaceIfNewer'] as const).map(policyLabel),
		).toEqual(['Replace', 'Skip', 'Keep both', 'Merge folders', 'Replace if newer']);
	});

	it('agree with the Cancel the operation wording wherever it appears', () => {
		expect(t('ops.conflict.cancel')).toBe(t('ops.problem.cancel'));
		expect(t('ops.conflict.cancelConfirm.confirm')).toBe(t('ops.conflict.cancel'));
	});
});

describe('the clipboard and pane operation messages', () => {
	it('have a one and an other form for everything counted', () => {
		expect(tn('files.copied', 1, 'en-CA')).toBe('Copied 1 item');
		expect(tn('files.copied', 3, 'en-CA')).toBe('Copied 3 items');
		expect(tn('files.cut', 1, 'en-CA')).toBe('Cut 1 item');
		expect(tn('files.cut', 1200, 'en-CA')).toBe('Cut 1,200 items');
		expect(tn('destination.title.copy', 1, 'en-CA')).toBe('Copy 1 item to…');
		expect(tn('destination.title.copy', 2, 'en-CA')).toBe('Copy 2 items to…');
		expect(tn('destination.title.move', 1, 'en-CA')).toBe('Move 1 item to…');
		expect(tn('destination.title.move', 5, 'en-CA')).toBe('Move 5 items to…');
	});

	it('name the folder or the reason in the sentences that need one', () => {
		expect(tf('destination.check.ok', { name: 'docs' })).toBe('Ready: docs can be written to.');
		expect(tf('destination.check.notFound', { name: 'docs' })).toBe('“docs” does not exist.');
		expect(tf('destination.check.notFolder', { name: 'a.txt' })).toBe('“a.txt” is not a folder.');
		expect(tf('destination.check.readOnly', { name: 'docs' })).toBe('“docs” cannot be written to.');
		expect(tf('destination.check.invalid', { input: '??' })).toBe('“??” is not a location.');
		expect(tf('destination.newFolder.failed', { reason: 'no room' })).toBe(
			'Could not make the folder: no room',
		);
		expect(tf('destination.newFolder.made', { name: 'untitled folder' })).toBe(
			'Made “untitled folder” and chose it.',
		);
	});

	it('use the same words in the menus and the dialog, and the keys’ names in the menus', () => {
		expect(t('destination.copy')).toBe(t('menu.copy'));
		expect(t('menu.copyTo')).toBe('Copy To…');
		expect(t('menu.moveTo')).toBe('Move To…');
		expect(t('menu.pasteInto')).toBe('Paste Into Folder');
		expect(t('menu.copyToOtherPane')).toBe('Copy to Other Pane');
		expect(t('menu.moveToOtherPane')).toBe('Move to Other Pane');
		expect(t('destination.move')).toBe('Move');
	});

	it('say the same thing for the planner’s refusals as the dialog does for a move into the same folder', () => {
		expect(t('destination.check.sameFolder')).toBe(`${t('ops.error.sameFolder')}.`);
	});
});

describe('chromeLabels', () => {
	it('provides every label the chrome uses, from the catalogue', () => {
		const labels = chromeLabels();
		expect(Object.keys(labels).sort()).toEqual(Object.keys(defaultChromeLabels).sort());
		expect(labels.systemWindowMenu).toBe('More options…');
	});
});

describe('the application menu and Action bar messages', () => {
	it('fill in the history rows and the Action bar tooltips', () => {
		expect(tf('appMenu.history.entry', { label: 'New folder', time: '2:30 p.m.' })).toBe(
			'New folder — 2:30 p.m.',
		);
		expect(tf('appMenu.history.undone', { label: 'New folder', time: '2:30 p.m.' })).toBe(
			'New folder — 2:30 p.m. (undone)',
		);
		expect(tf('actionBar.withShortcut', { name: 'Cut', keys: 'Ctrl+X' })).toBe('Cut (Ctrl+X)');
		expect(tf('actionBar.disabledBecause', { name: 'Cut', reason: 'Select something first' })).toBe(
			'Cut — Select something first',
		);
		expect(tf('actionBar.viewTo', { name: 'Grid' })).toBe('View: switch to Grid');
	});

	it('give every disabled command a reason that reads as a sentence fragment, with no stop', () => {
		const reasons = Object.entries(enMessages).filter(([id]) => id.startsWith('cmd.reason.'));
		expect(reasons.length).toBeGreaterThanOrEqual(8);
		for (const [id, message] of reasons) expect(message, id).not.toMatch(/[.!?]$/);
	});

	it('name the menus the way the mnemonics expect', () => {
		expect(t('appMenu.file')).toBe('File');
		expect(t('appMenu.edit')).toBe('Edit');
		expect(t('appMenu.view')).toBe('View');
		expect(t('appMenu.window')).toBe('Window');
	});

	it('use the same words as the other menus for the commands they share', () => {
		expect(t('menu.moveToTrash')).toBe('Move to Trash');
		expect(t('actionBar.delete')).toBe('Delete');
		expect(t('actionBar.hideLabels')).toBe('Hide Labels');
		expect(t('actionBar.hide')).toBe('Hide Action Bar');
	});
});

describe('screens', () => {
	it('finds the screen sources', () => {
		expect(Object.keys(screens).length).toBeGreaterThanOrEqual(6);
	});

	it('hold no literal title, description or label copy', () => {
		const literal =
			/\b(?:title|description|label|aria-label|placeholder)=(?:"[^"]*[A-Za-z][^"]*"|'[^']*[A-Za-z][^']*')/;
		for (const [file, source] of Object.entries(screens)) {
			if (file.includes('.test.')) continue;
			expect(source, file).not.toMatch(literal);
		}
	});
});

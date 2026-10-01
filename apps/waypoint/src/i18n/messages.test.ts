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

describe('chromeLabels', () => {
	it('provides every label the chrome uses, from the catalogue', () => {
		const labels = chromeLabels();
		expect(Object.keys(labels).sort()).toEqual(Object.keys(defaultChromeLabels).sort());
		expect(labels.systemWindowMenu).toBe('More options…');
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

// Verifies the catalogue is complete, feeds the chrome's labels, and that screens hold no literal copy
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defaultChromeLabels } from '@liminal-hq/waypoint-chrome/labels';
import { describe, expect, it } from 'vitest';
import { chromeLabels } from './chromeLabels';
import { enMessages, t, tf, tn } from './messages';

const screens = import.meta.glob<string>(
	[
		'../app/*.tsx',
		'../browse/*.tsx',
		'../nav/*.tsx',
		'../ops/*.tsx',
		'../tabs/*.tsx',
		'../status/*.tsx',
		'../sidebar/*.tsx',
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

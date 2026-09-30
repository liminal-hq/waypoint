// Verifies the catalogue is complete, feeds the chrome's labels, and that screens hold no literal copy
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defaultChromeLabels } from '@liminal-hq/waypoint-chrome/labels';
import { describe, expect, it } from 'vitest';
import { chromeLabels } from './chromeLabels';
import { enMessages, t } from './messages';

const screens = import.meta.glob<string>('../app/*.tsx', {
	query: '?raw',
	import: 'default',
	eager: true,
});

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

// Checks that the batch rename screens hold no literal copy and that its catalogue entries are whole
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { enMessages } from '../../i18n/messages';

const sources = import.meta.glob<string>('./*.tsx', {
	query: '?raw',
	import: 'default',
	eager: true,
});

describe('the batch rename messages', () => {
	const own = Object.entries(enMessages).filter(([id]) => id.startsWith('batchRename.'));

	it('are in the catalogue, none empty', () => {
		expect(own.length).toBeGreaterThan(80);
		for (const [id, message] of own) expect(message.trim(), id).not.toBe('');
	});

	it('give every counted message both of its forms', () => {
		for (const [id] of own) {
			const base = id.replace(/\.(one|other)$/, '');
			if (base === id) continue;
			expect(enMessages, base).toHaveProperty([`${base}.one`]);
			expect(enMessages, base).toHaveProperty([`${base}.other`]);
		}
	});

	it('use only tokens that a caller fills in', () => {
		const known = new Set(['count', 'number', 'reason', 'other', 'shown', 'total']);
		for (const [id, message] of own) {
			for (const [, token] of message.matchAll(/\{([A-Za-z]\w*)\}/g)) {
				expect(known.has(token!), `${id} uses {${token}}`).toBe(true);
			}
		}
	});
});

describe('the batch rename screens', () => {
	it('are found', () => {
		expect(Object.keys(sources).length).toBeGreaterThanOrEqual(3);
	});

	it('hold no literal title, description or label copy', () => {
		const literal =
			/\b(?:title|description|label|aria-label|placeholder)=(?:"[^"]*[A-Za-z][^"]*"|'[^']*[A-Za-z][^']*')/;
		for (const [file, source] of Object.entries(sources)) {
			if (file.includes('.test.')) continue;
			expect(source, file).not.toMatch(literal);
		}
	});
});

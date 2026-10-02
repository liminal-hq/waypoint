// Tests the pseudo-locale transforms: accents, expansion, right-to-left marks and untouched tokens
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { accentMessage, EXPANSION, pseudoCatalogue, rightToLeftMessage } from './pseudo';

describe('accentMessage', () => {
	it('accents the letters, lengthens by about 30% and brackets the result', () => {
		const out = accentMessage('Open the folder');
		expect(out.startsWith('[')).toBe(true);
		expect(out.endsWith(']')).toBe(true);
		expect(out).toContain('Öþéñ ţĥé ƒöĺďéŕ');
		expect(out.length).toBeGreaterThanOrEqual(
			Math.ceil('Open the folder'.length * (1 + EXPANSION)),
		);
		expect(out).not.toMatch(/[a-zA-Z]/);
	});

	it('leaves {name} tokens exactly as written, so interpolation still works', () => {
		const out = accentMessage('Moved {count} items to {name}');
		expect(out).toContain('{count}');
		expect(out).toContain('{name}');
		expect(out.replace(/\{\w+\}/g, '')).not.toMatch(/[a-zA-Z]/);
	});

	it('does not count a token when it works out the padding', () => {
		expect(accentMessage('{name}')).toBe('[{name}]');
	});

	it('keeps digits, punctuation and symbols', () => {
		expect(accentMessage('100%')).toContain('100%');
	});
});

describe('rightToLeftMessage', () => {
	it('wraps the text in a right-to-left embedding', () => {
		const out = rightToLeftMessage('Hello');
		expect(out.startsWith('‫')).toBe(true);
		expect(out.endsWith('‬')).toBe(true);
		expect(out).toContain('Hello');
	});

	it('uses the Arabic question mark, comma and semicolon', () => {
		expect(rightToLeftMessage('a, b; c?')).toContain('a، b؛ c؟');
	});

	it('marks each token and leaves it intact', () => {
		const out = rightToLeftMessage('Copied {count} items');
		expect(out).toContain('‏{count}‏');
	});
});

describe('pseudoCatalogue', () => {
	it('keeps every key and transforms every value', () => {
		const out = pseudoCatalogue(
			{ 'tabs.count.one': '{count} tab', 'tabs.count.other': '{count} tabs' },
			(message) => `<${message}>`,
		);
		expect(out).toEqual({
			'tabs.count.one': '<{count} tab>',
			'tabs.count.other': '<{count} tabs>',
		});
	});
});

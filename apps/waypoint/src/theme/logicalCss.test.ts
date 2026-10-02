// Guards the logical-property rule: the scanner's judgement, and that no tracked stylesheet breaks it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { physicalReason, scanCss, scanTsx } from '../../../../scripts/logicalCss';

const root = `${join(import.meta.dirname, '../../../..')}/`;
const css = (body: string): number[] => scanCss(body).map((offence) => offence.line);

describe('the logical-property scanner', () => {
	it('flags physical sides and offsets', () => {
		expect(
			css('a {\n\tmargin-left: 4px;\n\tpadding-right: 2px;\n\tborder-left: 1px solid red;\n}'),
		).toEqual([2, 3, 4]);
		expect(css('a {\n\tleft: 0;\n\tright: 1px;\n\ttext-align: left;\n\tfloat: right;\n}')).toEqual([
			2, 3, 4, 5,
		]);
		expect(
			css('a {\n\tborder-top-left-radius: 2px;\n\tbackground: linear-gradient(to right, a, b);\n}'),
		).toEqual([2, 3]);
	});

	it('accepts the logical forms', () => {
		const ok =
			'a {\n\tmargin-inline-start: 4px;\n\tinset-inline-end: 0;\n\ttext-align: start;\n\tborder-start-start-radius: 2px;\n\ttop: 0;\n\tpadding: 2px 4px;\n}';
		expect(css(ok)).toEqual([]);
	});

	it('flags a four-value shorthand only when its sides differ', () => {
		expect(physicalReason('padding', '1px 2px 3px 4px')).not.toBeNull();
		expect(physicalReason('padding', '1px 2px 3px 2px')).toBeNull();
		expect(physicalReason('inset', '0 50% 0 0')).not.toBeNull();
		expect(physicalReason('margin', '0 auto')).toBeNull();
	});

	it('flags a border radius that is not mirror-symmetric', () => {
		expect(physicalReason('border-radius', '8px 8px 0 0')).toBeNull();
		expect(physicalReason('border-radius', '8px 0 0 8px')).not.toBeNull();
		expect(physicalReason('border-radius', '4px')).toBeNull();
		expect(physicalReason('border-radius', '4px 0')).not.toBeNull();
	});

	it('allows a line marked physical, on the line or the one above', () => {
		expect(css('a {\n\tleft: 0; /* physical: pointer */\n}')).toEqual([]);
		expect(css('a {\n\t/* physical: pointer */\n\tleft: 0;\n}')).toEqual([]);
		expect(css('a {\n\t/* physical: pointer */\n\tleft: 0;\n\tright: 0;\n}')).toEqual([4]);
	});

	it('flags physical inline styles in components', () => {
		expect(scanTsx('<i style={{ left: 4 }} />').length).toBe(1);
		expect(scanTsx('<i style={{ marginLeft: 4 }} />').length).toBe(1);
		expect(scanTsx('<i style={{ marginInlineStart: 4 }} />').length).toBe(0);
		expect(scanTsx('<i style={{ left: 4 }} /> // physical: measured').length).toBe(0);
	});
});

describe('the app', () => {
	it('has no physical direction property outside the marked places', () => {
		const files = execFileSync(
			'git',
			[
				'ls-files',
				'--',
				'apps/**/*.css',
				'apps/**/*.tsx',
				'packages/**/*.css',
				'packages/**/*.tsx',
			],
			{
				encoding: 'utf8',
				cwd: root,
			},
		)
			.split('\n')
			.filter((file) => file && !/\.test\.tsx?$/.test(file));
		expect(files.length).toBeGreaterThan(50);
		const offences = files.flatMap((file) => {
			const source = readFileSync(`${root}${file}`, 'utf8');
			const found = file.endsWith('.css') ? scanCss(source) : scanTsx(source);
			return found.map((offence) => `${file}:${offence.line}: ${offence.why}`);
		});
		expect(offences).toEqual([]);
	});
});

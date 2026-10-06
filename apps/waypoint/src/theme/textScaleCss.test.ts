// Guards the text scale: every font size follows `--wp-text-scale` once, so no text grows more than the rest
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = `${join(import.meta.dirname, '../../../..')}/`;
const files = execFileSync('git', ['ls-files', '--', 'apps/**/*.css', 'packages/**/*.css'], {
	cwd: root,
	encoding: 'utf8',
})
	.split('\n')
	.filter(Boolean);

/** Every `font-size` value in a stylesheet (comments removed), with its line. */
function fontSizes(source: string): { line: number; value: string }[] {
	const plain = source.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, ' '));
	return [...plain.matchAll(/font-size:\s*([^;}]+)[;}]/g)].map((match) => ({
		line: plain.slice(0, match.index).split('\n').length,
		value: match[1]!.trim(),
	}));
}

describe('the text scale in stylesheets', () => {
	it('finds the font sizes', () => {
		expect(files.length).toBeGreaterThan(20);
	});

	it('scales every fixed font size by --wp-text-scale, once', () => {
		const offences: string[] = [];
		for (const file of files) {
			for (const { line, value } of fontSizes(readFileSync(join(root, file), 'utf8'))) {
				// `inherit` takes the already scaled size of the parent: scaling it again would apply the factor twice.
				if (value === 'inherit') continue;
				const scaled = /^calc\([0-9.]+px \* var\(--wp-text-scale\)\)$/.test(value);
				if (!scaled) offences.push(`${file}:${line}: font-size: ${value}`);
			}
		}
		expect(offences).toEqual([]);
	});
});

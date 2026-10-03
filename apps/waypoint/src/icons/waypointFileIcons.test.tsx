// Verifies the Waypoint set draws every icon group and every standard folder the protocol names
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { FileIcon } from '../browse/FileIcon';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import { WAYPOINT_FILE_GLYPHS, WAYPOINT_FOLDER_GLYPHS } from './waypointFileIcons';

/** The string literals of a generated union type, read from the file Rust generates. */
function literals(name: string): string[] {
	const file = join(
		import.meta.dirname,
		'../../../../packages/protocol/src/generated',
		`${name}.ts`,
	);
	const text = readFileSync(file, 'utf8');
	const union = text.slice(text.indexOf('export type'));
	return [...union.matchAll(/"([A-Za-z0-9]+)"/g)].map((match) => match[1]!);
}

const SHAPES = /<(path|rect|circle|ellipse)\b/;

describe('the Waypoint file icons', () => {
	it('has a glyph for every icon group Rust can send', () => {
		const groups = literals('IconGroup');
		expect(groups.length).toBeGreaterThanOrEqual(31);
		expect(Object.keys(WAYPOINT_FILE_GLYPHS).sort()).toEqual([...groups].sort());
		for (const group of groups as IconGroup[]) {
			expect(renderToStaticMarkup(<FileIcon group={group} />), group).toMatch(SHAPES);
		}
	});

	it('has a marked folder glyph for every standard folder', () => {
		const folders = literals('SpecialFolder');
		expect(folders).toHaveLength(10);
		expect(Object.keys(WAYPOINT_FOLDER_GLYPHS).sort()).toEqual([...folders].sort());
		for (const special of folders as SpecialFolder[]) {
			const markup = renderToStaticMarkup(<FileIcon group="folder" special={special} />);
			expect(markup, special).toContain(`data-special="${special}"`);
			expect(markup, special).toMatch(SHAPES);
		}
	});

	it('draws a standard folder as the folder shape plus a mark, and only for folders', () => {
		const plain = renderToStaticMarkup(<FileIcon group="folder" />);
		const downloads = renderToStaticMarkup(<FileIcon group="folder" special="downloads" />);
		expect(downloads).toContain(plain.match(/<path[^>]* d="([^"]+)"/)![1]);
		expect(downloads.length).toBeGreaterThan(plain.length);
		const file = renderToStaticMarkup(<FileIcon group="pdf" special="downloads" />);
		expect(file).not.toContain('data-special');
	});
});

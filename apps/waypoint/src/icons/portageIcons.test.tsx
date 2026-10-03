// Verifies the Portage art covers every icon group and standard folder and is clean, static SVG
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import { PortageIcon } from './PortageIcon';
import {
	PORTAGE_FALLBACK_ART,
	PORTAGE_FILE_ART,
	PORTAGE_STANDARD_FOLDER_GLYPHS,
	portageIconSvg,
} from './portageIcons';
import { PORTAGE_FOLDER_BADGES, PORTAGE_FOLDER_GLYPHS } from './portage/portageFolderArt';
import { portageFolderSvg, type PortageFolderVariant } from './portage/portageFolderSvg';
import { PORTAGE_FILE_SVGS } from './portage/portageFiles';
import { FOLDER_COLOURS, FOLDER_PALETTE, type FolderTone } from './portage/portagePalette';

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

const TONES: FolderTone[] = ['dark', 'light'];
const VARIANTS: PortageFolderVariant[] = ['closed', 'open', 'empty'];
const BADGES = Object.keys(PORTAGE_FOLDER_BADGES) as (keyof typeof PORTAGE_FOLDER_BADGES)[];
const GLYPHS = Object.keys(PORTAGE_FOLDER_GLYPHS) as (keyof typeof PORTAGE_FOLDER_GLYPHS)[];
const SHAPES = /<(path|rect|circle|ellipse)\b/;

/** Parses the SVG and returns its root; fails if the text is not well-formed. */
function parse(svg: string): Element {
	const doc = new DOMParser().parseFromString(svg, 'image/svg+xml');
	expect(doc.querySelector('parsererror'), svg.slice(0, 120)).toBeNull();
	const root = doc.documentElement;
	expect(root.tagName).toBe('svg');
	expect(root.getAttribute('viewBox')).toBe('0 0 64 64');
	return root;
}

function ids(root: Element): string[] {
	return [...root.querySelectorAll('[id]')].map((el) => el.id);
}

/** Every combination of colour, tone and variant, with each badge and glyph on its own. */
function folderOutputs(): string[] {
	const outputs: string[] = [];
	for (const colour of FOLDER_COLOURS) {
		for (const tone of TONES) {
			for (const variant of VARIANTS) {
				outputs.push(portageFolderSvg({ colour, tone, variant }));
				for (const badge of BADGES)
					outputs.push(portageFolderSvg({ colour, tone, variant, badge }));
				for (const glyph of GLYPHS)
					outputs.push(portageFolderSvg({ colour, tone, variant, glyph }));
			}
		}
	}
	return outputs;
}

describe('the Portage icon manifest', () => {
	it('resolves every icon group Rust can send to art', () => {
		const groups = literals('IconGroup');
		expect(groups.length).toBeGreaterThanOrEqual(31);
		expect(Object.keys(PORTAGE_FILE_ART).sort()).toEqual(
			groups.filter((group) => group !== 'folder').sort(),
		);
		for (const group of groups as IconGroup[]) {
			expect(portageIconSvg({ group }), group).toMatch(SHAPES);
		}
		for (const name of Object.values(PORTAGE_FILE_ART)) {
			expect(PORTAGE_FILE_SVGS[name], name).toBeTruthy();
		}
	});

	it('lists the groups that borrow another type’s art', () => {
		expect(Object.keys(PORTAGE_FALLBACK_ART)).toEqual(['other']);
		expect(PORTAGE_FILE_ART.other).toBe(PORTAGE_FALLBACK_ART.other);
	});

	it('draws every standard folder with its own mark', () => {
		const folders = literals('SpecialFolder');
		expect(Object.keys(PORTAGE_STANDARD_FOLDER_GLYPHS).sort()).toEqual([...folders].sort());
		const plain = portageIconSvg({ group: 'folder' });
		const seen = new Set<string>();
		for (const special of folders as SpecialFolder[]) {
			const svg = portageIconSvg({ group: 'folder', special });
			expect(svg, special).not.toBe(plain);
			seen.add(svg);
		}
		expect(seen.size).toBe(folders.length);
	});

	it('ignores folder options for a file', () => {
		expect(portageIconSvg({ group: 'pdf', colour: 'red', tone: 'light', badge: 'git' })).toBe(
			portageIconSvg({ group: 'pdf' }),
		);
	});

	it('has the ten folder colours, each in a dark and a light tone', () => {
		expect([...FOLDER_COLOURS]).toEqual([
			'liminal',
			'gnome',
			'cinnamon',
			'kde',
			'windows11',
			'red',
			'pink',
			'orange',
			'purple',
			'rainbow',
		]);
		expect(Object.keys(FOLDER_PALETTE).sort()).toEqual([...FOLDER_COLOURS].sort());
		expect(FOLDER_PALETTE.rainbow.dark).toHaveLength(6);
		expect(FOLDER_PALETTE.rainbow.light).toHaveLength(6);
	});
});

describe('the Portage file art', () => {
	it('is well-formed, static and self-contained', () => {
		for (const [name, svg] of Object.entries(PORTAGE_FILE_SVGS)) {
			const root = parse(svg);
			expect(root.querySelector('text, image, style, script, foreignObject, use'), name).toBeNull();
			expect(svg, name).not.toMatch(
				/https?:\/\/(?!www\.w3\.org\/2000\/svg)|href=|font-family|font-size|@import/,
			);
		}
	});

	it('keeps ids unique within each file and across all of them', () => {
		const all = new Set<string>();
		for (const [name, svg] of Object.entries(PORTAGE_FILE_SVGS)) {
			const own = ids(parse(svg));
			expect(new Set(own).size, name).toBe(own.length);
			for (const id of own) {
				expect(all.has(id), id).toBe(false);
				all.add(id);
			}
			for (const [, ref] of svg.matchAll(/url\(#([^)]+)\)/g)) {
				expect(own, `${name} references ${ref}`).toContain(ref);
			}
		}
	});
});

describe('portageFolderSvg', () => {
	it('draws every colour, tone, variant, badge and glyph as valid SVG with unique ids', () => {
		const outputs = folderOutputs();
		expect(outputs.length).toBe(10 * 2 * 3 * (1 + BADGES.length + GLYPHS.length));
		for (const svg of outputs) {
			const root = parse(svg);
			const own = ids(root);
			expect(new Set(own).size, svg.slice(0, 200)).toBe(own.length);
			for (const [, ref] of svg.matchAll(/url\(#([^)]+)\)/g)) expect(own).toContain(ref);
		}
	});

	it('uses different art in the light and dark tones of every colour', () => {
		for (const colour of FOLDER_COLOURS) {
			expect(portageFolderSvg({ colour, tone: 'dark' }), colour).not.toBe(
				portageFolderSvg({ colour, tone: 'light' }),
			);
		}
	});

	it('draws each variant and badge differently', () => {
		const set = new Set([
			...VARIANTS.map((variant) => portageFolderSvg({ variant })),
			...BADGES.map((badge) => portageFolderSvg({ badge })),
		]);
		expect(set.size).toBe(VARIANTS.length + BADGES.length);
	});

	it('gives the Rainbow folder a gradient and the others a flat colour', () => {
		expect(portageFolderSvg({ colour: 'rainbow' })).toContain('<linearGradient');
		expect(portageFolderSvg({ colour: 'rainbow' }).match(/<stop /g)).toHaveLength(6);
		expect(portageFolderSvg({ colour: 'red' })).not.toContain('<linearGradient');
	});

	it('gives different folders different ids so several can share a page', () => {
		const a = ids(parse(portageFolderSvg({ colour: 'red' })));
		const b = ids(parse(portageFolderSvg({ colour: 'pink' })));
		expect(a.filter((id) => b.includes(id))).toEqual([]);
	});

	it('references nothing outside itself', () => {
		for (const svg of folderOutputs()) {
			expect(svg).not.toMatch(/https?:\/\/(?!www\.w3\.org\/2000\/svg)|href=|<text|<image|<style/);
		}
	});
});

describe('PortageIcon', () => {
	it('is decorative, not focusable and sized by the size prop', () => {
		const html = renderToStaticMarkup(<PortageIcon group="pdf" size={32} />);
		expect(html).toContain('aria-hidden="true"');
		expect(html).toContain('focusable="false"');
		expect(html).toContain('width="32"');
		expect(html).toContain('height="32"');
		expect(html).toContain('viewBox="0 0 64 64"');
		expect(html).toMatch(SHAPES);
	});

	it('defaults to the 16 pixel list size and takes an extra class', () => {
		const html = renderToStaticMarkup(<PortageIcon group="folder" className="big" />);
		expect(html).toContain('width="16"');
		expect(html).toMatch(/class="[^"]*\bbig\b/);
	});

	it('marks a standard folder and draws it in the requested colour and tone', () => {
		const html = renderToStaticMarkup(
			<PortageIcon group="folder" special="music" colour="purple" tone="light" />,
		);
		expect(html).toContain('data-special="music"');
		expect(html).toContain('pf-purple-light-closed-music');
		expect(renderToStaticMarkup(<PortageIcon group="pdf" special="music" />)).not.toContain(
			'data-special',
		);
	});

	it('draws every icon group', () => {
		for (const group of literals('IconGroup') as IconGroup[]) {
			expect(renderToStaticMarkup(<PortageIcon group={group} />), group).toMatch(SHAPES);
		}
	});
});

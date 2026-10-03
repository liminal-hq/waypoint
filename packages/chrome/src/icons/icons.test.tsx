// Tests the chrome icons follow the host's icon style tokens and stay safe under the Filled style
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { expectFillableIcons, fillProblems, subpathKinds } from './expectFillableIcons';
import * as chromeIcons from './icons';

const components = Object.entries(chromeIcons).filter(([name]) => name.endsWith('Icon')) as [
	string,
	(props: object) => React.ReactElement,
][];

describe('the chrome icons', () => {
	it('exports icons to audit', () => {
		expect(components.length).toBeGreaterThanOrEqual(8);
	});

	it('marks every shape as fillable or leaves it an open stroke', () => {
		expectFillableIcons(
			Object.fromEntries(components.map(([name, Icon]) => [name, renderToStaticMarkup(<Icon />)])),
		);
	});

	it('reads the weight and the fill from the icon style tokens, with a default in the chrome', () => {
		const css = readFileSync(join(import.meta.dirname, 'icons.module.css'), 'utf8');
		expect(css).toMatch(/stroke-width:\s*var\(--wp-icon-stroke\)/);
		expect(css).toMatch(/\[data-fill\]\s*\{\s*fill:\s*var\(--wp-icon-fill\)/);
		const tokens = readFileSync(join(import.meta.dirname, '../tokens.css'), 'utf8');
		expect(tokens).toMatch(/--wp-icon-stroke:\s*1\.25px/);
		expect(tokens).toMatch(/--wp-icon-fill:\s*none/);
	});

	it('draws no fixed stroke width that the style could not change', () => {
		const markup = renderToStaticMarkup(<chromeIcons.CheckIcon />);
		expect(markup).not.toContain('stroke-width');
	});
});

describe('the fill audit', () => {
	it('classifies subpaths', () => {
		expect(subpathKinds('M1 1h5v5H1z')).toEqual(['closed']);
		expect(subpathKinds('M1 1h5v5H1zM2 2v3')).toEqual(['closed', 'line']);
		expect(subpathKinds('M3 3l4 4 4-4')).toEqual(['open']);
		expect(subpathKinds('M3 3h.01')).toEqual(['line']);
	});

	it('flags an unmarked closed shape, a fillable chevron and a marked line', () => {
		expect(fillProblems('<svg><rect x="1" /></svg>')).toHaveLength(1);
		expect(fillProblems('<svg><path d="M1 1h5v5H1z" /></svg>')).toHaveLength(1);
		expect(fillProblems('<svg><path data-fill d="M3 3l4 4 4-4" /></svg>')).toHaveLength(1);
		expect(fillProblems('<svg><line data-fill x1="1" /></svg>')).toHaveLength(1);
	});

	it('accepts marked shapes, open strokes, outlines and shapes the glyph fills itself', () => {
		expect(fillProblems('<svg><rect data-fill x="1" /><path d="M3 3l4 4 4-4" /></svg>')).toEqual(
			[],
		);
		expect(fillProblems('<svg><g data-outline><circle r="1" /></g></svg>')).toEqual([]);
		expect(fillProblems('<svg fill="currentColor"><circle r="1" /></svg>')).toEqual([]);
		expect(fillProblems('<svg><circle fill="currentColor" r="1" /></svg>')).toEqual([]);
	});
});

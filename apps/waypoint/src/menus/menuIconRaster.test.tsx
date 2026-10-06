// Verifies the icon rasteriser with a stub canvas: sizes, caching, the colour a glyph is drawn in and what it cannot draw
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createIconRasteriser, standaloneSvg, type RasteriserEnvironment } from './menuIconRaster';

afterEach(() => {
	document.body.replaceChildren();
	document.documentElement.removeAttribute('style');
});

function environment(scale = 1) {
	const drawn: Array<{ markup: string; px: number }> = [];
	const stub: RasteriserEnvironment & { scaleNow: number } = {
		scaleNow: scale,
		scale() {
			return this.scaleNow;
		},
		draw: vi.fn(async (markup: string, px: number) => {
			drawn.push({ markup, px });
			return new Array<number>(px * px * 4).fill(7);
		}),
	};
	return { stub, drawn };
}

const glyph = (
	<svg viewBox="0 0 16 16" className="glyph">
		<path d="M1 1h14v14H1z" fill="none" stroke="currentColor" strokeWidth="1.5" />
	</svg>
);

describe('createIconRasteriser', () => {
	it('draws an SVG glyph 16 logical pixels square at the screen’s scale', async () => {
		for (const [scale, px] of [
			[1, 16],
			[1.5, 24],
			[2, 32],
		] as const) {
			const { stub } = environment(scale);
			const picture = await createIconRasteriser(stub)(glyph);
			expect(picture).toMatchObject({ width: px, height: px });
			expect(picture?.rgba).toHaveLength(px * px * 4);
		}
	});

	it('never asks for more pixels than the command takes', async () => {
		const { stub } = environment(10);
		const picture = await createIconRasteriser(stub)(glyph);
		expect(picture).toMatchObject({ width: 64, height: 64 });
	});

	it('draws the glyph as a stand-alone image with its size and paint written onto it', async () => {
		document.documentElement.style.setProperty('--wp-text-primary', 'rgb(10, 20, 30)');
		const { stub, drawn } = environment(2);
		await createIconRasteriser(stub)(glyph);
		expect(drawn).toHaveLength(1);
		const markup = drawn[0]?.markup ?? '';
		expect(markup).toContain('xmlns="http://www.w3.org/2000/svg"');
		expect(markup).toContain('width="32"');
		expect(markup).toContain('height="32"');
		expect(markup).not.toContain('class=');
		expect(markup).toContain('stroke-width');
	});

	it('keeps a picture for the same icon, colour and scale, and draws again when any changes', async () => {
		const { stub, drawn } = environment(1);
		const rasterise = createIconRasteriser(stub);
		const first = await rasterise(glyph);
		expect(await rasterise(<svg viewBox="0 0 16 16" className="glyph" />)).not.toBe(first);
		const again = await rasterise(glyph);
		expect(again).toBe(first);
		expect(drawn).toHaveLength(2);
		stub.scaleNow = 2;
		await rasterise(glyph);
		expect(drawn).toHaveLength(3);
		document.documentElement.style.setProperty('--wp-icon-stroke', 'red');
		await rasterise(glyph);
		expect(drawn).toHaveLength(4);
	});

	it('has no picture for what is not an SVG, such as an application’s image or a swatch', async () => {
		const { stub } = environment();
		const rasterise = createIconRasteriser(stub);
		expect(await rasterise(<img alt="" src="appicon://x" />)).toBeNull();
		expect(await rasterise(<span aria-hidden="true" />)).toBeNull();
		expect(await rasterise(null)).toBeNull();
		expect(await rasterise(undefined)).toBeNull();
		expect(await rasterise(false)).toBeNull();
		expect(stub.draw).not.toHaveBeenCalled();
	});

	it('has no picture when the canvas gives the wrong number of bytes, or fails', async () => {
		const wrong = createIconRasteriser({ scale: () => 1, draw: async () => [1, 2, 3] });
		expect(await wrong(glyph)).toBeNull();
		const none = createIconRasteriser({ scale: () => 1, draw: async () => null });
		expect(await none(glyph)).toBeNull();
		const debug = vi.spyOn(console, 'debug').mockImplementation(() => {});
		const failing = createIconRasteriser({
			scale: () => 1,
			draw: async () => {
				throw new Error('no canvas');
			},
		});
		expect(await failing(glyph)).toBeNull();
		expect(debug).toHaveBeenCalled();
		debug.mockRestore();
	});

	it('renders its glyphs out of sight and out of the accessibility tree', async () => {
		const { stub } = environment();
		await createIconRasteriser(stub)(glyph);
		const stage = document.body.querySelector('[aria-hidden="true"]');
		expect(stage).not.toBeNull();
		expect(stage?.querySelector('svg')).not.toBeNull();
	});
});

describe('standaloneSvg', () => {
	it('copies the element without touching the one in the document', () => {
		document.body.innerHTML =
			'<svg viewBox="0 0 16 16" class="x"><circle r="4" fill="red"></circle></svg>';
		const svg = document.body.querySelector('svg') as SVGSVGElement;
		const markup = standaloneSvg(svg, 24);
		expect(markup).toContain('width="24"');
		expect(svg.getAttribute('class')).toBe('x');
		expect(svg.hasAttribute('width')).toBe(false);
	});
});

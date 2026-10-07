// Verifies the icon rasteriser with a stub canvas: sizes, caching, the colour a glyph is drawn in and what it cannot draw
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createIconRasteriser, standaloneSvg, type RasteriserEnvironment } from './menuIconRaster';

afterEach(() => {
	document.body.replaceChildren();
	document.documentElement.removeAttribute('style');
	document.head.replaceChildren();
});

function environment(scale = 1) {
	const drawn: Array<{ markup: string; px: number }> = [];
	const drawnImages: Array<{ url: string; px: number }> = [];
	const stub: RasteriserEnvironment & { scaleNow: number } = {
		scaleNow: scale,
		scale() {
			return this.scaleNow;
		},
		draw: vi.fn(async (markup: string, px: number) => {
			drawn.push({ markup, px });
			return new Array<number>(px * px * 4).fill(7);
		}),
		drawImage: vi.fn(async (url: string, px: number) => {
			drawnImages.push({ url, px });
			return new Array<number>(px * px * 4).fill(9);
		}),
	};
	return { stub, drawn, drawnImages };
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

	it('has no picture for what is neither an SVG, a filled swatch nor an image with an address', async () => {
		const { stub } = environment();
		const rasterise = createIconRasteriser(stub);
		expect(await rasterise(<img alt="" />)).toBeNull();
		expect(await rasterise(<img alt="" src="" />)).toBeNull();
		expect(await rasterise(<span aria-hidden="true" />)).toBeNull();
		expect(await rasterise(null)).toBeNull();
		expect(await rasterise(undefined)).toBeNull();
		expect(await rasterise(false)).toBeNull();
		expect(stub.draw).not.toHaveBeenCalled();
	});

	describe('an application’s image', () => {
		const app = <img alt="" src="appicon://localhost/org.gnome.gedit?size=16" />;

		it('is read from its address at the screen’s scale, as the system’s icon is, and never drawn as markup', async () => {
			const { stub, drawn, drawnImages } = environment(2);
			const picture = await createIconRasteriser(stub)(app);
			expect(picture).toMatchObject({ width: 32, height: 32 });
			expect(picture?.rgba).toHaveLength(32 * 32 * 4);
			expect(drawnImages).toEqual([{ url: 'appicon://localhost/org.gnome.gedit?size=16', px: 32 }]);
			expect(drawn).toEqual([]);
		});

		it('is kept for the same address and size, and read again for another address or scale', async () => {
			const { stub, drawnImages } = environment();
			const rasterise = createIconRasteriser(stub);
			await rasterise(app);
			await rasterise(app);
			expect(drawnImages).toHaveLength(1);
			await rasterise(<img alt="" src="appicon://localhost/other?size=16" />);
			expect(drawnImages).toHaveLength(2);
			stub.scaleNow = 2;
			await rasterise(app);
			expect(drawnImages).toHaveLength(3);
		});

		it('has no picture when it cannot be read, and is tried again next time', async () => {
			const { stub, drawnImages } = environment();
			vi.mocked(stub.drawImage).mockResolvedValueOnce(null);
			const rasterise = createIconRasteriser(stub);
			expect(await rasterise(app)).toBeNull();
			expect(await rasterise(app)).not.toBeNull();
			expect(drawnImages).toHaveLength(1);
		});
	});

	it('draws a dangerous item’s icon in the danger colour and any other in the text colour, and keeps them apart', async () => {
		const style = document.createElement('style');
		style.textContent =
			'div[aria-hidden] { color: rgb(1, 2, 3); } div[aria-hidden][data-danger] { color: rgb(200, 10, 20); }';
		document.head.append(style);
		const { stub, drawn } = environment(1);
		const rasterise = createIconRasteriser(stub);
		const plain = await rasterise(glyph, { danger: false });
		const danger = await rasterise(glyph, { danger: true });
		expect(drawn[0]?.markup).toContain('color: rgb(1, 2, 3)');
		expect(drawn[1]?.markup).toContain('color: rgb(200, 10, 20)');
		expect(danger).not.toBe(plain);
		expect(await rasterise(glyph, { danger: true })).toBe(danger);
		expect(await rasterise(glyph)).toBe(plain);
		expect(drawn).toHaveLength(2);
	});

	it('draws again when the theme changes the colour an icon has', async () => {
		const style = document.createElement('style');
		style.textContent = 'div[aria-hidden] { color: rgb(1, 2, 3); }';
		document.head.append(style);
		const { stub, drawn } = environment(1);
		const rasterise = createIconRasteriser(stub);
		await rasterise(glyph);
		style.textContent = 'div[aria-hidden] { color: rgb(250, 250, 250); }';
		await rasterise(glyph);
		expect(drawn).toHaveLength(2);
		expect(drawn[1]?.markup).toContain('color: rgb(250, 250, 250)');
	});

	describe('a colour swatch', () => {
		const dot = (colour: string) => (
			<span
				style={{
					display: 'inline-block',
					width: 10,
					height: 10,
					borderRadius: '50%',
					backgroundColor: colour,
				}}
			/>
		);

		it('is drawn as its shape in its colour, and says its picture marks the check', async () => {
			const { stub, drawn } = environment(2);
			const picture = await createIconRasteriser(stub)(dot('rgb(220, 38, 38)'));
			expect(picture).toMatchObject({ width: 32, height: 32, marksCheck: true });
			const markup = drawn[0]?.markup ?? '';
			expect(markup).toContain('fill="rgb(220, 38, 38)"');
			expect(markup).toContain('width="10"');
			expect(markup).toContain('rx="5"');
			expect(markup).not.toContain('<circle');
		});

		it('has a ring round it when its item is checked, and is kept apart from the unchecked one', async () => {
			const style = document.createElement('style');
			style.textContent = 'div[aria-hidden] { color: rgb(1, 2, 3); }';
			document.head.append(style);
			const { stub, drawn } = environment(1);
			const rasterise = createIconRasteriser(stub);
			const unchecked = await rasterise(dot('rgb(37, 99, 235)'), { checked: false });
			const checked = await rasterise(dot('rgb(37, 99, 235)'), { checked: true });
			expect(checked).not.toBe(unchecked);
			expect(drawn[0]?.markup).not.toContain('<circle');
			expect(drawn[1]?.markup).toContain('<circle');
			expect(drawn[1]?.markup).toContain('stroke="rgb(1, 2, 3)"');
			expect(await rasterise(dot('rgb(37, 99, 235)'), { checked: true })).toBe(checked);
			expect(drawn).toHaveLength(2);
		});

		it('is drawn again in the colour a theme gives it', async () => {
			const { stub, drawn } = environment(1);
			const rasterise = createIconRasteriser(stub);
			await rasterise(dot('rgb(220, 38, 38)'));
			await rasterise(dot('rgb(248, 113, 113)'));
			expect(drawn).toHaveLength(2);
			expect(drawn[1]?.markup).toContain('fill="rgb(248, 113, 113)"');
		});
	});

	it('has no picture when the canvas gives the wrong number of bytes, or fails', async () => {
		const wrong = createIconRasteriser({
			scale: () => 1,
			draw: async () => [1, 2, 3],
			drawImage: async () => null,
		});
		expect(await wrong(glyph)).toBeNull();
		const none = createIconRasteriser({
			scale: () => 1,
			draw: async () => null,
			drawImage: async () => null,
		});
		expect(await none(glyph)).toBeNull();
		const debug = vi.spyOn(console, 'debug').mockImplementation(() => {});
		const failing = createIconRasteriser({
			scale: () => 1,
			drawImage: async () => null,
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

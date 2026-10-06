// Verifies glyphs drawn as pictures: the markup, the resolved paint, the on-screen stroke and the shared addresses
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { cssUrl, glyphSvg, pictureUrl } from './iconPictures';
import { WAYPOINT_FILE_GLYPHS, WAYPOINT_FOLDER_GLYPHS } from './waypointFileIcons';

const paint = { stroke: 'rgb(10, 20, 30)', fill: 'rgb(200, 210, 220)', width: 1.5 };

describe('a glyph as a picture', () => {
	it('is a standalone SVG of the glyph shapes, with the marked shapes filled', () => {
		const svg = glyphSvg('other', WAYPOINT_FILE_GLYPHS.other, paint, 64);
		expect(svg).toMatch(/^<svg xmlns="http:\/\/www.w3.org\/2000\/svg" viewBox="0 0 16 16"/);
		expect(svg).toContain('stroke="rgb(10, 20, 30)"');
		expect(svg).toContain('<path fill="rgb(200, 210, 220)" d="M4 1.5h5.5L13 5v9.5H4z"></path>');
		expect(svg).toContain('<path d="M9.5 1.5V5H13"></path>');
		expect(svg).not.toContain('data-');
		const parsed = new DOMParser().parseFromString(svg, 'image/svg+xml');
		expect(parsed.querySelector('parsererror')).toBeNull();
	});

	it('keeps the stroke its on-screen width at every size, as the inline glyph does', () => {
		const width = (size: number) =>
			Number(
				/stroke-width="([\d.]+)"/.exec(
					glyphSvg('other', WAYPOINT_FILE_GLYPHS.other, paint, size),
				)![1],
			);
		// 1.5 px on screen is 1.5 / 4 of a unit at 64 px (16 units) and 1.5 / 16 at 256 px.
		expect(width(64)).toBeCloseTo(0.375);
		expect(width(256)).toBeCloseTo(0.094, 3);
	});

	it('keeps the marks of a standard folder unfilled', () => {
		const svg = glyphSvg('folder:home', WAYPOINT_FOLDER_GLYPHS.home, paint, 96);
		expect(svg).toContain('<g fill="none">');
		expect(svg.match(/fill="rgb\(200, 210, 220\)"/g)).toHaveLength(1);
	});

	it('is one address however many cells draw it, and a new one for other art', () => {
		const svg = glyphSvg('other', WAYPOINT_FILE_GLYPHS.other, paint, 48);
		expect(pictureUrl(svg)).toBe(pictureUrl(svg));
		expect(pictureUrl(glyphSvg('other', WAYPOINT_FILE_GLYPHS.other, paint, 56))).not.toBe(
			pictureUrl(svg),
		);
		expect(cssUrl('blob:x"y')).toBe('url("blob:x\\"y")');
	});
});

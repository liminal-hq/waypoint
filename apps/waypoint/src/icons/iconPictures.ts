// Icon art as pictures: a standalone SVG for a glyph in resolved colours, and one shared address per picture
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Fragment, isValidElement, type ReactNode } from 'react';

/**
 * Why pictures: an inline `<svg>` is laid out and its paths stroked again in every cell that shows
 * it, while a picture drawn as a background is decoded once and copied. In a wide grid scrolled fast,
 * the inline glyphs alone cost a quarter of each frame (#565).
 */

/** How a glyph is drawn, as the stylesheet resolves it for one kind of icon (a file or a folder). */
export interface GlyphPaint {
	/** The stroke colour, a resolved CSS colour. */
	stroke: string;
	/** The colour of the closed shapes marked `data-fill`, or `none`. */
	fill: string;
	/** The stroke width in CSS pixels on screen, whatever size the icon is drawn at. */
	width: number;
}

/** Pictures kept at most; the least recently asked for goes first. */
const MAX_PICTURES = 1024;
const pictures = new Map<string, string>();

/**
 * The address of an SVG document as a picture, made once and the same string after, so every cell
 * that draws it shares one decoded image. A picture not asked for in a long while is let go.
 */
export function pictureUrl(svg: string): string {
	const known = pictures.get(svg);
	if (known !== undefined) {
		// Most recently used last, so eviction takes what has not been drawn for longest.
		pictures.delete(svg);
		pictures.set(svg, known);
		return known;
	}
	const url =
		typeof URL.createObjectURL === 'function'
			? URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }))
			: `data:image/svg+xml,${encodeURIComponent(svg)}`;
	pictures.set(svg, url);
	if (pictures.size > MAX_PICTURES) {
		const [oldest, address] = pictures.entries().next().value as [string, string];
		pictures.delete(oldest);
		if (address.startsWith('blob:')) URL.revokeObjectURL(address);
	}
	return url;
}

/** A value for CSS `background-image`. */
export function cssUrl(url: string): string {
	return `url(${JSON.stringify(url)})`;
}

const ATTRIBUTE_NAMES: Record<string, string> = { className: 'class' };

function escapeAttribute(value: string): string {
	return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
}

/**
 * The markup of a glyph's shapes (the children of a Waypoint glyph's `<svg>`): plain SVG elements and
 * fragments, as `waypointFileIcons.tsx` writes them. A shape marked `data-fill` takes `fill`.
 */
function shapesMarkup(node: ReactNode, fill: string): string {
	if (node === null || node === undefined || typeof node === 'boolean') return '';
	if (Array.isArray(node))
		return node.map((child: ReactNode) => shapesMarkup(child, fill)).join('');
	if (typeof node === 'string' || typeof node === 'number') return escapeAttribute(String(node));
	if (!isValidElement<Record<string, unknown>>(node)) return '';
	const { children, ...props } = node.props;
	if (node.type === Fragment) return shapesMarkup(children as ReactNode, fill);
	if (typeof node.type !== 'string') return '';
	let attributes = '';
	for (const [name, value] of Object.entries(props)) {
		if (value === undefined || value === null || value === false) continue;
		if (name === 'data-fill') {
			attributes += ` fill="${escapeAttribute(fill)}"`;
			continue;
		}
		if (name.startsWith('data-')) continue;
		attributes += ` ${ATTRIBUTE_NAMES[name] ?? name}="${escapeAttribute(String(value))}"`;
	}
	return `<${node.type}${attributes}>${shapesMarkup(children as ReactNode, fill)}</${node.type}>`;
}

const GLYPH_CACHE = new Map<string, string>();

/**
 * A Waypoint glyph (16 by 16 shapes) as a standalone SVG drawn at `size` pixels, in `paint`. The
 * stroke keeps its on-screen width at every size, as the inline glyph's non-scaling stroke does.
 * Memoised by `key`, which names the glyph; the paint and size are part of the memo.
 */
export function glyphSvg(key: string, shapes: ReactNode, paint: GlyphPaint, size: number): string {
	const memo = `${key}|${size}|${paint.stroke}|${paint.fill}|${paint.width}`;
	let svg = GLYPH_CACHE.get(memo);
	if (svg === undefined) {
		const width = Math.round(((paint.width * 16) / Math.max(1, size)) * 1000) / 1000;
		svg =
			`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="${size}" height="${size}" ` +
			`fill="none" stroke="${escapeAttribute(paint.stroke)}" stroke-width="${width}" ` +
			`stroke-linecap="round" stroke-linejoin="round">${shapesMarkup(shapes, paint.fill)}</svg>`;
		if (GLYPH_CACHE.size >= MAX_PICTURES) GLYPH_CACHE.clear();
		GLYPH_CACHE.set(memo, svg);
	}
	return svg;
}

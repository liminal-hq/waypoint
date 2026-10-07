// Draws a menu icon (an SVG glyph or a colour swatch) as RGBA pixels, in the colour the page's menu shows it in, for the native menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { isValidElement, type ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import type { IconLook, IconPicture } from './nativeMenu';
import styles from './MenuIconStage.module.css';

/** The icon's size in logical pixels, which the command tells the system to draw it at. */
const LOGICAL_SIZE = 16;
/** The command accepts no more than this on a side. */
const MAX_EDGE = 64;
const SVG_NAMESPACE = 'http://www.w3.org/2000/svg';
/**
 * What an icon's look comes from. The glyphs are styled by classes (`--wp-icon-stroke`, the fill of
 * a `data-fill` shape) and by `currentColor`, none of which survive being drawn as a stand-alone
 * image, so each is written onto the copy that is drawn.
 */
const PAINT = [
	'fill',
	'fill-opacity',
	'stroke',
	'stroke-opacity',
	'stroke-width',
	'stroke-linecap',
	'stroke-linejoin',
	'stroke-dasharray',
	'opacity',
] as const;

/** An icon rasteriser: the picture of `icon`, or `null` where there is none to make. */
export type IconRasteriser = (icon: ReactNode, look?: IconLook) => Promise<IconPicture | null>;

export interface RasteriserEnvironment {
	/** Screen pixels per logical pixel. */
	scale(): number;
	/** Draws `markup` (a stand-alone SVG) `px` pixels square and returns its RGBA bytes; `null` when it cannot. */
	draw(markup: string, px: number): Promise<number[] | null>;
}

/** Draws through an `<img>` and a canvas: a data URL image does not taint the canvas. */
async function drawWithCanvas(markup: string, px: number): Promise<number[] | null> {
	const image = new Image(px, px);
	image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(markup)}`;
	await image.decode();
	const canvas = document.createElement('canvas');
	canvas.width = px;
	canvas.height = px;
	const context = canvas.getContext('2d');
	if (!context) return null;
	context.drawImage(image, 0, 0, px, px);
	return Array.from(context.getImageData(0, 0, px, px).data);
}

const DOM_ENVIRONMENT: RasteriserEnvironment = {
	scale: () => window.devicePixelRatio || 1,
	draw: drawWithCanvas,
};

/**
 * `svg` as a stand-alone image `px` square, with the paint it has in the document written onto each
 * element, and `colour` (the text colour the glyph is in) as the colour `currentColor` takes. `svg`
 * must be in the document so its styles resolve.
 */
export function standaloneSvg(svg: SVGSVGElement, px: number, colour?: string): string {
	const copy = svg.cloneNode(true) as SVGSVGElement;
	const from = [svg, ...Array.from(svg.querySelectorAll('*'))];
	const to = [copy, ...Array.from(copy.querySelectorAll('*'))];
	from.forEach((source, index) => {
		const target = to[index];
		if (!target) return;
		const computed = getComputedStyle(source);
		const style = PAINT.map((property) => [property, computed.getPropertyValue(property)] as const)
			.filter(([, value]) => value !== '')
			.map(([property, value]) => `${property}:${value}`)
			.join(';');
		target.removeAttribute('class');
		target.setAttribute('style', style);
	});
	if (colour) copy.style.setProperty('color', colour);
	copy.setAttribute('xmlns', SVG_NAMESPACE);
	copy.setAttribute('width', String(px));
	copy.setAttribute('height', String(px));
	return new XMLSerializer().serializeToString(copy);
}

const typeIds = new WeakMap<object, number>();
let nextTypeId = 1;

/** What makes two icon elements draw alike: their type and props. `null` when that cannot be told. */
function iconKey(icon: ReactNode): string | null {
	if (!isValidElement(icon)) return null;
	const type = icon.type;
	let name: string;
	if (typeof type === 'string') {
		name = type;
	} else {
		let id = typeIds.get(type as object);
		if (id === undefined) {
			id = nextTypeId++;
			typeIds.set(type as object, id);
		}
		name = `#${id}`;
	}
	try {
		return `${name}:${JSON.stringify(icon.props)}`;
	} catch {
		return null;
	}
}

/** The width of the ring that marks a checked swatch, in the 16-unit box a swatch is drawn in. */
const RING_WIDTH = 1.5;
const RING_RADIUS = 7.25;
const DEFAULT_SWATCH = 10;

function isClear(colour: string): boolean {
	const value = colour.replace(/\s+/g, '').toLowerCase();
	return (
		value === '' ||
		value === 'transparent' ||
		/^rgba\(.*,0(\.0+)?\)$/.test(value) ||
		/\/0(\.0+)?\)$/.test(value)
	);
}

/** A swatch's look: the colour it fills with and how it is shaped, read from the element drawn in the stage. */
interface Swatch {
	fill: string;
	diameter: number;
	/** The corner radius in the 16-unit box; a circle when it is at least half the diameter. */
	radius: number;
}

function swatchOf(element: Element | null): Swatch | null {
	if (!element) return null;
	const computed = getComputedStyle(element);
	const fill = computed.backgroundColor;
	if (isClear(fill)) return null;
	const width = parseFloat(computed.width);
	const diameter = Math.min(14, Math.max(4, Number.isFinite(width) ? width : DEFAULT_SWATCH));
	const corner = computed.borderRadius;
	const radius = corner.includes('%')
		? (parseFloat(corner) / 100) * diameter
		: Number.isFinite(parseFloat(corner))
			? parseFloat(corner)
			: 0;
	return { fill, diameter, radius: Math.min(radius, diameter / 2) };
}

/** A swatch as a stand-alone image: its shape in its colour, and a ring in `ring` round it when checked. */
function swatchMarkup(swatch: Swatch, px: number, checked: boolean, ring: string): string {
	const offset = (16 - swatch.diameter) / 2;
	const shape = `<rect x="${offset}" y="${offset}" width="${swatch.diameter}" height="${swatch.diameter}" rx="${swatch.radius}" fill="${escapeAttribute(swatch.fill)}"/>`;
	const mark = checked
		? `<circle cx="8" cy="8" r="${RING_RADIUS}" fill="none" stroke="${escapeAttribute(ring)}" stroke-width="${RING_WIDTH}"/>`
		: '';
	return `<svg xmlns="${SVG_NAMESPACE}" viewBox="0 0 16 16" width="${px}" height="${px}">${shape}${mark}</svg>`;
}

function escapeAttribute(value: string): string {
	return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
}

/**
 * Makes the rasteriser a window uses. An icon is rendered in a hidden stage in the document, so it
 * takes the styles it has in a menu, then drawn at 16 logical pixels at the screen's scale in the
 * colour the page's menu gives it: the danger colour for an item that is `danger`, the menu text
 * colour otherwise. A colour swatch (an element that fills with a colour rather than an SVG) is drawn
 * as the shape and colour it has, and with a ring round it when its item is checked. Pictures are
 * kept by icon, colours, scale and icon style, so a menu opened again draws nothing. Anything else
 * (an application's icon) gives `null`, and its item goes without.
 */
export function createIconRasteriser(
	environment: RasteriserEnvironment = DOM_ENVIRONMENT,
): IconRasteriser {
	const cache = new Map<string, IconPicture | null>();
	let made: { stage: HTMLElement; root: Root } | null = null;

	const stageOf = () => {
		if (!made) {
			const stage = document.createElement('div');
			stage.className = styles.stage ?? '';
			stage.setAttribute('aria-hidden', 'true');
			document.body.append(stage);
			made = { stage, root: createRoot(stage) };
		}
		return made;
	};

	return async (icon, look = {}) => {
		if (icon === undefined || icon === null || typeof icon === 'boolean') return null;
		const { stage, root } = stageOf();
		const px = Math.min(MAX_EDGE, Math.max(1, Math.round(LOGICAL_SIZE * environment.scale())));
		const theme = getComputedStyle(document.documentElement);
		if (look.danger) stage.setAttribute('data-danger', '');
		else stage.removeAttribute('data-danger');
		const colour = getComputedStyle(stage).color;
		const identity = iconKey(icon);

		try {
			flushSync(() => root.render(icon));
		} catch (error) {
			console.debug('could not draw a menu icon for the native menu', error);
			return null;
		}
		const svg = stage.querySelector('svg');
		const swatch = svg ? null : swatchOf(stage.firstElementChild);
		if (!svg && !swatch) return null;

		const checked = swatch !== null && look.checked === true;
		const key =
			identity === null
				? null
				: [
						identity,
						colour,
						swatch ? `${swatch.fill}/${swatch.diameter}/${swatch.radius}/${checked}` : '',
						px,
						theme.getPropertyValue('--wp-icon-stroke'),
						theme.getPropertyValue('--wp-icon-fill'),
					].join('|');
		if (key !== null && cache.has(key)) return cache.get(key) ?? null;

		let picture: IconPicture | null = null;
		try {
			const markup = swatch
				? swatchMarkup(swatch, px, checked, colour)
				: svg
					? standaloneSvg(svg, px, colour)
					: null;
			const rgba = markup ? await environment.draw(markup, px) : null;
			if (rgba && rgba.length === px * px * 4) {
				picture = { width: px, height: px, rgba, ...(swatch ? { marksCheck: true } : {}) };
			}
		} catch (error) {
			console.debug('could not draw a menu icon for the native menu', error);
		}
		if (key !== null) cache.set(key, picture);
		return picture;
	};
}

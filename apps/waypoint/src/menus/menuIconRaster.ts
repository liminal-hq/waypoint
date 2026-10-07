// Draws a menu icon (an SVG glyph or a colour swatch) as RGBA pixels, in the colour the page's menu shows it in, for the native menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ReactNode } from 'react';
import { createIconStage, type IconStage } from './iconStage';
import type { IconLook, IconPicture } from './nativeMenu';

/** The icon's size in logical pixels, which the command tells the system to draw it at. */
const LOGICAL_SIZE = 16;
/** The command accepts no more than this on a side. */
const MAX_EDGE = 64;
/**
 * How long an icon that is still arriving (the system's picture, the server a location is on) is
 * waited for before the glyph it shows meanwhile is drawn instead. The host gives the whole menu's
 * icons `NATIVE_PREPARE_MS`, so this is well within it, and the menu is still the system's.
 */
export const ICON_SETTLE_MS = 600;
/** After an icon has not arrived in time the others are not waited for this long, so a menu of them is not held row by row. */
const GIVE_UP_MS = 5000;
/** The most pictures kept. */
const MAX_CACHED = 256;
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
	/** Draws the picture at `url` (the system's icon, served with CORS headers) `px` square and returns its RGBA bytes; `null` when it cannot. */
	drawImage(url: string, px: number): Promise<number[] | null>;
}

function pixelsOf(image: HTMLImageElement, px: number): number[] | null {
	const canvas = document.createElement('canvas');
	canvas.width = px;
	canvas.height = px;
	const context = canvas.getContext('2d');
	if (!context) return null;
	context.imageSmoothingQuality = 'high';
	context.drawImage(image, 0, 0, px, px);
	return Array.from(context.getImageData(0, 0, px, px).data);
}

/** Draws through an `<img>` and a canvas: a data URL image does not taint the canvas. */
async function drawWithCanvas(markup: string, px: number): Promise<number[] | null> {
	const image = new Image(px, px);
	image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(markup)}`;
	await image.decode();
	return pixelsOf(image, px);
}

/** The same for a picture at an address; asking with CORS keeps the canvas readable, and a server that does not allow it fails the decode. */
async function drawImageWithCanvas(url: string, px: number): Promise<number[] | null> {
	const image = new Image(px, px);
	image.crossOrigin = 'anonymous';
	image.src = url;
	await image.decode();
	return pixelsOf(image, px);
}

const DOM_ENVIRONMENT: RasteriserEnvironment = {
	scale: () => window.devicePixelRatio || 1,
	draw: drawWithCanvas,
	drawImage: drawImageWithCanvas,
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
 * takes the styles it has in a menu (and, where the stage is hosted in the window's tree, the
 * providers: a location's icon reads the icon set, the file system and the connections like any
 * other), then drawn at 16 logical pixels at the screen's scale in the colour the page's menu gives
 * it: the danger colour for an item that is `danger`, the menu text colour otherwise.
 *
 * An icon that is still arriving is waited for, for `ICON_SETTLE_MS` at most: the System set's folder
 * is a picture that loads after the glyph it shows meanwhile, and a location's server is read from
 * Rust. That picture is drawn through the address it was loaded from; if it does not arrive, or the
 * canvas cannot read it, the glyph shown meanwhile is drawn, as the menu shows it.
 *
 * A colour swatch (an element that fills with a colour rather than an SVG) is drawn as the shape and
 * colour it has, and with a ring round it when its item is checked. Pictures are kept by what is
 * drawn (the glyph with its paint written onto it, or the picture's address, and the size), so a menu
 * opened again draws nothing, every folder in a look shares one picture, and a change of icon set,
 * folder colour, tone or system theme draws again because each changes what is drawn. Anything else
 * (an application's icon) gives `null`, and its item goes without.
 */
export function createIconRasteriser(
	environment: RasteriserEnvironment = DOM_ENVIRONMENT,
	stage: IconStage = createIconStage(),
): IconRasteriser {
	const cache = new Map<string, IconPicture | null>();
	let queue: Promise<unknown> = Promise.resolve();
	let giveUpUntil = 0;

	const toPicture = (rgba: number[] | null, px: number, marksCheck: boolean) =>
		rgba && rgba.length === px * px * 4
			? ({
					width: px,
					height: px,
					rgba,
					...(marksCheck ? { marksCheck: true } : {}),
				} as IconPicture)
			: null;

	const make = async (icon: ReactNode, look: IconLook): Promise<IconPicture | null> => {
		const { element } = stage;
		const px = Math.min(MAX_EDGE, Math.max(1, Math.round(LOGICAL_SIZE * environment.scale())));
		if (look.danger) element.setAttribute('data-danger', '');
		else element.removeAttribute('data-danger');

		try {
			stage.render(icon);
		} catch (error) {
			console.debug('could not draw a menu icon for the native menu', error);
			return null;
		}
		// Read once the stage is in the document, where its styles apply.
		const colour = getComputedStyle(element).color;

		// What the icon shows meanwhile, kept in case what it is waiting for does not come.
		let meanwhile: string | null = null;
		if (stage.settle.pending() > 0) {
			const standIn = element.querySelector('svg');
			if (standIn) meanwhile = standaloneSvg(standIn, px, colour);
			if (Date.now() >= giveUpUntil && !(await stage.settle.settled(ICON_SETTLE_MS))) {
				giveUpUntil = Date.now() + GIVE_UP_MS;
			}
		}

		const svg = element.querySelector('svg');
		const swatch = svg ? null : swatchOf(element.firstElementChild);
		if (!svg && !swatch) return null;
		const checked = swatch !== null && look.checked === true;
		const systemUrl = svg?.hasAttribute('data-system')
			? (svg.querySelector('image')?.getAttribute('href') ?? null)
			: null;
		// An image in a stand-alone SVG does not load, so a system icon is drawn from its address and never as markup.
		const markup = swatch
			? swatchMarkup(swatch, px, checked, colour)
			: systemUrl !== null
				? meanwhile
				: svg
					? standaloneSvg(svg, px, colour)
					: null;
		const tokens = getComputedStyle(document.documentElement);
		const key =
			systemUrl !== null
				? `image|${px}|${systemUrl}`
				: [
						'svg',
						px,
						tokens.getPropertyValue('--wp-icon-stroke'),
						tokens.getPropertyValue('--wp-icon-fill'),
						markup,
					].join('|');
		if (cache.has(key)) return cache.get(key) ?? null;

		let picture: IconPicture | null = null;
		let kept = true;
		try {
			if (systemUrl !== null) {
				try {
					picture = toPicture(await environment.drawImage(systemUrl, px), px, false);
				} catch (error) {
					console.debug('could not read the system icon for the native menu', error);
				}
				if (!picture && markup) {
					// The picture cannot be read: the glyph it replaced stands in (where it was seen), and the picture is tried again next time.
					kept = false;
					picture = toPicture(await environment.draw(markup, px), px, false);
				}
			} else if (markup) {
				picture = toPicture(await environment.draw(markup, px), px, swatch !== null);
			}
		} catch (error) {
			console.debug('could not draw a menu icon for the native menu', error);
		}
		if (kept) {
			if (cache.size >= MAX_CACHED) cache.clear();
			cache.set(key, picture);
		}
		return picture;
	};

	return (icon, look = {}) => {
		if (icon === undefined || icon === null || typeof icon === 'boolean') {
			return Promise.resolve(null);
		}
		// One stage, so one icon at a time.
		const next = queue.then(() => make(icon, look));
		queue = next.catch(() => undefined);
		return next;
	};
}

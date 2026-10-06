// Draws a menu icon (an SVG glyph) as RGBA pixels, in the menu's text colour, for the native menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { isValidElement, type ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { createRoot, type Root } from 'react-dom/client';
import type { NativeMenuIcon } from './nativeMenuClient';
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
export type IconRasteriser = (icon: ReactNode) => Promise<NativeMenuIcon | null>;

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
 * element. `svg` must be in the document so its styles resolve.
 */
export function standaloneSvg(svg: SVGSVGElement, px: number): string {
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

/**
 * Makes the rasteriser a window uses. An icon is rendered in a hidden stage in the document, so it
 * takes the styles and the text colour it has in a menu, then drawn at 16 logical pixels at the
 * screen's scale. Pictures are kept by icon, colour, scale and icon style, so a menu opened again
 * draws nothing. Only an SVG can be drawn: anything else (an application's icon, a colour swatch)
 * gives `null`, and its item goes without.
 */
export function createIconRasteriser(
	environment: RasteriserEnvironment = DOM_ENVIRONMENT,
): IconRasteriser {
	const cache = new Map<string, NativeMenuIcon | null>();
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

	return async (icon) => {
		if (icon === undefined || icon === null || typeof icon === 'boolean') return null;
		const { stage, root } = stageOf();
		const px = Math.min(MAX_EDGE, Math.max(1, Math.round(LOGICAL_SIZE * environment.scale())));
		const look = getComputedStyle(document.documentElement);
		const identity = iconKey(icon);
		const key =
			identity === null
				? null
				: [
						identity,
						getComputedStyle(stage).color,
						px,
						look.getPropertyValue('--wp-icon-stroke'),
						look.getPropertyValue('--wp-icon-fill'),
					].join('|');
		if (key !== null && cache.has(key)) return cache.get(key) ?? null;

		let picture: NativeMenuIcon | null = null;
		try {
			flushSync(() => root.render(icon));
			const svg = stage.querySelector('svg');
			const rgba = svg ? await environment.draw(standaloneSvg(svg, px), px) : null;
			if (rgba && rgba.length === px * px * 4) picture = { width: px, height: px, rgba };
		} catch (error) {
			console.debug('could not draw a menu icon for the native menu', error);
		}
		if (key !== null) cache.set(key, picture);
		return picture;
	};
}

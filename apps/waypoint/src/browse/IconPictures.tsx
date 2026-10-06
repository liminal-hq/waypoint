// Draws the file icons under it as shared pictures, in the colours the stylesheet resolves for the glyphs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	createContext,
	useContext,
	useLayoutEffect,
	useRef,
	useState,
	type ReactNode,
} from 'react';
import type { GlyphPaint } from '../icons/iconPictures';
import styles from './FileIcon.module.css';

/** How the stylesheet paints a file's glyph and a folder's. */
export interface GlyphPaints {
	file: GlyphPaint;
	folder: GlyphPaint;
}

const IconPicturesContext = createContext<GlyphPaints | null>(null);

/**
 * The paints when the icons here are drawn as pictures, or `null` where they are inline SVG (the
 * default, and until the paints have been read).
 */
export function useIconPictures(): GlyphPaints | null {
	return useContext(IconPicturesContext);
}

/** What changes the resolved colours of a glyph besides the root's attributes. */
const QUERIES = [
	'(prefers-color-scheme: dark)',
	'(prefers-contrast: more)',
	'(forced-colors: active)',
];

function paintOf(svg: SVGSVGElement): GlyphPaint {
	const style = getComputedStyle(svg);
	const shape = svg.firstElementChild;
	const stroke = !style.stroke || style.stroke === 'currentcolor' ? style.color : style.stroke;
	return {
		stroke: stroke || 'currentColor',
		fill: (shape ? getComputedStyle(shape).fill : '') || 'none',
		width: parseFloat(style.strokeWidth) || 1,
	};
}

const samePaint = (a: GlyphPaint, b: GlyphPaint) =>
	a.stroke === b.stroke && a.fill === b.fill && a.width === b.width;

interface IconPicturesProps {
	/** The class of a cell and the extra class of its glyph, so the hidden samples resolve the same rules. */
	cellClassName?: string | undefined;
	glyphClassName?: string | undefined;
	children: ReactNode;
}

/**
 * Makes every `FileIcon` under it draw its art as a picture (a background image shared by every
 * cell with the same icon) rather than an inline `<svg>`, for a view that draws hundreds of icons
 * at once. The Waypoint glyphs take their colours and stroke from two hidden sample glyphs styled
 * like the real ones, read again whenever the theme, the accent, the scheme or the contrast changes.
 */
export function IconPictures({ cellClassName, glyphClassName, children }: IconPicturesProps) {
	const fileSample = useRef<SVGSVGElement | null>(null);
	const folderSample = useRef<SVGSVGElement | null>(null);
	const [paints, setPaints] = useState<GlyphPaints | null>(null);

	useLayoutEffect(() => {
		const read = () => {
			const file = fileSample.current;
			const folder = folderSample.current;
			if (!file || !folder) return;
			const next = { file: paintOf(file), folder: paintOf(folder) };
			setPaints((last) =>
				last && samePaint(last.file, next.file) && samePaint(last.folder, next.folder)
					? last
					: next,
			);
		};
		read();
		// The theme engine writes the tokens, the scheme and the icon style on the root element.
		const observer = typeof MutationObserver === 'undefined' ? null : new MutationObserver(read);
		observer?.observe(document.documentElement, { attributes: true });
		const media =
			typeof globalThis.matchMedia === 'function'
				? QUERIES.map((query) => globalThis.matchMedia(query))
				: [];
		for (const query of media) query.addEventListener?.('change', read);
		return () => {
			observer?.disconnect();
			for (const query of media) query.removeEventListener?.('change', read);
		};
	}, []);

	return (
		<>
			{/* Hidden by style: the cell's own `display` would win over the `hidden` attribute. */}
			<div
				className={cellClassName}
				style={{ display: 'none' }}
				aria-hidden="true"
				data-glyph-sample=""
			>
				<svg
					ref={fileSample}
					className={`${styles.icon} ${glyphClassName ?? ''}`}
					data-group="other"
				>
					<path data-fill d="M0 0h1v1z" />
				</svg>
				<svg
					ref={folderSample}
					className={`${styles.icon} ${glyphClassName ?? ''}`}
					data-group="folder"
				>
					<path data-fill d="M0 0h1v1z" />
				</svg>
			</div>
			<IconPicturesContext.Provider value={paints}>{children}</IconPicturesContext.Provider>
		</>
	);
}

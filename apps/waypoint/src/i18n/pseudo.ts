// Pseudo-locale transforms: accented, lengthened text and a right-to-left wrapper, made from the English catalogue
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Catalogue } from './active';

/** `{name}` interpolation tokens, which a transform must leave exactly as they are. */
const TOKEN = /(\{\w+\})/;

const ACCENTS: Record<string, string> = {
	a: 'à',
	b: 'ƀ',
	c: 'ç',
	d: 'ď',
	e: 'é',
	f: 'ƒ',
	g: 'ĝ',
	h: 'ĥ',
	i: 'î',
	j: 'ĵ',
	k: 'ķ',
	l: 'ĺ',
	m: 'ḿ',
	n: 'ñ',
	o: 'ö',
	p: 'þ',
	q: 'ǫ',
	r: 'ŕ',
	s: 'š',
	t: 'ţ',
	u: 'û',
	v: 'ṽ',
	w: 'ŵ',
	x: 'ẋ',
	y: 'ý',
	z: 'ž',
	A: 'À',
	B: 'Ɓ',
	C: 'Ç',
	D: 'Ď',
	E: 'É',
	F: 'Ƒ',
	G: 'Ĝ',
	H: 'Ĥ',
	I: 'Î',
	J: 'Ĵ',
	K: 'Ķ',
	L: 'Ĺ',
	M: 'Ḿ',
	N: 'Ñ',
	O: 'Ö',
	P: 'Þ',
	Q: 'Ǫ',
	R: 'Ŕ',
	S: 'Š',
	T: 'Ţ',
	U: 'Û',
	V: 'Ṽ',
	W: 'Ŵ',
	X: 'Ẋ',
	Y: 'Ý',
	Z: 'Ž',
};

/** How much longer than English the accented pseudo-locale runs: German and Finnish do this. */
export const EXPANSION = 0.3;

/** Applies `change` to the text between interpolation tokens only. */
function aroundTokens(message: string, change: (text: string) => string): string {
	return message
		.split(TOKEN)
		.map((part, index) => (index % 2 === 1 ? part : change(part)))
		.join('');
}

/**
 * English with accented letters, about 30% more text and `[` `]` markers at both ends: text that is
 * not from the catalogue shows up unmarked, text that is clipped loses its closing bracket, and
 * layout meets the longer strings of other languages. `{name}` tokens are left alone.
 */
export function accentMessage(message: string): string {
	const accented = aroundTokens(message, (text) =>
		[...text].map((char) => ACCENTS[char] ?? char).join(''),
	);
	const visible = message.replace(/\{\w+\}/g, '').length;
	const padding = '·'.repeat(Math.ceil(visible * EXPANSION));
	return `[${accented}${padding ? ` ${padding}` : ''}]`;
}

const RIGHT_TO_LEFT_EMBEDDING = '‫';
const POP_DIRECTIONAL_FORMATTING = '‬';
const RIGHT_TO_LEFT_MARK = '‏';

const ARABIC_PUNCTUATION: Record<string, string> = { '?': '؟', ',': '،', ';': '؛' };

/**
 * English made right to left: wrapped in a right-to-left embedding so the line runs from the
 * right, with the Arabic question mark, comma and semicolon and a right-to-left mark around each
 * `{name}` token so a value interpolated next to it keeps its place. The words stay readable, which
 * is the point: the layout mirrors and the text is still testable by eye.
 */
export function rightToLeftMessage(message: string): string {
	const marked = message
		.split(TOKEN)
		.map((part, index) =>
			index % 2 === 1
				? `${RIGHT_TO_LEFT_MARK}${part}${RIGHT_TO_LEFT_MARK}`
				: [...part].map((char) => ARABIC_PUNCTUATION[char] ?? char).join(''),
		)
		.join('');
	return `${RIGHT_TO_LEFT_EMBEDDING}${marked}${POP_DIRECTIONAL_FORMATTING}`;
}

/** A whole catalogue through one of the transforms above. */
export function pseudoCatalogue(
	source: Catalogue,
	transform: (message: string) => string,
): Catalogue {
	const out: Catalogue = {};
	for (const [id, message] of Object.entries(source)) {
		out[id as keyof Catalogue] = transform(message);
	}
	return out;
}

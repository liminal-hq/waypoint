// Finds physical direction properties in stylesheets and inline styles, which do not mirror under `dir="rtl"`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The marker that allows a line: `physical:` and the reason, in a comment on that line or the one above it. */
export const ALLOW_MARKER = 'physical:';

export interface Offence {
	line: number;
	text: string;
	why: string;
}

/** The value split on whitespace outside parentheses, with a trailing `!important` dropped. */
function tokens(value: string): string[] {
	const found: string[] = [];
	let depth = 0;
	let current = '';
	for (const char of value.replace(/\s*!important\s*$/, '')) {
		if (char === '(') depth += 1;
		if (char === ')') depth -= 1;
		if (/\s/.test(char) && depth === 0) {
			if (current) found.push(current);
			current = '';
		} else current += char;
	}
	if (current) found.push(current);
	return found;
}

/** Why a CSS declaration is physical, or `null` when it is logical or symmetric. */
export function physicalReason(property: string, value: string): string | null {
	if (/^(margin|padding|scroll-margin|scroll-padding)-(left|right)$/.test(property))
		return `${property} is physical`;
	if (/^border-(left|right)(-(color|width|style))?$/.test(property))
		return `${property} is physical`;
	if (/^border-(top|bottom)-(left|right)-radius$/.test(property)) return `${property} is physical`;
	if (property === 'left' || property === 'right') return `${property} is physical`;
	if (/^(text-align|float|clear)$/.test(property) && /^(left|right)\b/.test(value))
		return `${property}: ${value} is physical`;
	if (/\bto (left|right)\b/.test(value)) return 'a gradient direction to the left or right';
	const t = tokens(value);
	if (
		/^(margin|padding|inset|scroll-margin|scroll-padding|border-width|border-style|border-color)$/.test(
			property,
		) &&
		t.length === 4 &&
		t[1] !== t[3]
	)
		return `${property} gives the right and left different values`;
	if (property === 'border-radius') {
		const radii = tokens(value.split('/')[0]!);
		const asymmetric =
			(radii.length === 2 || radii.length === 3) && radii[0] !== radii[1]
				? true
				: radii.length === 4 && (radii[0] !== radii[1] || radii[2] !== radii[3]);
		if (asymmetric) return 'border-radius corners are not mirror-symmetric';
	}
	return null;
}

/** Physical declarations in stylesheet text, skipping lines that carry the allow marker. */
export function scanCss(source: string): Offence[] {
	const found: Offence[] = [];
	const lines = source.split('\n');
	lines.forEach((text, index) => {
		if (text.includes(ALLOW_MARKER) || lines[index - 1]?.includes(ALLOW_MARKER)) return;
		const code = text.replace(/\/\*.*?\*\//g, '');
		const match = /^\s*([a-z-]+)\s*:\s*([^;{}]*?)\s*;?\s*$/.exec(code);
		if (!match) return;
		const why = physicalReason(match[1]!, match[2]!);
		if (why) found.push({ line: index + 1, text: text.trim(), why });
	});
	return found;
}

const TSX_RULES: [RegExp, string][] = [
	[/\b(margin|padding|border)(Left|Right)\b/, 'a physical camelCase style property'],
	[/\btextAlign\s*:\s*['"](left|right)['"]/, 'textAlign left or right'],
	[/\bstyle=\{\{[^}]*\b(left|right)\s*:/, 'an inline left or right'],
];

/** Physical inline styles in component source, skipping lines that carry the allow marker. */
export function scanTsx(source: string): Offence[] {
	const found: Offence[] = [];
	source.split('\n').forEach((text, index) => {
		if (text.includes(ALLOW_MARKER)) return;
		for (const [pattern, why] of TSX_RULES) {
			if (pattern.test(text)) {
				found.push({ line: index + 1, text: text.trim(), why });
				break;
			}
		}
	});
	return found;
}

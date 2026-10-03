// Verifies the regions' opacity tiers, the contrast floor that keeps text readable, and when a window draws translucent
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { contrastRatio, parseHex, TEXT_CONTRAST } from './accent';
import {
	alphaFloor,
	BACKDROP_UNDER_DARK,
	BACKDROP_UNDER_LIGHT,
	effectiveAlphas,
	FALLBACK_FLOOR,
	NO_SURFACE_COLOURS,
	requestedAlphas,
	transparencyState,
	type SurfaceColours,
} from './transparency';

const LIGHT: SurfaceColours = {
	text: '#211d1a',
	chrome: '#ffffff',
	window: '#f4f1ee',
	sidebar: '#ebe7e3',
	content: '#fbf9f7',
	menu: '#ffffff',
};
const DARK: SurfaceColours = {
	text: '#f5f1ed',
	chrome: '#292524',
	window: '#1c1917',
	sidebar: '#211e1b',
	content: '#171412',
	menu: '#292524',
};
const regions = { sidebar: true, content: false, titleBar: true };
const input = {
	opacity: 82,
	rowsOpacity: 90,
	sidebarOpacity: 94,
	contentOpacity: 98,
	regions,
	menus: false,
	menuOpacity: 96,
};

/** The blend the compositor makes, as a colour. */
function blended(surface: string, backdrop: string, alpha: number): string {
	const s = parseHex(surface)!;
	const b = parseHex(backdrop)!;
	return (
		'#' +
		s
			.map((v, i) =>
				Math.round(v * alpha + b[i]! * (1 - alpha))
					.toString(16)
					.padStart(2, '0'),
			)
			.join('')
	);
}

describe('requestedAlphas', () => {
	it('gives every part its own opacity, the defaults reproducing the old tiers of 82', () => {
		expect(requestedAlphas(input)).toEqual({
			titleBar: 0.82,
			rows: 0.9,
			sidebar: 0.94,
			content: 1,
			menu: 1,
		});
		const all = requestedAlphas({
			...input,
			opacity: 40,
			rowsOpacity: 55,
			sidebarOpacity: 70,
			contentOpacity: 98,
			regions: { sidebar: true, content: true, titleBar: true },
			menus: true,
			menuOpacity: 60,
		});
		expect(all).toEqual({ titleBar: 0.4, rows: 0.55, sidebar: 0.7, content: 0.98, menu: 0.6 });
	});

	it('does not let the title bar slider move the other parts', () => {
		const a = requestedAlphas(input);
		const b = requestedAlphas({ ...input, opacity: 40 });
		expect(b.titleBar).toBe(0.4);
		expect([b.rows, b.sidebar, b.content]).toEqual([a.rows, a.sidebar, a.content]);
	});

	it('keeps a part solid when its region is not translucent', () => {
		const none = requestedAlphas({
			...input,
			regions: { sidebar: false, content: false, titleBar: false },
		});
		expect([none.titleBar, none.rows, none.sidebar, none.content]).toEqual([1, 1, 1, 1]);
		expect(requestedAlphas({ ...input, sidebarOpacity: 100 }).sidebar).toBe(1);
	});
});

describe('alphaFloor', () => {
	it('keeps 4.5:1 on the blend with the worst backdrop, at the least opacity that does', () => {
		for (const [text, surface, backdrop] of [
			[LIGHT.text!, LIGHT.content!, BACKDROP_UNDER_LIGHT],
			[LIGHT.text!, LIGHT.sidebar!, BACKDROP_UNDER_LIGHT],
			[DARK.text!, DARK.content!, BACKDROP_UNDER_DARK],
			[DARK.text!, DARK.window!, BACKDROP_UNDER_DARK],
		] as const) {
			const floor = alphaFloor(text, surface)!;
			expect(floor, `${surface}`).toBeGreaterThanOrEqual(0);
			expect(floor).toBeLessThanOrEqual(1);
			expect(contrastRatio(text, blended(surface, backdrop, floor))).toBeGreaterThanOrEqual(
				TEXT_CONTRAST - 0.02,
			);
			if (floor > 0.02) {
				// One step lower is under the line: the floor is the least opacity, not a safe round number.
				expect(contrastRatio(text, blended(surface, backdrop, floor - 0.03))).toBeLessThan(
					TEXT_CONTRAST,
				);
			}
		}
	});

	it('lands between a third and two thirds for the real themes, so the slider keeps most of its range', () => {
		for (const colours of [LIGHT, DARK]) {
			for (const surface of [colours.chrome, colours.window, colours.sidebar, colours.content]) {
				const floor = alphaFloor(colours.text!, surface!)!;
				expect(floor).toBeGreaterThan(0.3);
				expect(floor).toBeLessThan(0.7);
			}
		}
	});

	it('is solid when the pair cannot reach 4.5:1 even solid, and unknown for a colour it cannot read', () => {
		expect(alphaFloor('#ffffff', '#eeeeee')).toBe(1);
		expect(alphaFloor('nonsense', '#ffffff')).toBeNull();
		expect(alphaFloor('#ffffff', 'rgb(0 0 0)')).toBeNull();
	});
});

describe('effectiveAlphas', () => {
	it('draws what was asked for where the text stays readable', () => {
		const { alphas, raised } = effectiveAlphas(input, LIGHT);
		expect(alphas).toEqual(requestedAlphas(input));
		expect(Object.values(raised).some(Boolean)).toBe(false);
	});

	it('lifts a part to its floor when the person asks for less, and no part past solid', () => {
		const low = {
			...input,
			opacity: 40,
			rowsOpacity: 40,
			sidebarOpacity: 40,
			contentOpacity: 40,
			regions: { sidebar: true, content: true, titleBar: true },
		};
		const { alphas, requested, raised } = effectiveAlphas(low, DARK);
		for (const region of ['titleBar', 'rows', 'sidebar', 'content'] as const) {
			expect(alphas[region]).toBeGreaterThanOrEqual(requested[region]);
			expect(alphas[region]).toBeLessThanOrEqual(1);
		}
		expect(raised.titleBar).toBe(true);
		expect(alphas.titleBar).toBe(alphaFloor(DARK.text!, DARK.chrome!));
		expect(alphas.titleBar).toBeGreaterThan(0.4);
	});

	it('leaves a solid part solid and never raises it', () => {
		const { alphas, raised } = effectiveAlphas({ ...input, opacity: 40 }, DARK);
		expect(alphas.content).toBe(1);
		expect(raised.content).toBe(false);
		expect(alphas.menu).toBe(1);
	});

	it('falls back to a 70 % floor when the colours cannot be read', () => {
		const { alphas } = effectiveAlphas({ ...input, opacity: 40 }, NO_SURFACE_COLOURS);
		expect(alphas.titleBar).toBe(FALLBACK_FLOOR);
		expect(effectiveAlphas(input, NO_SURFACE_COLOURS).alphas.titleBar).toBe(0.82);
	});

	it('floors the menus too, so they are never fainter than their text allows', () => {
		const menus = { ...input, menus: true, menuOpacity: 60 };
		const { alphas } = effectiveAlphas(menus, DARK);
		// 60 % is already above a dark menu's floor, so it is drawn as asked.
		expect(alphas.menu).toBe(Math.max(0.6, alphaFloor(DARK.text!, DARK.menu!)!));
		const faint = effectiveAlphas({ ...menus, menuOpacity: 60 }, { ...DARK, text: '#9a918a' });
		expect(faint.alphas.menu).toBeGreaterThan(0.6);
		expect(faint.raised.menu).toBe(true);
	});
});

describe('transparencyState', () => {
	const base = {
		enabled: true,
		highContrast: false,
		reducedTransparency: false,
		available: true,
		focused: true,
		solidWhenUnfocused: true,
	};

	it('is on when it is asked for and nothing stands in the way', () => {
		expect(transparencyState(base)).toEqual({ on: true, reason: null });
	});

	it('is off, without a reason, when it is not asked for', () => {
		expect(transparencyState({ ...base, enabled: false })).toEqual({ on: false, reason: null });
	});

	it('is off under high contrast and under reduced transparency, whatever else holds', () => {
		expect(transparencyState({ ...base, highContrast: true })).toEqual({
			on: false,
			reason: 'high-contrast',
		});
		expect(transparencyState({ ...base, reducedTransparency: true })).toEqual({
			on: false,
			reason: 'reduced-transparency',
		});
		expect(
			transparencyState({
				...base,
				highContrast: true,
				reducedTransparency: true,
				available: false,
			}).reason,
		).toBe('high-contrast');
	});

	it('is off until the platform says windows can be see-through, and when it says they cannot', () => {
		expect(transparencyState({ ...base, available: null }).reason).toBe('unavailable');
		expect(transparencyState({ ...base, available: false }).reason).toBe('unavailable');
	});

	it('is solid while the window is not in front, unless that is turned off', () => {
		expect(transparencyState({ ...base, focused: false })).toEqual({
			on: false,
			reason: 'unfocused',
		});
		expect(transparencyState({ ...base, focused: false, solidWhenUnfocused: false }).on).toBe(true);
	});
});

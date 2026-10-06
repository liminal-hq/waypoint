// Decides how see-through each part of a window is, and the least opacity that keeps its text readable
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { BlurLevel } from '@liminal-hq/waypoint-protocol/generated/BlurLevel';
import type { TransparencyRegions } from '@liminal-hq/waypoint-protocol/generated/TransparencyRegions';
import { luminance, parseHex, TEXT_CONTRAST } from './accent';

/** The opacity a surface falls back to when its colours cannot be read (a plain browser, a test). */
export const FALLBACK_FLOOR = 0.7;

/**
 * The backdrop the floor is worked out against: the worst a wallpaper reasonably gets for the
 * text on it. Under a light window that is black; under a dark window it is a light grey (not
 * white: a pure white wallpaper under a translucent dark window cannot be saved by any opacity
 * short of solid, and is the case the "Solid when unfocused" and blur settings are there for).
 */
export const BACKDROP_UNDER_LIGHT = '#000000';
export const BACKDROP_UNDER_DARK = '#a0a0a0';

type Rgb = [number, number, number];

function blend(surface: Rgb, backdrop: Rgb, alpha: number): Rgb {
	return surface.map((v, i) => v * alpha + backdrop[i]! * (1 - alpha)) as Rgb;
}

function ratio(a: Rgb, b: Rgb): number {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
	return (hi + 0.05) / (lo + 0.05);
}

/**
 * The least opacity (0 to 1, in steps of 0.01) at which `text` keeps `TEXT_CONTRAST` against
 * `surface` laid over the worst backdrop. The compositor blends in sRGB, so the blend is too.
 * `1` when the pair does not reach the contrast even on a solid surface (more opacity cannot help,
 * and the surface is as solid as it gets). `null` when a colour is not `#rrggbb`.
 */
export function alphaFloor(text: string, surface: string): number | null {
	const t = parseHex(text);
	const s = parseHex(surface);
	if (!t || !s) return null;
	const backdrop = parseHex(
		luminance(t) > luminance(s) ? BACKDROP_UNDER_DARK : BACKDROP_UNDER_LIGHT,
	)!;
	const reaches = (alpha: number): boolean => ratio(t, blend(s, backdrop, alpha)) >= TEXT_CONTRAST;
	if (reaches(0)) return 0;
	if (!reaches(1)) return 1;
	let low = 0;
	let high = 1;
	for (let i = 0; i < 24; i += 1) {
		const mid = (low + high) / 2;
		if (reaches(mid)) high = mid;
		else low = mid;
	}
	return Math.min(1, Math.ceil(high * 100) / 100);
}

/** The colours the floor is worked out from, as `#rrggbb`; `null` for what could not be read. */
export interface SurfaceColours {
	text: string | null;
	chrome: string | null;
	window: string | null;
	sidebar: string | null;
	content: string | null;
	menu: string | null;
}

export const NO_SURFACE_COLOURS: SurfaceColours = {
	text: null,
	chrome: null,
	window: null,
	sidebar: null,
	content: null,
	menu: null,
};

/** The parts of a window that have their own opacity. */
export type Region = 'titleBar' | 'rows' | 'sidebar' | 'content' | 'menu';

export type RegionAlphas = Record<Region, number>;

export interface TransparencyInput {
	/** The title bar's and menu bar's opacity in percent (40 to 100). */
	opacity: number;
	/** The tabs', toolbar's and status bar's opacity in percent (40 to 100). */
	rowsOpacity: number;
	/** The sidebar's and Inspector's opacity in percent (40 to 100). */
	sidebarOpacity: number;
	/** The file area's opacity in percent (40 to 100). */
	contentOpacity: number;
	/** The blur behind the window. `high` draws the four parts above at `BLUR_OPACITY` so it shows. */
	blur: BlurLevel;
	regions: TransparencyRegions;
	menus: boolean;
	/** The menus' opacity in percent (60 to 100). */
	menuOpacity: number;
}

/**
 * The opacity, in percent, that the title bar, the tabs and toolbar, the sidebar and the file area
 * are drawn at while the blur is on: the least the settings allow, so the blur behind the window
 * shows through instead of being covered by their own opacity. Their sliders and their switches
 * keep what the person set and are dimmed meanwhile (the switches read as on: the blur is behind
 * every part); the contrast floor still lifts any part whose text would not read.
 */
export const BLUR_OPACITY = 40;

/** Whether a blur is being drawn behind the window, which takes over the opacity of its parts and keeps the window from drawing solid when it is not in front. */
export function blurShows(blur: BlurLevel): boolean {
	return blur === 'high';
}

export interface EffectiveTransparency {
	/** What the page draws, 0 (invisible) to 1 (solid), per part of the window. */
	alphas: RegionAlphas;
	/** What the person asked for, before the floor. */
	requested: RegionAlphas;
	/** Whether the floor lifted that part above what was asked for. */
	raised: Record<Region, boolean>;
}

const REGIONS: Region[] = ['titleBar', 'rows', 'sidebar', 'content', 'menu'];

const clamp01 = (value: number): number => Math.min(1, Math.max(0, value));
const hundredths = (value: number): number => Math.round(clamp01(value) * 100) / 100;

/**
 * What each part of the window would be at the person's settings, before the contrast floor: its
 * own opacity, or solid when its switch is off. The tabs and toolbar follow the title bar's switch.
 * With the blur on, the four parts are all translucent whatever their switches say, at `BLUR_OPACITY`.
 */
export function requestedAlphas(input: TransparencyInput): RegionAlphas {
	const own = (percent: number): number => hundredths(percent / 100);
	const blurred = blurShows(input.blur);
	// The menus keep their own switch and opacity.
	const part = (on: boolean, percent: number): number =>
		blurred || on ? own(blurred ? BLUR_OPACITY : percent) : 1;
	return {
		titleBar: part(input.regions.titleBar, input.opacity),
		rows: part(input.regions.titleBar, input.rowsOpacity),
		sidebar: part(input.regions.sidebar, input.sidebarOpacity),
		content: part(input.regions.content, input.contentOpacity),
		menu: input.menus ? own(input.menuOpacity) : 1,
	};
}

/**
 * What the page draws: each part's requested opacity, lifted to the floor that keeps its text at
 * 4.5:1 over the worst backdrop. The floor is the scrim of SPEC 9: a solid backing under the text
 * of exactly the strength it needs, never more. A part whose colours cannot be read gets
 * `FALLBACK_FLOOR`.
 */
export function effectiveAlphas(
	input: TransparencyInput,
	colours: SurfaceColours,
): EffectiveTransparency {
	const requested = requestedAlphas(input);
	const surface: Record<Region, string | null> = {
		titleBar: colours.chrome,
		rows: colours.window,
		sidebar: colours.sidebar,
		content: colours.content,
		menu: colours.menu,
	};
	const alphas = { ...requested };
	const raised = {} as Record<Region, boolean>;
	for (const region of REGIONS) {
		const floor =
			colours.text && surface[region]
				? (alphaFloor(colours.text, surface[region]) ?? FALLBACK_FLOOR)
				: FALLBACK_FLOOR;
		alphas[region] = requested[region] < 1 ? Math.max(requested[region], floor) : 1;
		raised[region] = alphas[region] > requested[region];
	}
	return { alphas, requested, raised };
}

/** Why transparency is not drawing though it is switched on; `null` when it is, or when it is not asked for. */
export type TransparencyOffReason =
	'high-contrast' | 'reduced-transparency' | 'unavailable' | 'unfocused' | null;

/**
 * Whether the window draws translucent now, and why not when it does not. The order is the order
 * of the guardrails: accessibility first (a person who needs it never sees it), then what the
 * system can do, then the window's own state.
 */
export function transparencyState(input: {
	enabled: boolean;
	highContrast: boolean;
	reducedTransparency: boolean;
	/** Whether the platform reports that windows can be see-through; `null` until it has said. */
	available: boolean | null;
	focused: boolean;
	solidWhenUnfocused: boolean;
	/** The blur behind the window; while it shows, a window that is not in front keeps drawing translucent. */
	blur: BlurLevel;
}): { on: boolean; reason: TransparencyOffReason } {
	if (!input.enabled) return { on: false, reason: null };
	if (input.highContrast) return { on: false, reason: 'high-contrast' };
	if (input.reducedTransparency) return { on: false, reason: 'reduced-transparency' };
	if (input.available !== true) return { on: false, reason: 'unavailable' };
	if (input.solidWhenUnfocused && !blurShows(input.blur) && !input.focused) {
		return { on: false, reason: 'unfocused' };
	}
	return { on: true, reason: null };
}

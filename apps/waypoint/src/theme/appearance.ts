// Decides a window's look from the OS preferences and the settings, and writes it onto the root
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Settings } from '@liminal-hq/waypoint-protocol/generated/Settings';
import { resolveLocale } from '../i18n/locales';
import { resolveIconTheme, type ResolvedIconTheme } from '../icons/iconTheme';
import type { Palette } from '../services/osPaletteClient';
import { resolveAccent } from './accent';
import { PALETTE_TOKENS, decidePalette, type LiftedColour, type PaletteState } from './palette';
import { blurInForce, transparencyState, type TransparencyOffReason } from './transparency';

/** What the OS says about the look. `null` means it did not say. */
export interface OsAppearance {
	scheme: 'light' | 'dark' | null;
	highContrast: boolean;
	reducedMotion: boolean;
	reducedTransparency: boolean;
	/** The OS text scale (1 is normal). */
	textScale: number;
	/** The OS accent as `#rrggbb`. */
	accent: string | null;
}

/** Where the OS preferences come from: `pluginOsAppearance` (the `system-appearance` plugin over the media queries), or the media queries alone. */
export interface OsAppearanceSource {
	read(): OsAppearance;
	/** Calls `listener` when any preference changes; returns the way to stop. */
	subscribe(listener: () => void): () => void;
}

export const NO_OS_APPEARANCE: OsAppearance = {
	scheme: null,
	highContrast: false,
	reducedMotion: false,
	reducedTransparency: false,
	textScale: 1,
	accent: null,
};

const QUERIES = {
	dark: '(prefers-color-scheme: dark)',
	light: '(prefers-color-scheme: light)',
	contrast: '(prefers-contrast: more)',
	motion: '(prefers-reduced-motion: reduce)',
	transparency: '(prefers-reduced-transparency: reduce)',
} as const;

/**
 * The webview's own media queries: all there is where the OS reports nothing better. WebKitGTK does
 * not report every one reliably, nor the text scale or the accent at all, which is why Rust's answer
 * (`pluginOsAppearance`) is laid over this wherever it has one (A58).
 */
export function webOsAppearance(): OsAppearanceSource {
	const lists = (): MediaQueryList[] => {
		if (typeof globalThis.matchMedia !== 'function') return [];
		return Object.values(QUERIES).map((query) => globalThis.matchMedia(query));
	};
	return {
		read() {
			if (typeof globalThis.matchMedia !== 'function') return NO_OS_APPEARANCE;
			const matches = (query: string): boolean => globalThis.matchMedia(query).matches;
			return {
				...NO_OS_APPEARANCE,
				scheme: matches(QUERIES.dark) ? 'dark' : matches(QUERIES.light) ? 'light' : null,
				highContrast: matches(QUERIES.contrast),
				reducedMotion: matches(QUERIES.motion),
				reducedTransparency: matches(QUERIES.transparency),
			};
		},
		subscribe(listener) {
			const all = lists();
			for (const list of all) list.addEventListener?.('change', listener);
			return () => {
				for (const list of all) list.removeEventListener?.('change', listener);
			};
		},
	};
}

export interface ResolvedAppearance {
	theme: 'light' | 'dark' | null;
	contrast: 'normal' | 'high';
	density: 'compact' | 'comfortable' | 'spacious';
	touch: boolean;
	motion: 'reduce' | 'full';
	transparency: 'on' | 'off';
	/** Why transparency is switched on but not drawing (`null` when it draws, or is not asked for). */
	transparencyReason: TransparencyOffReason;
	/** Text scale, 1 for normal. */
	textScale: number;
	/** Set when the person's accent differs from the brand colour; the tokens carry the brand one. */
	accent: { fill: string; text: string } | null;
	/** Whether the OS palette is drawn (D144), and the tokens it sets when it is. */
	palette: {
		state: PaletteState;
		tokens: Record<string, string> | null;
		/** The colours the contrast floor moved, for the Appearance page's note. */
		lifted: LiftedColour[];
		/** Whether the system has a usable palette; `null` until it has said. */
		available: boolean | null;
	};
	lang: string;
	dir: 'ltr' | 'rtl';
	iconStyle: Settings['appearance']['iconStyle'];
	/** The icon set the views draw (`system` draws as `waypoint` for now). */
	iconTheme: ResolvedIconTheme;
	folderColour: Settings['appearance']['folderColour'];
	strongFocus: boolean;
}

const RTL_LANGUAGES = new Set(['ar', 'he', 'fa', 'ur', 'ps', 'sd', 'ug', 'yi', 'dv']);

/** The direction a language is written in. */
export function directionFor(language: string): 'ltr' | 'rtl' {
	const primary = language.split(/[-_]/)[0]!.toLowerCase();
	return RTL_LANGUAGES.has(primary) ? 'rtl' : 'ltr';
}

function follow(choice: 'follow' | 'on' | 'off', os: boolean): boolean {
	return choice === 'follow' ? os : choice === 'on';
}

/** What the window should look like, from the settings, the OS and whether the last pointer was touch. */
export function resolveAppearance(
	settings: Settings,
	os: OsAppearance,
	options: {
		touchPointer: boolean;
		systemLanguage: string;
		/** Whether the developer-only pseudo-locales exist (default: a developer build). */
		developer?: boolean;
		hasFinePointer: boolean;
		/** Whether the window is in front (default true), which "Solid when unfocused" depends on. */
		focused?: boolean;
		/** Whether the platform reports that windows can be see-through: `null` until it has said (default true). */
		opacityAvailable?: boolean | null;
		/** Whether the platform can blur behind the window: `null` until it has said (default true, as for `opacityAvailable`). */
		blurAvailable?: boolean | null;
		/** The OS palette, or `null` until the plugin has answered or where it cannot. */
		palette?: Palette | null;
	},
): ResolvedAppearance {
	const { appearance, accessibility, locale, transparency } = settings;
	const highContrast = follow(accessibility.highContrast, os.highContrast);
	const palette = decidePalette({
		enabled: appearance.matchSystemColours,
		palette: options.palette ?? null,
		mode: appearance.mode,
		highContrast,
	});
	// With the palette drawn in "System" mode the variant is the one the palette describes, so the
	// native parts of the window (scrollbars, form controls) agree with the colours around them.
	const theme =
		palette.state === 'applied' && appearance.mode === 'system'
			? palette.mapped!.variant
			: appearance.mode === 'system'
				? appearance.themeSource === 'os' || os.scheme !== null
					? os.scheme
					: null
				: appearance.mode;
	const reducedMotion = follow(accessibility.reducedMotion, os.reducedMotion);
	const reducedTransparency = follow(accessibility.reducedTransparency, os.reducedTransparency);
	const touch =
		accessibility.touchMode === 'on' ||
		(accessibility.touchMode === 'auto' && (options.touchPointer || !options.hasFinePointer));
	// The language of the text in force, which is the catalogue in use, not the OS language itself.
	const language = resolveLocale(
		locale.language,
		options.systemLanguage,
		options.developer ?? import.meta.env.DEV,
	).locale;
	const brandAccent = appearance.accent.kind === 'ember';
	const translucent = transparencyState({
		enabled: transparency.enabled,
		highContrast,
		reducedTransparency,
		available: options.opacityAvailable === undefined ? true : options.opacityAvailable,
		focused: options.focused ?? true,
		solidWhenUnfocused: transparency.solidWhenUnfocused,
		blur: blurInForce(
			transparency.blur,
			options.blurAvailable === undefined ? true : options.blurAvailable,
		),
	});
	return {
		theme,
		contrast: highContrast ? 'high' : 'normal',
		density: appearance.density,
		touch,
		motion: reducedMotion ? 'reduce' : 'full',
		transparency: translucent.on ? 'on' : 'off',
		transparencyReason: translucent.reason,
		textScale: Math.max(accessibility.textSize / 100, os.textScale),
		accent:
			brandAccent || highContrast
				? null
				: resolveAccent(appearance.accent, theme ?? 'light', os.accent),
		palette: {
			state: palette.state,
			tokens: palette.mapped?.tokens ?? null,
			lifted: palette.preview?.lifted ?? [],
			available: options.palette ? options.palette.status.available : null,
		},
		lang: language,
		dir: locale.direction === 'auto' ? directionFor(language) : locale.direction,
		iconStyle: appearance.iconStyle,
		iconTheme: resolveIconTheme(appearance.iconTheme),
		folderColour: appearance.folderColour,
		strongFocus: accessibility.strongFocusRing || highContrast,
	};
}

/** Writes the resolved look onto `root` as attributes and custom properties. */
export function applyAppearance(root: HTMLElement, look: ResolvedAppearance): void {
	const set = (name: string, value: string | null): void => {
		if (value === null) delete root.dataset[name];
		else root.dataset[name] = value;
	};
	set('theme', look.theme);
	set('contrast', look.contrast);
	set('density', look.density);
	set('touch', look.touch ? 'true' : 'false');
	set('motion', look.motion);
	set('transparency', look.transparency);
	set('transparencyReason', look.transparencyReason);
	set('iconStyle', look.iconStyle);
	set('iconTheme', look.iconTheme);
	set('folderColour', look.folderColour);
	set('focus', look.strongFocus ? 'strong' : 'normal');
	root.style.setProperty('--wp-text-scale', String(look.textScale));
	if (look.accent) {
		root.style.setProperty('--wp-accent', look.accent.fill);
		root.style.setProperty('--wp-accent-fg', look.accent.text);
		root.style.setProperty('--wp-accent-contrast', look.accent.text);
	} else {
		root.style.removeProperty('--wp-accent');
		root.style.removeProperty('--wp-accent-fg');
		root.style.removeProperty('--wp-accent-contrast');
	}
	for (const name of PALETTE_TOKENS) root.style.removeProperty(name);
	if (look.palette.tokens) {
		for (const [name, value] of Object.entries(look.palette.tokens)) {
			root.style.setProperty(name, value);
		}
	}
	set('palette', look.palette.tokens ? 'system' : null);
	root.lang = look.lang;
	root.dir = look.dir;
}

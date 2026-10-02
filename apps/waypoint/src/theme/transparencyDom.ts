// Reads the surface colours off the root and writes the alphas of a translucent window onto it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TransparencySettings } from '@liminal-hq/waypoint-protocol/generated/TransparencySettings';
import {
	effectiveAlphas,
	NO_SURFACE_COLOURS,
	type EffectiveTransparency,
	type Region,
	type SurfaceColours,
} from './transparency';

/** The custom property each part of the window reads its opacity from (tokens.css). */
export const ALPHA_PROPERTIES: Record<Region, string> = {
	titleBar: '--wp-alpha-title-bar',
	rows: '--wp-alpha-rows',
	sidebar: '--wp-alpha-sidebar',
	content: '--wp-alpha-content',
	menu: '--wp-alpha-menu',
};

/** The colours the floor is worked out from, read from the tokens in force on `element`; `null` for any the page cannot read (a plain browser, a test). */
export function readSurfaceColours(element: HTMLElement): SurfaceColours {
	if (typeof globalThis.getComputedStyle !== 'function') return NO_SURFACE_COLOURS;
	const style = globalThis.getComputedStyle(element);
	const read = (name: string): string | null => {
		const value = style.getPropertyValue(name).trim();
		return /^#[0-9a-fA-F]{6}$/.test(value) ? value : null;
	};
	return {
		text: read('--wp-text-primary'),
		chrome: read('--wp-bg-chrome'),
		window: read('--wp-solid-window'),
		sidebar: read('--wp-solid-sidebar'),
		content: read('--wp-solid-content'),
		menu: read('--wp-bg-raised'),
	};
}

/**
 * Writes the opacity of every part of the window onto `root`, or takes them off when the window is
 * not translucent. Returns what it wrote, for a caller that wants to show it.
 */
export function applyTransparency(
	root: HTMLElement,
	on: boolean,
	settings: TransparencySettings,
): EffectiveTransparency | null {
	const properties = Object.values(ALPHA_PROPERTIES);
	if (!on) {
		for (const name of properties) root.style.removeProperty(name);
		delete root.dataset.menus;
		return null;
	}
	const effective = effectiveAlphas(
		{
			opacity: settings.opacity,
			regions: settings.regions,
			menus: settings.menus,
			menuOpacity: settings.menuOpacity,
		},
		readSurfaceColours(root),
	);
	for (const [region, name] of Object.entries(ALPHA_PROPERTIES)) {
		root.style.setProperty(name, String(effective.alphas[region as Region]));
	}
	root.dataset.menus = effective.alphas.menu < 1 ? 'translucent' : 'solid';
	return effective;
}

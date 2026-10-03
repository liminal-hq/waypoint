// What "Reset to defaults" on the Transparency page changes, and when there is nothing to reset
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TransparencySettings } from '@liminal-hq/waypoint-protocol/generated/TransparencySettings';
import { DEFAULT_SETTINGS } from '../services/settingsClient';

/**
 * The settings with every tuning on the Transparency page back at its default: the four
 * opacities, the menus' switch and opacity, blur, the parts of the window and "Solid when not in
 * front". The "Transparent window" switch is kept as it is: it chooses whether the feature is
 * used at all, and a reset that switched the window opaque (or translucent) would be a surprise.
 */
export function resetTransparency(current: TransparencySettings): TransparencySettings {
	return { ...DEFAULT_SETTINGS.transparency, enabled: current.enabled };
}

/** Whether every setting `resetTransparency` restores is already at its default. */
export function transparencyAtDefaults(current: TransparencySettings): boolean {
	const defaults = resetTransparency(current);
	return (Object.keys(defaults) as (keyof TransparencySettings)[]).every((key) =>
		key === 'regions'
			? (Object.keys(defaults.regions) as (keyof typeof defaults.regions)[]).every(
					(region) => defaults.regions[region] === current.regions[region],
				)
			: defaults[key] === current[key],
	);
}

// The native plugins whose availability the Services panel reports, adapted to the shared status shape
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getStatus as nativeDndStatus } from '@liminal-hq/plugin-native-dnd';
import { getStatus as osPrefsStatus } from '@liminal-hq/plugin-os-prefs';
import { getStatus as systemAppearanceStatus } from '@liminal-hq/plugin-system-appearance';
import { getStatus as thumbnailsStatus } from '@liminal-hq/plugin-thumbnails';
import { getStatus as trashStatus } from '@liminal-hq/plugin-trash';
import { getStatus as volumesStatus } from '@liminal-hq/plugin-volumes';
import { getStatus as windowEffectsStatus } from '@liminal-hq/plugin-window-effects';
import { getStatus as windowManagerStatus } from '@liminal-hq/plugin-window-manager';
import { getStatus as windowTearoffStatus } from '@liminal-hq/plugin-window-tearoff';
import { getStatus as opsStatus } from '@liminal-hq/waypoint-plugin-ops';
import { getStatus as sessionStatus } from '@liminal-hq/waypoint-plugin-session';
import { getStatus as settingsStatus } from '@liminal-hq/waypoint-plugin-settings';
import { getStatus as vfsStatus } from '@liminal-hq/waypoint-plugin-vfs';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import { collectStatuses, type StatusSource } from './status';

/** One feature of a reusable plugin: whether it works and, when it does not, why. */
interface FeatureReport {
	available: boolean;
	reason: string | null;
}

/** The feature names that work, and the first reason one that does not (or works only partly) gives. */
function summarise(
	status: { available: boolean; reason: string | null },
	features: ReadonlyArray<readonly [string, FeatureReport]>,
): PluginStatus {
	const working = features.filter(([, report]) => report.available).map(([name]) => name);
	const reason =
		status.reason ?? features.find(([, report]) => !report.available && report.reason)?.[1].reason;
	return { available: status.available, reason: reason ?? null, features: working };
}

/**
 * The Trash plugin's status in the shared shape. It stays available when it can only move files to
 * the Trash (a sandbox's portal), with the features it lacks (`list`, `restore`, `empty`, `expiry`)
 * missing from the list and the first reason given, which is what the Trash place explains.
 */
export async function trashServiceStatus(): Promise<PluginStatus> {
	const status = await trashStatus();
	return summarise(
		status,
		status.features.map((feature) => [feature.name, feature] as const),
	);
}

/** The system preferences plugin's status in the shared shape: its features carry their own reasons. */
export async function osPrefsServiceStatus(): Promise<PluginStatus> {
	const status = await osPrefsStatus();
	return summarise(
		status,
		status.features.map((feature) => [feature.name, feature] as const),
	);
}

/** The thumbnails plugin's status in the shared shape: its features carry their own reasons. */
export async function thumbnailsServiceStatus(): Promise<PluginStatus> {
	const status = await thumbnailsStatus();
	return summarise(
		{ available: status.available, reason: status.reason?.message ?? null },
		status.features.map(
			(feature) =>
				[
					feature.name,
					{ available: feature.available, reason: feature.reason?.message ?? null },
				] as const,
		),
	);
}

/** The volumes plugin's status in the shared shape: its features carry typed reasons, shown as the sentence they hold. */
export async function volumesServiceStatus(): Promise<PluginStatus> {
	const status = await volumesStatus();
	return summarise(
		{ available: status.available, reason: status.message },
		status.features.map(
			(feature) =>
				[feature.name, { available: feature.available, reason: feature.message }] as const,
		),
	);
}

/** The reason codes of features that belong to another platform (Mica on Linux, the shadow inset on Windows): not a fault to show. */
const FOREIGN_REASONS: ReadonlyArray<string> = ['windows-only', 'gtk-only'];

/** The window effects plugin's status in the shared shape: its features carry typed reasons, shown as the sentence they hold, except for the ones that simply belong to another platform. */
export async function windowEffectsServiceStatus(): Promise<PluginStatus> {
	const status = await windowEffectsStatus();
	return summarise(
		{ available: status.available, reason: status.message },
		status.features.map(
			(feature) =>
				[
					feature.name,
					{
						available: feature.available,
						reason:
							feature.reason && FOREIGN_REASONS.includes(feature.reason) ? null : feature.message,
					},
				] as const,
		),
	);
}

/** The native drag and drop plugin's status in the shared shape. */
export async function nativeDndServiceStatus(): Promise<PluginStatus> {
	const status = await nativeDndStatus();
	return summarise(status, Object.entries(status.features));
}

/**
 * The plugins the panel reports, each through its own guest-js `getStatus`. Every plugin
 * `src-tauri` registers has an entry (`scripts/check-services.sh` fails the build otherwise); the
 * key is the crate name without `tauri_plugin_`, except the file system's.
 */
export const SERVICE_SOURCES: Record<string, StatusSource> = {
	'file-system': vfsStatus,
	trash: trashServiceStatus,
	'native-dnd': nativeDndServiceStatus,
	// Whether the Shelf window (and every other window) can be kept on top, and why not where it cannot.
	'window-manager': windowManagerStatus,
	'waypoint-ops': opsStatus,
	'waypoint-session': sessionStatus,
	'waypoint-settings': settingsStatus,
	'os-prefs': osPrefsServiceStatus,
	'system-appearance': systemAppearanceStatus,
	'window-tearoff': windowTearoffStatus,
	thumbnails: thumbnailsServiceStatus,
	volumes: volumesServiceStatus,
	'window-effects': windowEffectsServiceStatus,
};

/** Asks every plugin whether it works here; one that cannot answer is reported unavailable with the error as its reason. */
export function collectServiceStatuses(): Promise<Record<string, PluginStatus>> {
	return collectStatuses(SERVICE_SOURCES);
}

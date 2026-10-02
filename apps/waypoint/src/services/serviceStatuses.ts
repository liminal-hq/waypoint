// The native plugins whose availability the Services panel reports, adapted to the shared status shape
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getStatus as nativeDndStatus } from '@liminal-hq/plugin-native-dnd';
import { getStatus as trashStatus } from '@liminal-hq/plugin-trash';
import { getStatus as opsStatus } from '@liminal-hq/waypoint-plugin-ops';
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

/** The native drag and drop plugin's status in the shared shape. */
export async function nativeDndServiceStatus(): Promise<PluginStatus> {
	const status = await nativeDndStatus();
	return summarise(status, Object.entries(status.features));
}

/** The plugins the panel reports, each through its own guest-js `getStatus`. */
export const SERVICE_SOURCES: Record<string, StatusSource> = {
	'file-system': vfsStatus,
	trash: trashServiceStatus,
	'native-dnd': nativeDndServiceStatus,
	'waypoint-ops': opsStatus,
};

/** Asks every plugin whether it works here; one that cannot answer is reported unavailable with the error as its reason. */
export function collectServiceStatuses(): Promise<Record<string, PluginStatus>> {
	return collectStatuses(SERVICE_SOURCES);
}

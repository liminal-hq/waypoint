// Aggregates the availability status that each native plugin reports through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PluginStatus } from '../domain/protocol/generated/PluginStatus';

/** A plugin's own guest-js `getStatus`, so command names stay inside the plugin. */
export type StatusSource = () => Promise<PluginStatus>;

/**
 * Asks every source whether its plugin works here. A source that throws (plugin missing,
 * permission denied, crashed) is reported as unavailable with the error as the reason,
 * so one broken plugin never hides the others.
 */
export async function collectStatuses(
	sources: Record<string, StatusSource>,
): Promise<Record<string, PluginStatus>> {
	const entries = await Promise.all(
		Object.entries(sources).map(async ([name, source]): Promise<[string, PluginStatus]> => {
			try {
				return [name, await source()];
			} catch (error) {
				const reason = error instanceof Error ? error.message : String(error);
				return [name, { available: false, reason, features: [] }];
			}
		}),
	);
	return Object.fromEntries(entries);
}

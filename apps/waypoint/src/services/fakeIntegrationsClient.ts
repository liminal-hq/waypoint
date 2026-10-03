// An IntegrationsClient that answers from values a test sets
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IntegrationAvailability } from '@liminal-hq/waypoint-protocol/generated/IntegrationAvailability';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import type { IntegrationsClient } from './integrationsClient';

const works = { available: true, reason: null };

/** Everything works: a GNOME session with the notification server, logind and a display. */
export function everythingWorks(): IntegrationAvailability {
	return {
		notifications: works,
		launcherProgress: works,
		preventSleep: works,
		fileManagerService: works,
		globalShortcut: works,
	};
}

/** The same with some switches unavailable, each with its reason. */
export function without(
	reasons: Partial<Record<keyof IntegrationAvailability, string>>,
): IntegrationAvailability {
	const base = everythingWorks();
	for (const [key, reason] of Object.entries(reasons)) {
		base[key as keyof IntegrationAvailability] = { available: false, reason };
	}
	return base;
}

export interface FakeIntegrationsClient extends IntegrationsClient {
	/** How many times each was asked. */
	readonly asked: { availability: number; statuses: number };
}

/** A client that answers with `availability` and `statuses`, or rejects when `fail` is set. */
export function createFakeIntegrationsClient(
	availability: IntegrationAvailability = everythingWorks(),
	statuses: Record<string, PluginStatus> = {
		'xdg-portal': { available: false, reason: 'No desktop portal is running.', features: [] },
		'desktop-integration': {
			available: true,
			reason: null,
			features: ['notify', 'inhibitSleep', 'launcherProgress', 'fileManager', 'globalShortcuts'],
		},
	},
	fail?: string,
): FakeIntegrationsClient {
	const asked = { availability: 0, statuses: 0 };
	return {
		asked,
		async availability() {
			asked.availability += 1;
			if (fail) throw new Error(fail);
			return availability;
		},
		async statuses() {
			asked.statuses += 1;
			if (fail) throw new Error(fail);
			return statuses;
		},
	};
}
